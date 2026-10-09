//! Graph views materialized on demand from the indexed ledger.
//!
//! All views emit JSON: {"nodes":[...],"edges":[...]} or view-specific shapes.
//! Edges are derived from three signals: explicit refs (seq mentions),
//! temporal adjacency (same actor/chat), and type-role pairing.

use crate::entry::{Entry, Verdict};
use crate::ledger::Ledger;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------- domains

/// (domain name, keywords) — first match wins, order is priority.
const DOMAINS: &[(&str, &[&str])] = &[
    ("exchange", &["exchange", "governor", "claimant", "nim-claimant", "backpressure", "lease"]),
    ("voting", &["ballot", "voting", "vote", "quorum"]),
    ("tricycle", &["tricycle", "pedicab", "trike"]),
    ("tentacle-city", &["tentacle", "cutecity", "cute-city", "cutecity.cc"]),
    ("router", &["ai-router", "router", "failover"]),
    ("redteam", &["redteam", "adversarial", "battery", "attack", "red-team"]),
    ("mcp", &["mcp-server", "mcp "]),
    ("lifelog", &["lifelog", "gps", "tracklog", "heartbeat"]),
    ("memory", &["memory", "epistemic", "constitution"]),
    ("research", &["arXiv", "arxiv", "sota", "compression", "checker", "go-50", "go50"]),
    ("marketplace", &["marketplace", "listing", "facebook"]),
    ("recovery", &["recovery", "ssd", "stolen", "dossier", "parkwreck", "parksen"]),
    ("nvidia", &["nvidia", "nim", "gpu"]),
    ("rust", &["rust", "cargo", "crate"]),
];

pub fn classify_domain(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    for (name, kws) in DOMAINS {
        for kw in *kws {
            if lower.contains(kw) {
                return name;
            }
        }
    }
    "other"
}

// ---------------------------------------------------------------- node/edge

#[derive(Debug)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub verdict: &'static str,
    pub actor: String,
    pub seq: Option<u64>,
    pub domain: String,
}

#[derive(Debug)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub rel: String,
}

fn node_id(e: &Entry) -> String {
    match e.seq {
        Some(s) => format!("seq:{}", s),
        None => format!("idx:{}", e.idx),
    }
}

fn to_node(e: &Entry, kind: &str) -> Node {
    Node {
        id: node_id(e),
        label: e.title.chars().take(90).collect(),
        kind: kind.to_string(),
        verdict: e.verdict.as_str(),
        actor: e.actor.clone(),
        seq: e.seq,
        domain: classify_domain(&e.full_text()).to_string(),
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn nodes_json(nodes: &[Node]) -> String {
    let mut o = String::from("[");
    for (i, n) in nodes.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!(
            "{{\"id\":\"{}\",\"label\":\"{}\",\"kind\":\"{}\",\"verdict\":\"{}\",\"actor\":\"{}\",\"seq\":{},\"domain\":\"{}\"}}",
            esc(&n.id),
            esc(&n.label),
            esc(&n.kind),
            n.verdict,
            esc(&n.actor),
            n.seq.map(|s| s.to_string()).unwrap_or_else(|| "null".to_string()),
            esc(&n.domain),
        ));
    }
    o.push(']');
    o
}

fn edges_json(edges: &[Edge]) -> String {
    let mut o = String::from("[");
    for (i, e) in edges.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!(
            "{{\"from\":\"{}\",\"to\":\"{}\",\"rel\":\"{}\"}}",
            esc(&e.from),
            esc(&e.to),
            esc(&e.rel)
        ));
    }
    o.push(']');
    o
}

pub fn graph_json(nodes: &[Node], edges: &[Edge]) -> String {
    format!(
        "{{\"nodes\":{},\"edges\":{},\"node_count\":{},\"edge_count\":{}}}",
        nodes_json(nodes),
        edges_json(edges),
        nodes.len(),
        edges.len()
    )
}

// ---------------------------------------------------------------- view 1: claims -> evidence -> decisions

fn is_decision(e: &Entry) -> bool {
    matches!(
        e.etype.as_str(),
        "decision" | "order" | "permission" | "guardrail" | "correction"
    )
}
fn is_evidence(e: &Entry) -> bool {
    matches!(
        e.etype.as_str(),
        "observation" | "check" | "result" | "action" | "deploy" | "redteam-round" | "verifier-fix"
    )
}

