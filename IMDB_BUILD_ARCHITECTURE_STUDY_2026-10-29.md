# IMDb Cold-Build Architecture Study — Violations and Efficiency Gains

**Date:** 2026-10-29
**Repository:** `D:\d2\sheaf-db`
**Code studied:** `crates/sheaf-engine/src/bin/shared/imdb_build.rs`, `imdb_parse.rs`, `imdb_answers.rs`, `imdb_fiber.rs`, `imdb_q4.rs` (NOT markdown files)

---

## 1. The claim, checked against the code

> "The initial ingestion of 72 million IMDb records still requires 112.72 seconds."

**Not accurate for the code path that exists.** The 112.72 s figure in `SHELFDB_WRITE_PATH_FUNDAMENTAL_ARCHITECTURE_AND_OPTIMIZATION.md` refers to a *graded-poset cold ingest with inline δ²=0 verification + BCSF emission* (`real_load_bench.rs` / `imdb_fiber_bench.rs#L100`). The code actually in the repo builds pre-computed *answer fibers* (`imdb_fiber_bench`), which is a different, narrower path.

Measured against the real dataset present in the repo (`D:\d2\data\imdb/`, ~7 GB, 132M rows across 4 parsed TSVs):

| Stage | Rows | Best measured (warm cache) | Worst measured (cold cache) |
|-------|------|---------------------------|-----------------------------|
| names | 15.7M | ~5.4s | ~7.8s |
| ratings | 1.7M | ~0.25s | ~0.7s |
| basics | 12.9M | ~2.5s | ~7.0s |
| principals | 11.6M titles / 42.5M edges | ~15s | ~27s |
| answers (q1/q2/q3) | — | ~1.4s | ~3.6s |
| **Cold bundle build** | — | **~31s** | **~54s** |
| **Re-run (bundle fresh)** | — | **~0ms build** | **~0ms build** |

The `112.72 s` figure does not match this code path; it belongs to the full graded-poset ingest with face verification, which is a separate, deeper pipeline. The answer-fiber build is ~30–54s cold and **0ms on re-run** because the bundle is cached and fingerprint-validated.

---

## 2. Violations found against the working architecture

The parse module (`imdb_parse.rs`) documents three rules it claims to follow. One was violated:

| Rule (from imdb_parse.rs doc) | Status | Location |
|-------------------------------|--------|----------|
| Field extractor scans a line ONCE | OK | `FieldScratch::scan` |
| Numeric fields read by a byte loop, not `str::parse` | **VIOLATED** | `parse_ratings_range` used `str::from_utf8(...).parse()` for the f32 avg rating — carries UTF-8 validation, sign/radix/overflow checks per row |
| Parse runs over several byte ranges at once | OK | `stream_stage` + range splitting |

**Fix:** Added `parse_f32()` (pure byte loop, no UTF-8/sign/radix/overflow logic) and routed the rating field through it instead of `str::parse`. Ratings stage dropped from 681 ms → ~250 ms.

---

## 3. Efficiency gains implemented (all verified, no regression)

### Gain A — `parse_f32` byte loop (imdb_parse.rs)
Replaced `std::str::from_utf8(field).unwrap_or("0").parse().unwrap_or(0.0)` with a dedicated ASCII decimal byte loop. Removes per-row UTF-8 validation and generic parse overhead.

### Gain B — Principals stream-count run-length encoding (imdb_parse.rs + imdb_build.rs)
`title.principals.tsv` is title-sorted (verified: `tt0000001` rows adjacent). Replaced per-thread `HashMap<u64, u32>` (42M hash inserts + lookups + a global `sort_unstable` of ~11M entries) with run-length counting of consecutive equal titles inside each byte range, then one sort + adjacent-duplicate sum. Principals parse CPU dropped from ~84s → ~75s (4-thread summed).

### Gain C — SIMD field scanning in all hot parse paths (imdb_parse.rs)
`scan_simd()` and `scan_avx2()` were implemented and proven byte-for-byte equal to scalar `scan` (covered by `scan_agrees` test) but **unused**. All four hot parse functions (`parse_names_range`, `parse_ratings_range`, `parse_basics_range`, `parse_principals_range`) now call `scan_simd()` instead of scalar `scan()`.

### Gain D — Independent stages stay sequential by design (imdb_build.rs)
Attempted to parallelize names/ratings/basics on worker threads. **Reverted:** on this 4-core/8-thread host each stage already saturates all cores with 4 workers; running three 4-thread stages concurrently oversubscribes the CPU (12 threads on 8 logical cores) and slowed the build from ~31s → ~53s. Each stage runs sequentially, which is correct here.

---

## 4. "Reduce it to milliseconds?" — the architectural answer

**Cold ingest of 7 GB cannot be milliseconds.** It is bounded by disk read (7 GB) + CPU parsing (132M rows). The measured cold build is ~30–54s. No code change makes parsing 7 GB sub-millisecond.

**Milliseconds is already achieved for queries.** The architecture builds the answer fibers **once**, then caches them with source fingerprints. On re-run:

```
bundle present and fresh, skipping build
q1: query_ms=0.0001   q2: query_ms=0.0025   q3: query_ms=0.0008
```

So the "milliseconds" target is the *query* path, served from cached sections, not cold ingest.

### Remaining architectural gains to reduce cold build further (not yet implemented)

1. **Memory-map the TSVs** instead of `read` + copy into 128 MiB blocks (`stream_stage` allocates a fresh 128 MiB `Vec` per block). On Windows this means `CreateFileMapping`/`MapViewOfFile`; avoids the read syscall and buffer copy, and lets the page cache serve data directly to the parser.
2. **Row-level delta invalidation.** Section reuse is currently all-or-nothing per file (fingerprint = size+mtime). If only `title.ratings.tsv` changes, the whole ratings stage re-parses. Row-level delta tracking would rebuild only changed row ranges.
3. **Overlap I/O with parse across stages.** Currently stages are sequential; on cold disk stage N's read waits for stage N-1's parse. Memory-mapping + stage pipelining would overlap these.

None of these change correctness; all gate on the existing fingerprint/section-reuse contract so re-runs remain instant.

---

## 5. Verification evidence

- `cargo test --workspace`: all pass, 0 failures (sheaf_storage 6 incl. `ring_wal_round_trip`, sheaf_query 4, sheaf_kernel 1, sheaf-engine imdb 5).
- `cargo check --workspace`: clean, no warnings.
- `imdb_fiber_bench` release: all gates PASS — `gate_answer_q1_matches_scan`, `gate_answer_q2_matches_scan`, `gate_answer_q3_matches_scan`, `gate_q1_exact`, `gate_q2_fast`, `gate_q3_fast`, `gate_q4_exact`, `gate_bundle_sections_fresh`.
- Query results match DuckDB/PostgreSQL baseline exactly (q1 count=44906 votes=294435444 avg=8.145506; q2 top names; q4 matches DuckDB).
