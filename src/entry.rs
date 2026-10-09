//! Canonical ledger entry: normalizes the loose JSONL schema into one shape.
//!
//! Observed schema variants in ledger-0001.jsonl:
//!   content: summary+detail | text | action+detail | event+summary
//!   type:    "type" | "kind"  (case varies: DECISION vs decision)
//!   verdict: proven|disproven|unknown | done|done-standalone|PROVEN|mixed | missing
//!   refs:   ["path/...", "seq=418", "memory://..."]

use serde_json::Value;

/// Trinary verdict, normalized. Never binary: unknown is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    Proven,
    Disproven,
    Unknown,
}

impl Verdict {
    pub fn normalize(raw: Option<&str>) -> Verdict {
        match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("proven") | Some("done") | Some("done-standalone") => Verdict::Proven,
            Some("disproven") | Some("refuted") | Some("failed") => Verdict::Disproven,
            _ => Verdict::Unknown, // missing | mixed | "?" | anything else
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Proven => "proven",
            Verdict::Disproven => "disproven",
            Verdict::Unknown => "unknown",
        }
    }
}

/// One normalized ledger entry.
#[derive(Debug, Clone)]
pub struct Entry {
    pub idx: usize,        // position in parse order
    pub seq: Option<u64>, // ledger sequence (may be absent)
    pub ts: String,        // raw timestamp string
    pub actor: String,
    pub device: String,
    pub chat: String,
    pub etype: String,     // normalized lowercase type/kind
    pub verdict: Verdict,
    pub title: String,     // summary | action | event | first 120 chars of text
    pub body: String,      // detail | text | summary (whichever carries content)
    pub refs: Vec<String>, // raw refs
    pub ref_seqs: Vec<u64>,// seq numbers extracted from refs + body mentions
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn first_nonempty(vals: &[String]) -> String {
    vals.iter().find(|s| !s.trim().is_empty()).cloned().unwrap_or_default()
}

/// Extract `seq=NNN`, `seq NNN`, `seq:NNN` mentions from free text.
pub fn extract_seq_mentions(text: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i..].starts_with(b"seq") {
            let mut j = i + 3;
            while j < bytes.len() && (bytes[j] == b'=' || bytes[j] == b':' || bytes[j] == b' ' || bytes[j] == b'#') {
                j += 1;
            }
            let mut k = j;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                k += 1;
            }
            if k > j {
                if let Ok(n) = text[j..k].parse::<u64>() {
                    out.push(n);
                }
            }
            i = k.max(i + 1);
        } else {
            i += 1;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

impl Entry {
    pub fn from_value(idx: usize, v: &Value) -> Option<Entry> {
        let actor = s(v, "actor");
        if actor.is_empty() {
            return None; // actor is the one mandatory field
        }
        let summary = s(v, "summary");
        let detail = s(v, "detail");
        let text = s(v, "text");
        let action = s(v, "action");
        let event = s(v, "event");

        let title = first_nonempty(&[
            summary.clone(),
            action.clone(),
            event.clone(),
            text.chars().take(140).collect::<String>(),
        ]);
        let body = first_nonempty(&[detail, text, summary]);

        let raw_type = first_nonempty(&[s(v, "type"), s(v, "kind")]);
        let etype = raw_type.to_ascii_lowercase();
        let etype = if etype.is_empty() { "untyped".to_string() } else { etype };

        let refs: Vec<String> = v
            .get("refs")
            .and_then(|r| r.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // seq mentions from refs + body text
        let mut ref_seqs = Vec::new();
        for r in &refs {
            ref_seqs.extend(extract_seq_mentions(r));
        }
        ref_seqs.extend(extract_seq_mentions(&body));
        ref_seqs.extend(extract_seq_mentions(&title));
        ref_seqs.sort_unstable();
        ref_seqs.dedup();

        Some(Entry {
            idx,
            seq: v.get("seq").and_then(|x| x.as_u64()),
            ts: s(v, "ts"),
            actor,
            device: s(v, "device"),
            chat: s(v, "chat"),
            etype,
            verdict: Verdict::normalize(v.get("verdict").and_then(|x| x.as_str())),
            title,
            body,
            refs,
            ref_seqs,
        })
    }

    /// Combined searchable text (lowercased lazily by callers).
    pub fn full_text(&self) -> String {
        format!("{} {}", self.title, self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seq_mention_extraction() {
        assert_eq!(extract_seq_mentions("ledger seq=418 done"), vec![418]);
        assert_eq!(extract_seq_mentions("seq 420 and seq=421"), vec![420, 421]);
        assert_eq!(extract_seq_mentions("no refs here"), Vec::<u64>::new());
    }
    #[test]
    fn verdict_normalization() {
        assert_eq!(Verdict::normalize(Some("proven")), Verdict::Proven);
        assert_eq!(Verdict::normalize(Some("PROVEN")), Verdict::Proven);
        assert_eq!(Verdict::normalize(Some("done-standalone")), Verdict::Proven);
        assert_eq!(Verdict::normalize(Some("disproven")), Verdict::Disproven);
        assert_eq!(Verdict::normalize(None), Verdict::Unknown);
        assert_eq!(Verdict::normalize(Some("mixed")), Verdict::Unknown);
    }
}
