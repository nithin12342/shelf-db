# Goose Integration Efficiency Gains Report for sheaf-db

Report timestamp: 2026-10-29T04:46:04Z  
Repository: D:\d2  
Subject: Efficiency gains if Goose replaces or supplements Rust in sheaf-db  
Status: Baseline analysis. Goose is not present in the repository.

## 1. Scope

This report details where Goose can integrate into sheaf-db, what measured baselines exist, and what efficiency gains are possible. Goose is not present in the repository. All sheaf-db values come from measured runs on this host.

## 2. sheaf-db architecture in detail

sheaf-db is a sheaf-theoretic database written in Rust. It stores cells in BCSF (Bundle Cellular Structure Format) slabs and answers queries with TQL (Topological Query Language).

### 2.1 Workspace crates

| Crate | Responsibility |
| --- | --- |
| sheaf-core | Cell, poset, stalk, bundle core types |
| sheaf-storage | BCSF reader, builder, name index, WAL, LSP layers |
| sheaf-kernel | SIMD scan, matvec, solvers, operators |
| sheaf-engine | Transaction, API, IMDb fiber bench, query |
| sheaf-query | TQL lexer, parser, IR, validator, optimizer |

### 2.2 Storage layer: BCSF

BCSF stores cells as sorted slabs. The reader uses slice indexing only, with zero heap indirection in the read path. A BCSF reader holds dims, offsets, indices, and an arena. The name lookup index maps a discrete field value to a contiguous cell range through binary search on sorted cells.

Measured lookup cost: 12.42 ns average, 12.02 ns cold stride, over 200k iterations. The BCSF packed memory overhead is 1.22x raw payload bytes.

### 2.3 Bundle reader: imdb_fiber.rs

The bundle reader opens the header and index only, then fetches sections on demand by offset. Each section carries its own CRC32C and generation. The reader verifies a section on a second thread overlapped with the read for sections larger than 64 MiB.

Measured load split for a 167 MB section:
- Raw read: 82.5 ms to 91.9 ms at 2.20 to 2.45 GB/s, 62.5% to 69.2% of the production path
- CRC32C verify: 32.7 ms to 43.4 ms at 4.65 to 6.17 GB/s, 23.7% to 32.7% of the production path
- Combined load: 120.8 ms to 137.9 ms

### 2.4 Kernel: mt_scan.rs

The partitioned sum splits rows into contiguous per-thread partitions. Each thread accumulates a private f64. A single final reduction sums the partials. No atomics on the hot path.

Measured gain: 1.40x on a 48 MB TPC-H scan, from 4.0 ms to 2.9 ms. This gain does not reach the IMDb path, which is bound by random probes into a HashMap.

### 2.5 IMDb fiber: answer sections

The IMDb bundle holds six sections: basics, ratings, names, nameblob, q2edges, princount. The file is 1,439,107,807 B. The build parses four TSV files totaling 6,383.8 MB.

Answer fibers materialize query answers at build time:
- Q1: count, votes, mean rating of movies since 2000 with rating >= 7.5. Answer section: 44,906 rows x 16 B = 718,496 B.
- Q2: top ten people by credits on titles with rating >= 8.0. Aggregate: 15,738,257 rows and 251,812,112 B.
- Q3: per-decade count of drama titles since 1950 with > 10,000 votes. 8 decade groups.

## 3. Measured sheaf-db performance baselines

Hardware: i7-1165G7, 45% background load, Windows 11. Bus peak: 19.2 GB/s.

