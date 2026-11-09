# Conversation record: theoretical-maximum proposal (2026-10-14)

## Request
Propose solutions to the measured bottlenecks for high performance against the 7 benchmarked databases — "the most efficient way to store, find, manipulate, group, analyze, and represent relationships between data, theoretical maximum."

## Proposal given (per operation)
- STORE: columnar + validity-free + tiered compression (hot at bandwidth floor, cold 3–5× smaller).
- FIND: MPHF/branchless point index (~3–5 ns; diminishing returns, lowest priority).
- MANIPULATE: group commit + append-only epoch headers (~1 µs durable class, ~100 ns amortized); fsync floor disclosed.
- GROUP: incremental pre-aggregation at ingest — GROUP BY becomes O(#groups); dissolves the query class (biggest structural payoff).
- ANALYSIS: no proposal — BJ-PCG already optimal-ish (M4 negative stands); defending current design is the position.
- RELATIONSHIPS: fibers + bitmask compression (space win; speed neutral-to-positive).
- SCAN: multi-threaded partitioned aggregation, predicted 3–4 ms vs DuckDB 6.4 ms (only lever physics allows).

## Follow-up requests in the same exchange
1. Append the program to CONTEXT (§10), SPEC (REQ/SPEC-030..035), ARCHITECTURE (planned rows + A7), SKELETON (METHOD-024..029 stubs, FILE-060/061/062, status 22/29) — all applied.
2. Separate task plan in markdown here (this directory): `MAX_PERF_PLAN.md` — written.
3. This conversation record: `CONVERSATION_MAXPERF.md` (this file).

## Open decisions for the user
- Build order recommended: P1 → P3 → P4, then P2/P6, then P5. Awaiting go-ahead (P1+P3 first was suggested).
- W1 caps P1 expectations (~4–6 ms realistic, not sub-ms); recorded in plan so a miss lands as documented physics, not failure.
