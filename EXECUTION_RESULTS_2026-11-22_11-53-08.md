# SQL to TQL execution results

**Timestamp:** 2026-11-22_11-53-08, local time 11:53:08 +05:30.
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `a4328dc5890d93cf97a89c26c46a6ecfe33b82d2`.
**Data set:** deterministic synthetic schema with 100 customers, 400 orders, and 800 lineitem rows.

## 1. Result

All 12 SQL queries returned rows. All 12 rendered TQL programs returned the same row checksum as the SQL query. The optimized plan and the baseline plan agreed on 11 of 12 queries. Query Q11 exceeded the 100,000 candidate limit on the nested-loop baseline. The fiber bundle lifecycle gate passed.

## 2. Method

1. The generator wrote schema and load files with seed `0x2545_f491_4f6c_dd1d`.
2. The application loaded the three tables through `sql-file`.
3. The runner executed each SQL query and captured the `tql` field from the result.
4. The runner wrote that text as a TQL program and executed it through `tql-file`.
5. The runner compared SHA-256 checksums of the canonical row arrays.
6. The runner repeated the SQL query with `--baseline` and compared the plan and the checksum.
7. The fiber lifecycle binary ran its own 10,000 operation gate against fiber bundles.

Table 1 answers this question: what each query measured.
Rows are queries. Columns are the row count, the two parity results, the optimized time, the baseline time, and the skipped page count.

| Query | Rows (count) | SQL and TQL parity | Optimized and baseline parity | Optimized time (microseconds) | Baseline time (microseconds) | Pages skipped (count) |
|---|---|---|---|---|---|---|
| Q01 | 1 | pass | pass | 191 | 319 | 6 |
| Q02 | 105 | pass | pass | 1488 | 1261 | 0 |
| Q03 | 1 | pass | pass | 2692 | 4329 | 19 |
| Q04 | 1 | pass | pass | 2564 | 2483 | 0 |
| Q05 | 1 | pass | pass | 1401 | 1402 | 0 |
| Q06 | 2 | pass | pass | 5706 | 5122 | 0 |
| Q07 | 3 | pass | pass | 406 | 471 | 0 |
| Q08 | 2 | pass | pass | 4941 | 3497 | 0 |
| Q09 | 20 | pass | pass | 12026 | 1614 | 0 |
| Q10 | 158 | pass | pass | 3630 | 191779 | 0 |
| Q11 | 157 | pass | unsupported | 5142 | not available | 3 |
| Q12 | 10 | pass | pass | 6588 | 186785 | 0 |

Table 2 answers this question: which fiber bundle gates passed.
Rows are gates. Columns are the gate name and the result.

| Gate | Result |
|---|---|
| Concurrent build equals quiet build | pass |
| Frozen epoch exposed | pass |
| Epoch stamp detects staleness | pass |
| Fiber equals fallback on 10,000 operations | pass |
| No deadlock | pass |
| Lag bounded to 2 generations | pass |

## 3. Warnings

- Each timing is one measurement. It is not a median. Run the major benchmark before you compare engines.
- Q11 has no baseline time. The nested-loop plan exceeded the candidate limit. The application returned an explicit limit error.
- A TQL optimizer cost stays an estimate. The TQL to fiber execution bridge is not built.
- The fiber lifecycle binary uses its own internal fixture. It does not consume the SQL corpus.
