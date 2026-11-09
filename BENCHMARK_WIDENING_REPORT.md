# Widening to hundreds of cases: verdict and staged plan

| Question | Answer |
|---|---|
| Widen at all? | YES — but via standard suites, not ad-hoc queries |
| Hundreds at once? | NO — staged; Phase 1 ≈ 42 queries, Phase 2 only on open questions |
| Why suites, not invented? | Comparable with published numbers; peer-reviewed shapes; no benchmarketing |

## Standard suites per database type

| Suite | Count | Dataset | Measures | Our mapping |
|---|---|---|---|---|
| TPC-H | 22 + 2 refresh | 8 tables, SF1–100T | QphH (power + throughput) | DuckDB/PG/MariaDB/SQLite/shelf (we run ~3 shapes now) |
| TPC-DS | 99 templates | 24 tables, skewed | QphDS (window, cube, mining) | Phase 2 only |
| TPC-C | 5 txns, 9 tables | Warehouses (100 MB/W) | tpmC New-Order rate | OLTP writes (shelf WAL/epoch side) |
| YCSB | 6 workloads A–F | 1 KB records, 10M–1B | ops/s + p50/p99 per op | Redis/Mongo/Scylla/shelf-KV |
| LDBC SNB | 14 complex + 7 short + 8 insert; BI 20; algo 6–8 | Datagen social graph SF0.1–10T | ops/s, p95, power | Neo4j/shelf-graph |
| JOB | 113 in 33 families | IMDB real dump ~3.6 GB | Runtime + plan quality | Optimizer stress (fiber side) |
| Graphalytics/500 | 6 / 2 kernels | Kronecker/ Datagen to 1B edges | EVPS, GTEPS | Traversal scale-out |
| TSBS / ANN | 3 use-cases / recall curve | IoT / SIFT1M–Deep1B | Ingest + recall-vs-QPS | Out of scope (no time-series/vector engine here) |

## Staged plan (counts and cost)

| Phase | Queries | New datasets | Est. wall | Est. harness work |
|---|---|---|---|---|
| 1a TPC-H full 22 | +19 (have SF1 + 3) | None (have it) | 3–5 h (SQLite/Maria joins dominate) | 1 TPC-H runner script |
| 1b YCSB A–F | 6 workloads | None (synthetic) | ~1 h × 3 engines + shelf | YCSB tool (Java) + shelf-KV shape |
| 1c LDBC interactive 14 | 14 complex reads | Datagen SF1 (~1 h gen) | ~2 h (gen + import + run) | Datagen + Neo4j import + shelf-graph shapes |
| Phase 1 total | ~42 | 1 generated | ~8 h wall | ~1 week |
| 2a JOB 113 | 113 | IMDB 3.6 GB (real) | 2–4 h (PG-slow) | Optimizer harness + fiber-side shapes |
| 2b TPC-DS subset | ~20 hardest | TPC-DS SF1 | ~3 h | Runner extension |
| 2c LDBC BI 20 | 20 | Reuse Datagen | ~2 h | BI shapes |

## Why not hundreds immediately

| Reason | Math |
|---|---|
| Wall time | 113 JOB × slow engines ≈ days; interpretation ≈ weeks |
| Same facts re-proven | 100 join variants re-prove one fiber result; marginal info per query → 0 after ~20 |
| Harness cost | Each suite needs loader + shelf counterpart + merge schema (~days each) |
| Load discipline | Hundreds of timings need quiet-box scheduling; median-of-N multiplies wall ×N |
| Standard beats volume | One TPC-H number talks to the industry; 200 ad-hoc numbers talk to nobody |

## Real datasets (new, recommended)

| Dataset | Size | Serves | Status |
|---|---|---|---|
| IMDB dump (JOB) | ~3.6 GB real | Optimizer stress, 4–17 joins | Phase 2a |
| LDBC Datagen SF1–10 | Generated, realistic skew | Graph complex queries | Phase 1c |
| IEEE fraud (have) | 590k rows real | GROUP BY / risk filters | Have it |
| Facebook edges (have) | 88k real | 2-hop / triangles | Have it |

## Bottom line

```
Phase 1 (do)   : TPC-H 22 + YCSB 6 + LDBC-14 = ~42 queries, ~8 h wall, ~1 week work
Phase 2 (iff)  : JOB + TPC-DS-subset + LDBC-BI, only if Phase 1 leaves open questions
Never          : hundreds of ad-hoc queries (cost without comparability)
Each suite     : loader + shelf counterpart + runner merge (same pattern as run_all.py)
Gate           : every new query needs exactness gate + published-number cross-check
```
