# ledger-graph — organelle mapping decisions

Pilot repo. Decomposition reviewed 2026-10-10 under the corrected doctrine
(decompose ACTUAL capabilities; map-or-design per unit, justified).

## Capability units found (from src/main.rs + module layout)

| Unit | Source | Organelle op | Decision |
|---|---|---|---|
| Parse ledger JSONL → index stats | `ledger.rs` (Ledger::load, 99 lines) + `entry.rs` (Entry/Verdict normalization, 175 lines) | `parse` | **Bespoke** (`lg`). Justification: ledger-specific schema normalization (trinary verdict mapping, seq/chat/actor/ref indexing) — not generic JSON validation. `ref-json-validate` only checks schema conformance; this builds a queryable index. No ref organelle fits. |
| Materialize graph views (claims/actors/domains/verdicts/contradictions) | `views.rs` (462 lines, pure functions over `Ledger`) | `view` | **Bespoke** (`lg`). Justification: domain-specific graph projections over the ledger index. No ref organelle computes graph views. |
| Benchmark (real + synthetic 100k) | `bench.rs` (104 lines) | — (excluded) | **Deliberate exclusion.** `bench` is a developer measurement harness, not a capability consumers invoke. Exposing it as an organelle op would grant no meaningful contract (its "output" is timing noise). Recorded here so the exclusion is visible, not an oversight. |

## Why one organelle, not three

`parse` and `view` share one indivisible core: `Ledger::load` (parse +
normalize + index). Splitting them into separate organelles would duplicate
the index build or force an artificial shared-state organelle. One organelle
(`lg`), two ops, both ReadOnly/Proven — the decomposition follows the
code's actual seam (index vs. project), not an arbitrary file split.

## Reference-organelle check (all 12 considered, none adopted)

- `ref-json-validate`: validates against a JSON schema; we *normalize and
  index* a loose schema. Different computation.
- `ref-ledger-append`: appends entries; we only read. Opposite direction.
- `ref-text-transform`, `ref-hash-seal`, `ref-kv-store`, others: no overlap.
- Verdict: genuinely bespoke. Nothing reinvented that already existed.

## Execution facts

- ReadOnly, Proven (deterministic parse; views are pure functions of input).
- fs: read-only, per-call path allowlist (default `~/ledger/ledger-0001.jsonl`).
- No network, no writes, no secrets. Stateless: suspend = no-op, kill = drop.
- Envelope: 1 cpu / 512 MB / 60 s wall-time.
- Receipts: content-addressed sha256, chained; `verify_chain` green in tests.