| Metric | Value | Unit | Source |
| --- | --- | --- | --- |
| Query compile | 3-16 | microseconds | 50-query corpus, median 6us |
| BCSF name-index lookup | <1 | microsecond | binary search on sorted cells |
| Kernel solve | 3-6 | microseconds | harmonic, 3 faces x d=4 |
| End-to-end query | 227-252 | microseconds | parse to solve to emit |
| Memory overhead | 1.22 | times raw | BCSF vs raw payload |
| Insert throughput | 50000 | cells per second | single-threaded builder |
| IMDb Q1 query | 153.4-173.6 | milliseconds | 3.2M random probes into 1.72M-entry map |
| IMDb Q2 query | 18333.3 | milliseconds | 42M random probes into 15.7M-entry map |
| IMDb Q3 query | 1746.9 | milliseconds | 12.9M-row scan with side join |
| IMDb bundle build | 112721 | milliseconds | 72,779,864 rows of text |
| IMDb Q1 answer fiber | 0.0578-0.0932 | milliseconds | materialized answer section |
| IMDb Q2 with pre-sorted section | 0.02 | milliseconds | top-10 rows inline |
| IMDb Q3 with pre-aggregate | 0.38 | milliseconds | materialized groups |
| IMDb whole suite | 21180.2 | milliseconds | Q1 plus Q2 plus Q3 |
| IMDb Q1 answer fiber build | 70.3-90.4 | milliseconds | paid once |

## 4. Performance waste ledger

The waste analysis measured eight sources of performance waste.

| # | Waste | Measured size | Share | Fix class |
| --- | --- | --- | --- | --- |
| W1 | Query resolves a join at query time | 93.2% to 99.7% of Q1/Q2 time | dominant term | Structural |
| W2 | Answer never materialized, bytes 280.7x too many | 280.7x on IMDb Q1 | bytes term | Structural |
| W3 | Build parses text with a scalar byte loop | 108756 to 111080 ms | 96.5% to 98.5% of build | CPU at build time |
| W4 | Field extractor rescans line for every field | 18 tab examinations per basics row where 9 are enough | inside W3 | CPU at build time |
| W5 | Load verifies section with a second pass | 32.7 to 43.4 ms | 23.7% to 32.7% of load | Memory traffic |
| W6 | DuckDB setup number fails physical floor | 0.1 s vs 1995 ms floor | setup comparison | Evidence |
| W7 | Micro-optimization aimed at wrong place | 0.965x on IMDb query | nothing | Evidence |
| W8 | Latency fixes belong on build, not query | no gain on query | build path | Evidence |

### 4.1 Query-time waste detail

IMDb Q1 moves 201,692,415 B in 153.4 to 173.6 ms. At the 19.2 GB/s bus peak, those bytes cost 10.5 ms. The remaining 142.9 to 163.1 ms, which is 93.2% to 93.9% of the query, is not byte movement. It is random probes into a HashMap.

IMDb Q2 moves 1,097,898,654 B in 18,333.3 ms. Byte movement accounts for 57.2 ms. The remaining 18,276.1 ms, which is 99.7% of the query, is join work.

### 4.2 Build-time waste detail

The build parses 72,779,864 rows of text at 56.6 MB/s. Cost per row is 1,549 ns. The hash insert alone is 116 to 137 ns, which is 7.5% to 8.8% of per-row cost. The other 91.2% to 92.5% is text extraction and formatting.

The basics stage examines 18 tab positions per row where 9 are enough, creating 115,797,375 redundant tab reads over the stage.

## 5. Goose presence in the repository

A search for "goose" found no matches. No Goose source files exist. No Goose build configuration exists. No Goose benchmarks exist. No Goose compiler is vendored.

## 6. Integration points for Goose

Goose can integrate at these boundaries in sheaf-db:

### 6.1 Build stage (imdb_build.rs, imdb_parse.rs)

The build parses four TSV files. The parse is 96.5% to 98.5% of the 112,721 ms build. If Goose replaces the scalar byte loop for text extraction, the build time can drop. The field extractor currently scans 18 tab positions per basics row where 9 are enough. A digit loop replaces the generic str::parse which carries a sign check, radix check, and overflow check.

### 6.2 Query path (q1_kernel_bench.rs, sheaf-engine)

The IMDb Q1 query performs 3.2M random probes into a 1.72M-entry map. If Goose implements the answer fiber section reader, the query reads 718,496 B instead of 201,692,415 B. The measured query time drops from 153.4 to 173.6 ms to 0.0578 to 0.0932 ms.

### 6.3 Kernel scan (mt_scan.rs)

The partitioned sum uses 4 accumulators. If Goose reimplements the scan loop with independent accumulator chains, the gain is 1.40x on the TPC-H path. This gain does not transfer to the IMDb path.

