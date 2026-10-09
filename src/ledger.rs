//! Streaming parser + in-memory indexes.
//!
//! Design for 100k scale: single pass over the file, bump-allocated entries,
//! HashMap indexes built inline. No intermediate DOM retained.

use crate::entry::Entry;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

pub struct Ledger {
    pub entries: Vec<Entry>,
    pub parse_ms: u128,
    // indexes: value -> entry indices
    pub by_seq: HashMap<u64, usize>,
    pub by_actor: HashMap<String, Vec<usize>>,
    pub by_type: HashMap<String, Vec<usize>>,
    pub by_chat: HashMap<String, Vec<usize>>,
    pub by_verdict: HashMap<String, Vec<usize>>,
    // seq -> entries that reference it (reverse ref edges)
    pub referenced_by: HashMap<u64, Vec<usize>>,
}

impl Ledger {
    pub fn load(path: &str) -> std::io::Result<Ledger> {
        let t0 = Instant::now();
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(1 << 20, file); // 1MB buffer
        let mut entries: Vec<Entry> = Vec::new();
        let mut skipped = 0usize;

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if let Some(e) = Entry::from_value(entries.len(), &v) {
                entries.push(e);
            } else {
                skipped += 1;
            }
        }
        let parse_ms = t0.elapsed().as_millis();

        // build indexes in one pass
        let mut by_seq = HashMap::new();
        let mut by_actor: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_type: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_chat: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_verdict: HashMap<String, Vec<usize>> = HashMap::new();
        let mut referenced_by: HashMap<u64, Vec<usize>> = HashMap::new();

        for (i, e) in entries.iter().enumerate() {
            if let Some(sq) = e.seq {
                by_seq.entry(sq).or_insert(i);
            }
            by_actor.entry(e.actor.clone()).or_default().push(i);
            by_type.entry(e.etype.clone()).or_default().push(i);
            if !e.chat.is_empty() {
                by_chat.entry(e.chat.clone()).or_default().push(i);
            }
            by_verdict
                .entry(e.verdict.as_str().to_string())
                .or_default()
                .push(i);
            for rs in &e.ref_seqs {
                referenced_by.entry(*rs).or_default().push(i);
            }
        }

        if skipped > 0 {
            eprintln!("warn: skipped {} unparsable lines", skipped);
        }
        Ok(Ledger {
            entries,
            parse_ms,
            by_seq,
            by_actor,
            by_type,
            by_chat,
            by_verdict,
            referenced_by,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
