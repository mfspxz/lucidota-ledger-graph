//! ledger-graph: fast ledger parser + graph views.
//!
//! Usage:
//!   ledger-graph parse [PATH]              parse + index stats (JSON to stdout)
//!   ledger-graph view <name> [PATH]        materialize a graph view (JSON to stdout)
//!     names: claims | actors | domains | verdicts | contradictions
//!   ledger-graph bench [PATH]              benchmark: real + synthetic 100k
//!
//! Default PATH: ~/ledger/ledger-0001.jsonl

mod bench;
mod entry;
mod ledger;
mod organelle;
mod views;

use ledger::Ledger;

fn ledger_path(arg: Option<&String>) -> String {
    arg.cloned().unwrap_or_else(|| {
        std::env::var("HOME").unwrap_or_else(|_| "/home/hatch".to_string())
            + "/ledger/ledger-0001.jsonl"
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("parse");

    match cmd {
        "bench" => {
            bench::run_bench(&ledger_path(args.get(2)));
        }
        "view" => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let l = Ledger::load(&ledger_path(args.get(3))).expect("load ledger");
            let out = match name {
                "claims" => views::view_claims(&l),
                "actors" => views::view_actors(&l),
                "domains" => views::view_domains(&l),
                "verdicts" => views::view_verdicts(&l),
                "contradictions" => views::view_contradictions(&l),
                _ => {
                    eprintln!("unknown view '{}'. names: claims|actors|domains|verdicts|contradictions", name);
                    std::process::exit(2);
                }
            };
            println!("{}", out);
            eprintln!(
                "view '{}': {} entries -> {} bytes in (parse {} ms)",
                name,
                l.len(),
                out.len(),
                l.parse_ms
            );
        }
        "parse" => {
            let l = Ledger::load(&ledger_path(args.get(2))).expect("load ledger");
            let mut types: Vec<(&String, usize)> =
                l.by_type.iter().map(|(k, v)| (k, v.len())).collect();
            types.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            let types_s: Vec<String> =
                types.iter().map(|(k, c)| format!("\"{}\":{}", k, c)).collect();
            println!(
                "{{\"entries\":{},\"parse_ms\":{},\"actors\":{},\"types\":{{{}}},\"chats\":{},\"seq_indexed\":{},\"ref_edges\":{}}}",
                l.len(),
                l.parse_ms,
                l.by_actor.len(),
                types_s.join(","),
                l.by_chat.len(),
                l.by_seq.len(),
                l.referenced_by.values().map(|v| v.len()).sum::<usize>(),
            );
        }
        _ => {
            eprintln!("usage: ledger-graph <parse|view|bench> ...");
            std::process::exit(2);
        }
    }
}