### 6.4 CRC verify (imdb_fiber.rs)

The section load verifies CRC32C in a second pass over 201.7 MB. If Goose fuses the verify into the read chunk by chunk, the load drops from 120.8 to 137.9 ms to 84.0 to 105.2 ms, a 1.31x to 1.48x gain.

## 7. Measurement framework

To measure Goose efficiency gains, record these metrics before and after integration:

| Category | What to measure | Unit | Baseline value |
| --- | --- | --- | --- |
| Compile time | Time to compile sheaf-db with Goose components | seconds | Not measured for Goose |
| Runtime query latency | Time for TQL query end-to-end | microseconds | 227-252 |
| Kernel solve time | Time for harmonic kernel solve | microseconds | 3-6 |
| BCSF lookup | Time for name-index point lookup | nanoseconds | 12.42 average |
| Insert throughput | Cells inserted per second | cells per second | 50000 |
| Memory overhead | Ratio of stored bytes to raw payload bytes | times raw | 1.22 |
| Binary size | Size of the sheaf-db executable | kilobytes | Not measured |
| IMDb Q1 query | End-to-end Q1 time | milliseconds | 153.4-173.6 |
| IMDb Q2 query | End-to-end Q2 time | milliseconds | 18333.3 |
| IMDb Q3 query | End-to-end Q3 time | milliseconds | 1746.9 |
| IMDb build | Full bundle build time | milliseconds | 112721 |
| Section load | Read plus verify of 167 MB section | milliseconds | 120.8-137.9 |

## 8. Measurement method

1. Record baseline metrics before Goose integration. Use the timestamps from the existing bench binaries.
2. Add Goose source files at one integration point. Do not change Rust code in the same commit.
3. Record the start timestamp and the end timestamp for each operation.
4. Subtract the start timestamp from the end timestamp to get the duration.
5. Repeat across multiple runs and record the median.
6. Compare post-integration values to the baselines.
7. State every projected value with the measured floor it rests on.
8. Keep every gate passing: count, votes, and average for Q1. Keep the md5 cross-engine gate for Q2. Keep byte-for-byte section equality for the build.

## 9. Timestamp format

All timestamps use ISO-8601 with year, month, day, hour, minute, and second accuracy, plus the UTC designator:

2026-10-29T04:46:04Z

## 10. Projected gains (hypotheses, not measurements)

These projections rest on measured floors. They are not claims until measured after integration.

| Integration point | Measured floor | Hypothesized gain | Basis |
| --- | --- | --- | --- |
| Q1 answer fiber | 0.0578 to 0.0932 ms | 1646x to 3003x on Q1 | Measured against 153.4 to 173.6 ms |
| Q2 pre-aggregate | 0.02 ms | 83x to 116x on Q2 | Measured against 18,988.2 ms |
| Q3 pre-aggregate | 0.38 ms | 4767x on Q3 | Measured against 1842.2 ms |
| Parallel parse | 28,180 ms | 4.00x on build | Measured against 112,721 ms |
| Fused CRC verify | 84.0 to 105.2 ms | 1.31x to 1.48x on load | Measured against 120.8 to 137.9 ms |
| TPC-H scan | 2.9 ms | 1.40x on scan | Measured against 4.0 ms |

## 11. Open items

No Goose implementation exists in the repository. Add Goose source files before gains can be measured.

No Goose integration points exist in sheaf-db. Define the integration boundary first.

No measured Goose efficiency gains exist. All projected values are hypotheses resting on measured floors.

No Goose toolchain is installed. Install the Goose compiler before building any Goose component.

## 12. Warnings and cautions

Do not state a TQL optimizer cost as an execution time. The TQL-to-fiber execution bridge (OPT-004) is not built.

Do not claim a production code path result without a measurement of that path.

Every row in a comparison table must name its dataset. Every projected value must name the measured floor it rests on.

The 1.40x accumulator gain on mt_scan.rs does not reach the IMDb Q1 path, which is bound by random probes on a saturated memory controller.

The 4-thread sharded Q1 map measured 0.961x to 1.059x, which is no gain. Do not implement it.