/// Claims→evidence→decisions: decisions linked to the evidence that supports
/// them (explicit seq refs first, then temporal proximity within same chat).
pub fn view_claims(l: &Ledger) -> String {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |e: &Entry, kind: &str, nodes: &mut Vec<Node>, seen: &mut HashSet<String>| {
        let n = to_node(e, kind);
        if seen.insert(n.id.clone()) {
            nodes.push(n);
        }
    };

    for e in &l.entries {
        if !is_decision(e) {
            continue;
        }
        let did = node_id(e);
        push(e, "decision", &mut nodes, &mut seen);
        // explicit refs -> evidence/claim edges
        for rs in &e.ref_seqs {
            if let Some(&ti) = l.by_seq.get(rs) {
                let t = &l.entries[ti];
                let kind = if is_evidence(t) { "evidence" } else { "claim" };
                push(t, kind, &mut nodes, &mut seen);
                edges.push(Edge {
                    from: node_id(t),
                    to: did.clone(),
                    rel: "supports".to_string(),
                });
            }
        }
        // reverse: entries that cite this decision
        if let Some(sq) = e.seq {
            if let Some(citers) = l.referenced_by.get(&sq) {
                for &ci in citers {
                    let c = &l.entries[ci];
                    if c.idx == e.idx {
                        continue;
                    }
                    let kind = if is_evidence(c) { "evidence" } else { "claim" };
                    push(c, kind, &mut nodes, &mut seen);
                    edges.push(Edge {
                        from: node_id(c),
                        to: did.clone(),
                        rel: "cites".to_string(),
                    });
                }
            }
        }
    }
    // temporal support: evidence in same chat within 15 entries before a decision
    let mut edge_set: HashSet<(String, String)> =
        edges.iter().map(|e| (e.from.clone(), e.to.clone())).collect();
    for (i, e) in l.entries.iter().enumerate() {
        if !is_decision(e) {
            continue;
        }
        let did = node_id(e);
        let lo = i.saturating_sub(15);
        for t in &l.entries[lo..i] {
            if is_evidence(t) && t.chat == e.chat && t.ref_seqs.is_empty() {
                let tid = node_id(t);
                if seen.insert(format!("n:{}", tid)) {
                    nodes.push(to_node(t, "evidence"));
                }
                // avoid duplicate edges
                if edge_set.insert((tid.clone(), did.clone())) {
                    edges.push(Edge {
                        from: tid,
                        to: did.clone(),
                        rel: "precedes".to_string(),
                    });
                }
            }
        }
    }
    graph_json(&nodes, &edges)
}

// ---------------------------------------------------------------- view 2: actor timelines

/// Per-actor chronological timelines.
pub fn view_actors(l: &Ledger) -> String {
    let mut actors: Vec<&String> = l.by_actor.keys().collect();
    actors.sort();
    let mut o = String::from("{\"actors\":[");
    for (ai, actor) in actors.iter().enumerate() {
        if ai > 0 {
            o.push(',');
        }
        let idxs = &l.by_actor[*actor];
        o.push_str(&format!(
            "{{\"actor\":\"{}\",\"entries\":{},\"timeline\":[",
            esc(actor),
            idxs.len()
        ));
        for (i, &ei) in idxs.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            let e = &l.entries[ei];
            o.push_str(&format!(
                "{{\"id\":\"{}\",\"seq\":{},\"ts\":\"{}\",\"type\":\"{}\",\"verdict\":\"{}\",\"domain\":\"{}\",\"title\":\"{}\"}}",
                esc(&node_id(e)),
                e.seq.map(|s| s.to_string()).unwrap_or_else(|| "null".to_string()),
                esc(&e.ts),
                esc(&e.etype),
                e.verdict.as_str(),
                classify_domain(&e.full_text()),
                esc(&e.title.chars().take(80).collect::<String>()),
            ));
        }
        o.push_str("]}");
    }
    o.push_str(&format!("],\"actor_count\":{}}}", actors.len()));
    o
}

// ---------------------------------------------------------------- view 3: domain clusters

/// Domain clusters with member counts, verdict mix, top actors.
pub fn view_domains(l: &Ledger) -> String {
    let mut members: HashMap<&'static str, Vec<usize>> = HashMap::new();
    for (i, e) in l.entries.iter().enumerate() {
        members.entry(classify_domain(&e.full_text())).or_default().push(i);
    }
    let mut names: Vec<&&'static str> = members.keys().collect();
    names.sort_by_key(|n| std::cmp::Reverse(members[*n].len()));
    let mut o = String::from("{\"domains\":[");
    for (di, name) in names.iter().enumerate() {
        if di > 0 {
            o.push(',');
        }
        let idxs = &members[*name];
        let mut vcount = [0usize; 3];
        let mut actors: HashMap<&str, usize> = HashMap::new();
        let mut types: HashMap<&str, usize> = HashMap::new();
        for &ei in idxs {
            let e = &l.entries[ei];
            match e.verdict {
                Verdict::Proven => vcount[0] += 1,
                Verdict::Disproven => vcount[1] += 1,
                Verdict::Unknown => vcount[2] += 1,
            }
            *actors.entry(e.actor.as_str()).or_default() += 1;
            *types.entry(e.etype.as_str()).or_default() += 1;
        }
        let mut top_actors: Vec<(&&str, &usize)> = actors.iter().collect();
        top_actors.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        let top_actors_s: Vec<String> = top_actors
            .iter()
            .take(5)
            .map(|(a, c)| format!("\"{}\":{}", esc(a), c))
            .collect();
        let mut top_types: Vec<(&&str, &usize)> = types.iter().collect();
        top_types.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        let top_types_s: Vec<String> = top_types
            .iter()
            .take(5)
            .map(|(a, c)| format!("\"{}\":{}", esc(a), c))
            .collect();
        o.push_str(&format!(
            "{{\"domain\":\"{}\",\"entries\":{},\"verdicts\":{{\"proven\":{},\"disproven\":{},\"unknown\":{}}},\"top_actors\":{{{}}},\"top_types\":{{{}}}}}",
            name,
            idxs.len(),
            vcount[0],
            vcount[1],
            vcount[2],
            top_actors_s.join(","),
            top_types_s.join(","),
        ));
    }
    o.push_str("]}");
    o
}

