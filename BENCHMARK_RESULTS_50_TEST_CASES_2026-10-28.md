# BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md: Execution With Pre-Folded Topological Fiber Bundle

## 1. Execution Telemetry and Timestamps

- **Document Date:** 2026-10-28
- **Run Start Time:** 2026-10-28 17:54:46 IST (UTC+05:30)
- **Run End Time:** 2026-10-28 17:55:13 IST (UTC+05:30)
- **Total Elapsed Duration:** 26.72 seconds
- **Host Hardware:** Intel Core i7-1165G7 @ 2.80GHz, 15.8 GB RAM, PCIe 4.0 NVMe Solid State Drive
- **Operating System:** Windows 11 Version 10.0.26100
- **Runner Command:** `py run_cli_manual_suite.py`
- **Output Record File:** `results/MANUAL_CLI_EXECUTION_20261027.json`
- **Active Subsystem:** Pre-folded topological fiber bundle structure in [`sheaf_db_app.rs`](file:///d:/d2/sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs) and [`fiber_join_bench.rs`](file:///d:/d2/sheaf-db/crates/sheaf-engine/src/bin/fiber_join_bench.rs)

---

## 2. Five Problem Workloads Results

The table below answers how the pre-folded topological fiber bundle performed against competitors on the five problem workloads.
Each row represents one problem workload. Each column represents the competitor name, competitor latency, ShelfDB latency, and the outcome.

| Workload Name | Best Competitor Engine | Competitor Latency (us or ms) | ShelfDB Latency (us or ms) | Outcome Status |
|---|---|---|---|---|
| Reference Sync Commit | SQLite PRAGMA synchronous FULL | 4,719.30 us | 6.90 us (Disk WAL) / 0.10 us (Ring WAL) | Win |
| Problem 1: Durable Single-Row Write | SQLite Single-Row WAL | 1.83 us | 0.10 us (Ring Delta WAL) | Win |
| Problem 2: Relational Join (TPC-H 6M) | DuckDB Hash Join | 158.30 ms | 8.70 ms (Pre-Folded Fiber Bundle) | Win (18.2x faster) |
| Problem 3: Analytical Column Scan | DuckDB Vectorized Scan | 5.60 ms | 3.80 ms (4-Accumulator Fused Scan) | Win |
| Problem 4: Cold Full Ingestion (IMDb) | SQLite Table Ingestion | 1.91 s | 112.72 s (Full Poset Build, 0.009 ms Warm) | Loss |
| Problem 5: 1D Time-Series Slice Filter | Kdb+ q In-Process Slice | 10.00 us | 3.00 us (Dense Pointer Slice) | Win |

---

## 3. All 50 Test Cases: Measured Execution With Fiber Bundle

The table below answers the measured latency, matched row count, and outcome across all 50 test cases using the pre-folded topological fiber bundle.
Each row represents one test case. Each column represents the test identifier, test name, matched row count, ShelfDB measured time, best competitor name, best competitor time, and the outcome.

| Test Case Identifier | Test Case Name | Matched Rows | ShelfDB Measured Time (us) | Best Competitor Name | Best Competitor Time (us) | Outcome Status |
|---|---|---|---|---|---|---|
| TC-01 | Strict High-Value Risk Equality | 1 | 25 | SQLite | 2,067.3 | Win |
| TC-02 | Single Point Exact Account | 1 | 15 | MariaDB | 1,875.3 | Win |
| TC-03 | Ultra-High Outlier Amount | 0 | 935 | SQLite | 411.7 | Loss |
| TC-04 | Zero-Match Null Filtering | 0 | 631 | SQLite | 491.2 | Loss |
| TC-05 | Compound Balance Disjunction | 3 | 11 | SQLite | 1,009.4 | Win |
| TC-06 | Rooted Merchant Attack Pattern | 0 | 718 | SQLite | 379.5 | Loss |
| TC-07 | Regional European Filtering | 2 | 15 | SQLite | 1,364.2 | Win |
| TC-08 | Category String Exact Match | 2 | 22 | Redis | 1,710.1 | Win |
| TC-09 | Negative Risk Exclusion | 1 | 26 | SQLite | 650.8 | Win |
| TC-10 | Multi-Variable Range & Inequality | 2 | 14 | SQLite | 508.4 | Win |
| TC-11 | Regularization Lambda Scaling 0.0001 | 2 | 9 | SQLite | 477.2 | Win |
| TC-12 | Regularization Lambda Scaling 0.0005 | 2 | 56 | SQLite | 379.3 | Win |
| TC-13 | Regularization Lambda Scaling 0.001 | 2 | 15 | SQLite | 341.2 | Win |
| TC-14 | Regularization Lambda Scaling 0.005 | 2 | 14 | SQLite | 769.0 | Win |
| TC-15 | Regularization Lambda Scaling 0.02 | 2 | 18 | SQLite | 318.9 | Win |
| TC-16 | Regularization Lambda Scaling 0.05 | 2 | 10 | SQLite | 404.5 | Win |
| TC-17 | Regularization Lambda Scaling 0.1 | 2 | 14 | SQLite | 370.4 | Win |
| TC-18 | Regularization Lambda Scaling 0.25 | 2 | 14 | SQLite | 427.3 | Win |
| TC-19 | Regularization Lambda Scaling 0.5 | 2 | 15 | SQLite | 428.6 | Win |
| TC-20 | Regularization Lambda Scaling 1.0 | 2 | 14 | SQLite | 368.8 | Win |
| TC-21 | Iteration Bound Scaling 1 Iter | 3 | 11 | SQLite | 1,038.2 | Win |
| TC-22 | Iteration Bound Scaling 2 Iters | 3 | 5 | SQLite | 267.2 | Win |
| TC-23 | Iteration Bound Scaling 3 Iters | 3 | 9 | SQLite | 423.8 | Win |
| TC-24 | Iteration Bound Scaling 5 Iters | 3 | 11 | SQLite | 394.4 | Win |
| TC-25 | Iteration Bound Scaling 8 Iters | 3 | 11 | SQLite | 290.8 | Win |
| TC-26 | Iteration Bound Scaling 12 Iters | 3 | 12 | SQLite | 290.5 | Win |
| TC-27 | Iteration Bound Scaling 18 Iters | 3 | 8 | SQLite | 260.6 | Win |
| TC-28 | Iteration Bound Scaling 35 Iters | 3 | 11 | SQLite | 258.0 | Win |
| TC-29 | Iteration Bound Scaling 50 Iters | 3 | 8 | SQLite | 265.0 | Win |
| TC-30 | Iteration Bound Scaling 100 Iters | 3 | 11 | SQLite | 333.6 | Win |
| TC-31 | Energy Threshold Certificate 1e-06 | 3 | 11 | SQLite | 1,797.0 | Win |
| TC-32 | Energy Threshold Certificate 5e-06 | 3 | 12 | SQLite | 359.1 | Win |
| TC-33 | Energy Threshold Certificate 1e-05 | 3 | 11 | SQLite | 279.3 | Win |
| TC-34 | Energy Threshold Certificate 5e-05 | 3 | 14 | SQLite | 351.9 | Win |
| TC-35 | Energy Threshold Certificate 0.0001 | 3 | 11 | SQLite | 496.2 | Win |
| TC-36 | Energy Threshold Certificate 0.0005 | 3 | 8 | SQLite | 462.1 | Win |
| TC-37 | Energy Threshold Certificate 0.001 | 3 | 11 | SQLite | 731.3 | Win |
| TC-38 | Energy Threshold Certificate 0.005 | 3 | 17 | SQLite | 262.9 | Win |
| TC-39 | Energy Threshold Certificate 0.01 | 3 | 11 | SQLite | 293.5 | Win |
| TC-40 | Energy Threshold Certificate 0.05 | 3 | 15 | SQLite | 311.4 | Win |
| TC-41 | Discrete Only Projection | 2 | 15 | SQLite | 463.2 | Win |
| TC-42 | Spectral Certification Only | 3 | 11 | SQLite | 466.9 | Win |
| TC-43 | Cross-Border APAC Region Flow | 0 | 451 | SQLite | 745.5 | Win |
| TC-44 | Low Balance High Transaction Anomaly | 1 | 25 | SQLite | 645.9 | Win |
| TC-45 | High Balance Wealth Verification | 1 | 27 | Redis | 1,519.2 | Win |
| TC-46 | Gaming Merchant Micro-Payments | 0 | 709 | SQLite | 410.8 | Loss |
| TC-47 | Luxury Low-Risk Channel | 0 | 747 | SQLite | 525.0 | Loss |
| TC-48 | All European Rooted Channels | 2 | 15 | SQLite | 332.4 | Win |
| TC-49 | Maximum Solver Tightness Stress | 2 | 16 | SQLite | 581.0 | Win |
| TC-50 | Universal Boundary Full Scan | 5 | 8 | SQLite | 684.5 | Win |

---

## 4. Overall Win Rate Summary

- **Total Suite Duration:** 26.72 seconds (improved from 33.53 seconds).
- **Matching Query Cases (44 cases):** 44 Wins, 0 Losses (100.0% Win Rate, 5 &mu;s to 56 &mu;s).
- **Relational Join With Pre-Folded Bundle (Problem 2):** 8.70 ms query (Beats DuckDB 158.30 ms by 18.2x).
- **Total High-Variance Test Cases:** 45 Wins, 5 Losses (90.0% Win Rate).
