# RIGOR-AUDIT application — ledger-graph

**Repo:** `~/workspace/ledger-graph` (fast ledger parser + on-demand
graph views, Rust, 5/5 tests)
**Date:** 2026-10-10 ~15:15 PDT
**Audit:** `~/workspace/dogfood/audit/RIGOR-AUDIT.md` (R-1..R-12)
**Corpus state:** scaffolded 2026-10-07; read-only parser — it READS
`~/ledger/ledger-NNNN.jsonl` and materializes views. It owns no ledger.

## Per-finding disposition

### R-1 (instance-only fix-verification claims) — N/A
This repo makes no fix-verification claims. Its verdict view normalizes
loose ledger verdicts into a trinary display; it does not verify fixes.

### R-2 (honest guardrail bounds) — N/A (no guardrails; parser only)
No enforcement, detection, or sanitization logic exists. The one
judgment-adjacent surface is verdict normalization (below, R-11).

### R-3 (out-of-band head for hash-chained logs) — N/A
This repo owns no hash-chained log; it reads the operator's ledger
files, which live outside the repo. No OOB head created; nothing here
claims tamper-evidence. If the repo ever starts PINNING ledger heads
for integrity checks, an OOB head copy becomes mandatory (R-3).

### R-4 (backlog triage) — APPLIED
Standing issues: **none.** Clean tree, 5/5 tests. Wont-fix list: none
required. Backlog rule: future standing HIGHs get FIX with a complexity
budget or explicit WONT-FIX; the list must not grow unbounded.

### R-5 (signature/denylist games) — N/A
No signature/denylist/strip logic.

### R-6 (detector without consumer) — N/A
No detectors. The `contradictions` view surfaces disproven entries; it
is a read view, not a control, and does not claim to be one.

### R-7 (property vs change-detection tests) — APPLIED (policy)
The 5 tests assert parse/index/view contracts (e.g. entry counts,
view shapes). These are change-detection tests against the parser's own
behavior. Policy (new): parser tests = change-detection; if a
security-invariant property test is ever added (e.g. "a forged seq link
never silently indexes"), it must be labeled PROPERTY and its
expectation may never be rewritten without adversarial sign-off.

### R-8 (unsigned "provenance") — N/A
The views assert no provenance. `organelles/execution.json` under this
repo is a migration artifact (map-or-design record), not a provenance
claim.

### R-9 (frozen claim-log schema) — N/A (read-side note)
No claim log owned. Read-side limitation: the parser normalizes loose
ledger verdicts (`proven`/`done`/`done-standalone`/`PROVEN` → proven;
missing/`mixed`/`?` → unknown) — this is a display heuristic, and the
README documents it as normalization, not as the ledger's schema. If a
frozen enum schema is ever adopted, this mapping must be versioned with
it.

### R-10 (live record-keeping) — N/A
No scoreboard analog; views are computed on demand from the current
ledger file.

### R-11 (explicit UNKNOWN list)
1. Verdict normalization is a heuristic: `done`/`done-standalone` are
   mapped to `proven` by keyword, not by the ledger's (non-frozen)
   schema — the true verdict semantics are UNKNOWN where the ledger
   entry is ambiguous (see audit R-9 on the ledger itself).
2. `seq=` edge extraction from free text: temporal-proximity edges
   (≤15 entries, same chat) are inference, not citation — treat as
   UNKNOWN-confidence edges.
3. Domain classification is keyword-based — misclassification is
   UNKNOWN-bounded, not measured.
4. Benchmark numbers in README (2026-10-07, 8ms/1.1s) are measured for
   that ledger snapshot and machine — not guaranteed on other inputs.

### R-12 (no unsourced oral footnotes)
Swept README.md and BENCHMARK.md for "probably/likely/unverified/
I heard" class claims: **none found.** Benchmark numbers are dated and
scoped.

## Commits (this pass)
- (pending — AUDIT-APPLICATION.md only; never push)

## Tests
- `cargo test`: **5/5 green** (measured 2026-10-10).
