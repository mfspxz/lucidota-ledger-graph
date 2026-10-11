[Active R&D Lab] — no CI runs on record; active development, expect rough edges.

# ledger-graph

Fast ledger parser + on-demand graph views. Rust. Zero Python at runtime.

## Usage

```sh
ledger-graph parse [PATH]           # parse + index stats (JSON)
ledger-graph view <name> [PATH]     # materialize a graph view (JSON to stdout)
ledger-graph bench [PATH]           # benchmark: real ledger + synthetic 100k
```

Default PATH: `~/ledger/ledger-0001.jsonl`.

## Views

| View | Output |
|---|---|
| `claims` | claims→evidence→decisions graph (nodes/edges JSON). Decisions = decision/order/permission/guardrail/correction types; evidence = observation/check/result/action/deploy. Edges from explicit `seq=` refs, reverse citations, and temporal proximity (same chat, ≤15 entries before). |
| `actors` | Per-actor chronological timelines (100 actors currently). |
| `domains` | Domain clusters by keyword classification: exchange, voting, tricycle, tentacle-city, router, redteam, mcp, lifelog, memory, research, marketplace, recovery, nvidia, rust, other. Verdict mix + top actors/types per domain. |
| `verdicts` | Verdict distribution overall, by entry type, by domain. Trinary: proven/disproven/unknown. |
| `contradictions` | Disproven entries + corrections + failures/incidents, with refutes/cites edges. |

## Schema normalization

The ledger JSONL is loose. The parser normalizes:
- content: `summary`+`detail` | `text` | `action`+`detail` | `event`+`summary`
- type: `type` | `kind`, lowercased (`DECISION` → `decision`)
- verdict: `proven`/`done`/`done-standalone`/`PROVEN` → proven; `disproven`/`refuted` → disproven; missing/`mixed`/`?` → unknown
- `seq=NNN` / `seq NNN` mentions extracted from refs + body + title → graph edges

## Benchmarks (2026-10-07, release build)

Real ledger (578 entries): parse+index **8 ms**; every view **<5 ms**.

Synthetic 100k: parse+index **1.1 s**; full pipeline (parse+index+5 views) **5.8 s** → **~17k entries/sec**.

View cost at 100k is dominated by JSON serialization of the materialized graph
(claims view emits ~19MB). Parse/index themselves scale linearly.

## Design notes

- Single streaming pass, 1MB bufreader, indexes built inline.
- Edge derivation is heuristic (refs + temporal + type-role), not ground truth —
  the ledger's only hard links are `refs[]` file paths and `seq=` mentions.
- `other` domain (149/578) = entries with no keyword hit; extend `DOMAINS` in
  `src/views.rs` as vocabulary grows.