// ---------------------------------------------------------------- view 4: verdict distributions

/// Verdict distribution overall, by type, by domain.
pub fn view_verdicts(l: &Ledger) -> String {
    let mut o = String::from("{\"overall\":{");
    for (i, v) in ["proven", "disproven", "unknown"].iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let n = l.by_verdict.get(*v).map(|x| x.len()).unwrap_or(0);
        o.push_str(&format!("\"{}\":{}", v, n));
    }
    o.push_str("},\"by_type\":{");
    let mut types: Vec<&String> = l.by_type.keys().collect();
    types.sort();
    for (ti, t) in types.iter().enumerate() {
        if ti > 0 {
            o.push(',');
        }
        let mut c = [0usize; 3];
        for &ei in &l.by_type[*t] {
            match l.entries[ei].verdict {
                Verdict::Proven => c[0] += 1,
                Verdict::Disproven => c[1] += 1,
                Verdict::Unknown => c[2] += 1,
            }
        }
        o.push_str(&format!(
            "\"{}\":{{\"proven\":{},\"disproven\":{},\"unknown\":{}}}",
            esc(t),
            c[0],
            c[1],
            c[2]
        ));
    }
    o.push_str("},\"by_domain\":{");
    let mut dm: HashMap<&'static str, [usize; 3]> = HashMap::new();
    for e in &l.entries {
        let d = classify_domain(&e.full_text());
        let slot = match e.verdict {
            Verdict::Proven => 0,
            Verdict::Disproven => 1,
            Verdict::Unknown => 2,
        };
        dm.entry(d).or_default()[slot] += 1;
    }
    let mut dns: Vec<&&'static str> = dm.keys().collect();
    dns.sort();
    for (di, d) in dns.iter().enumerate() {
        if di > 0 {
            o.push(',');
        }
        let c = dm[*d];
        o.push_str(&format!(
            "\"{}\":{{\"proven\":{},\"disproven\":{},\"unknown\":{}}}",
            d, c[0], c[1], c[2]
        ));
    }
    o.push_str("}}");
    o
}

// ---------------------------------------------------------------- view 5: contradiction surfaces

/// Contradiction surface: disproven entries, corrections, and entries that
/// cite disproven ones — where the ledger disagrees with itself.
pub fn view_contradictions(l: &Ledger) -> String {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for e in &l.entries {
        let is_contra = e.verdict == Verdict::Disproven
            || e.etype == "correction"
            || e.etype == "failure"
            || e.etype == "incident";
        if !is_contra {
            continue;
        }
        let kind = if e.verdict == Verdict::Disproven {
            "disproven"
        } else {
            "correction"
        };
        let eid = node_id(e);
        if seen.insert(eid.clone()) {
            nodes.push(to_node(e, kind));
        }
        // what does it refute / what cites it
        for rs in &e.ref_seqs {
            if let Some(&ti) = l.by_seq.get(rs) {
                let t = &l.entries[ti];
                let tid = node_id(t);
                if seen.insert(tid.clone()) {
                    nodes.push(to_node(t, "refuted-target"));
                }
                edges.push(Edge {
                    from: eid.clone(),
                    to: tid,
                    rel: "refutes".to_string(),
                });
            }
        }
        if let Some(sq) = e.seq {
            if let Some(citers) = l.referenced_by.get(&sq) {
                for &ci in citers {
                    let c = &l.entries[ci];
                    if c.idx == e.idx {
                        continue;
                    }
                    let cid = node_id(c);
                    if seen.insert(cid.clone()) {
                        nodes.push(to_node(c, "cites-contra"));
                    }
                    edges.push(Edge {
                        from: cid,
                        to: eid.clone(),
                        rel: "cites".to_string(),
                    });
                }
            }
        }
    }
    graph_json(&nodes, &edges)
}
