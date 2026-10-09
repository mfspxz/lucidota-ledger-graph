//! Benchmarks: real ledger + synthetic 100k scale test.

use crate::ledger::Ledger;
use crate::views;
use std::io::Write;
use std::time::Instant;

const ACTORS: &[&str] = &[
    "lucidota", "zackary", "subagent:probe", "subagent:redteam", "subagent:build",
];
const TYPES: &[&str] = &[
    "action", "decision", "observation", "check", "result", "correction", "deploy", "order",
];
const VERDICTS: &[&str] = &["proven", "disproven", "unknown", "unknown", "proven"];
const DOMAINS: &[&str] = &[
    "exchange governor claim", "ballot voting quorum", "tricycle pedicab SOP",
    "tentacle city router", "nvidia nim model", "lifelog gps heartbeat",
];

/// Generate a synthetic ledger JSONL with realistic shape.
pub fn generate_synthetic(path: &str, n: usize) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    // simple deterministic PRNG (xorshift64)
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in 0..n {
        let actor = ACTORS[(rnd() as usize) % ACTORS.len()];
        let etype = TYPES[(rnd() as usize) % TYPES.len()];
        let verdict = VERDICTS[(rnd() as usize) % VERDICTS.len()];
        let domain = DOMAINS[(rnd() as usize) % DOMAINS.len()];
        let seq_ref = if rnd() % 3 == 0 && i > 10 {
            format!(",\"refs\":[\"seq={}\"]", (rnd() as usize) % i)
        } else {
            String::new()
        };
        writeln!(
            f,
            "{{\"actor\":\"{}\",\"device\":\"vm\",\"chat\":\"side_chat:test\",\"type\":\"{}\",\"verdict\":\"{}\",\"summary\":\"Synthetic {} entry {} for benchmark\",\"detail\":\"Detail text about {} with seq={} mention\",\"ts\":\"2026-10-07T00:00:00Z\",\"seq\":{}{}}}",
            actor, etype, verdict, domain, i, domain, (rnd() as usize) % i.max(1), i + 1, seq_ref
        )?;
    }
    Ok(())
}

pub fn run_bench(ledger_path: &str) {
    // --- real ledger
    let t0 = Instant::now();
    let l = Ledger::load(ledger_path).expect("load ledger");
    let load_ms = t0.elapsed().as_millis();
    println!("real: {} entries parsed+indexed in {} ms", l.len(), load_ms);
    println!("      parse: {} ms", l.parse_ms);

    let views: &[(&str, fn(&Ledger) -> String)] = &[
        ("claims", views::view_claims),
        ("actors", views::view_actors),
        ("domains", views::view_domains),
        ("verdicts", views::view_verdicts),
        ("contradictions", views::view_contradictions),
    ];
    for (name, f) in views {
        let t = Instant::now();
        let out = f(&l);
        let ms = t.elapsed().as_millis();
        println!("real: view {:<14} {:>6} ms  ({} bytes)", name, ms, out.len());
    }

    // --- synthetic 100k
    let syn_path = "/tmp/ledger-graph-synth-100k.jsonl";
    let t = Instant::now();
    generate_synthetic(syn_path, 100_000).expect("generate");
    println!("synth: generated 100k entries in {} ms", t.elapsed().as_millis());

    let t = Instant::now();
    let sl = Ledger::load(syn_path).expect("load synth");
    let load_ms = t.elapsed().as_millis();
    println!("synth: 100k parsed+indexed in {} ms", load_ms);
    println!("synth: parse: {} ms", sl.parse_ms);

    for (name, f) in views {
        let t = Instant::now();
        let out = f(&sl);
        let ms = t.elapsed().as_millis();
        println!("synth: view {:<14} {:>6} ms  ({} bytes)", name, ms, out.len());
    }
    let total = load_ms
        + views
            .iter()
            .map(|(_, f)| {
                let t = Instant::now();
                let _ = f(&sl);
                t.elapsed().as_millis()
            })
            .sum::<u128>();
    println!("synth: full pipeline (parse+index+5 views) {} ms", total);
    println!(
        "synth: throughput {:.0} entries/sec",
        100_000.0 / (total as f64 / 1000.0)
    );
}
