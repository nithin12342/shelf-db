# AGENTS.md - project instructions for the d2 repository

These instructions apply to every response in this project.

The base writing standard is **ASD-STE100 Simplified Technical English**. ASD-STE100 is the controlled language that aerospace companies use for maintenance manuals. ASD Europe published the standard. The current issue is Issue 9, from January 15, 2025. The official text is free at <https://www.asd-ste100.org/>.

The global rules at `~/.config/opencode/AGENTS.md` also apply. This file holds the rules for this project.

## Database scope and protected archive

- This is a database project. Neural-network creation, training, model architectures, training-specific bridges, and training datasets are out of scope.
- Do not count excluded work as unfinished database functionality. Database mathematics and topological operators remain in scope.
- The protected archive is `ARCHIVED_NEURAL_NETWORK_DOCUMENTATION/` at the project root.
- Its absolute path is `C:/Users/thela/OneDrive/Desktop/folder/shelf-db/ARCHIVED_NEURAL_NETWORK_DOCUMENTATION`.
- Do not read, search, index, summarize, restore, copy, or use Markdown files in that archive.
- Exclude the archive before every recursive scan, document ingestion, or context-loading operation. The restriction also applies to tools and subagents.
- Restored mixed database documents outside the archive are approved reading sources. Use their database requirements, not their neural-network requirements.
- Do not recover excluded neural-network requirements through Git history, compressed bundles, old conversation context, or duplicate files.
- Start document discovery at `FEATURE_DOCUMENT_CHRONOLOGY/chronology.json`. Its ordered groups identify authoritative originals and historical copies.
- The active `AGENTS.md` and `SKELETON.md` take priority over historical copies. Historical results do not prove current implementation status.
- Preserve this exclusion during instruction-file updates. Only a new explicit user request that reopens archive access can change it.

---

## 1. Writing standard (ASD-STE100)

### 1.1 Sentences

1. Use the active voice. Name the actor.
2. Use simple tenses. Use present, past, or future tense. Do not use present perfect.
3. Use can, will, and must for the modals. Do not use should, would, may, might, or could.
4. Keep the articles. Keep the word "that".
5. Do not use contractions.
6. Do not use semicolons. Write two sentences.
7. Do not use em-dashes. Name the relation with because, but, or for example.
8. Keep most sentences near 20 words or fewer.

### 1.2 Words

1. Define a concept term at its first use. Use fewer than ten words for the definition.
2. Use one word for one meaning in the whole answer.
3. Write "make sure that" for check, verify, confirm, validate, and ensure.
4. Write "configuration" for config, settings, and options.
5. Spell out every abbreviation at first use, in the same sentence.
6. Do not use short codes on their own, such as B1, B2, B3, N7, P3, or S4.
7. If a code is a required identifier, write the plain-English name next to it every time.
8. Delete words that carry no fact. Common examples are simply, robust, powerful, comprehensive, leverage, crucial, and seamlessly.
9. Do not write "not just X, it is Y". Do not use "studies show". Name the source or drop the claim.

### 1.3 Safety markers

1. Use WARNING for a risk of injury. Use CAUTION for a risk of damage. Use WARNING if both risks apply.
2. Put the command or the condition first. Give the risk after the command.
3. Keep the marker under 20 words.

### 1.4 Check before you send

1. Search for the em-dash character. Remove each one.
2. Search for a semicolon. Write two sentences instead.
3. Search for "should", "may", "might", "could", and "would". Replace each one.
4. Count the words in your three longest sentences. Split any sentence over 20 words.
5. Show the evidence. Do not claim that you did the work.

---

## 2. Response format

### 2.1 Shape of the answer

1. Give the answer or the result first, in one or two sentences.
2. Give the detail as numbered steps.
3. Give the evidence.
4. Give the warnings, cautions, and notes.

### 2.2 Tables

1. Give every table a title that says what it answers.
2. State what each row is and what each column is, before or just under the table.
3. Put the unit in the header of every numeric column, for example "Query time (ms)".
4. Put pass or fail, and any remarks, in their own column. Do not mix them into a number.
5. Order the rows so the reference system is first or last, and label it clearly.
6. If a comparison spans several tables, restate the row and column meaning in each table.
7. Write each column header in short words. Do not use an abbreviation in a header.

### 2.3 Diagrams and lists

1. Use simple plain-text diagrams for flows, hierarchies, and pipelines. Label every element.
2. Use short bullets for lists. Put one fact in each bullet.
3. Do not write a paragraph longer than four lines.
4. Do not use a paragraph where a list or a table is clearer.
5. Do not use bold as decoration. Do not use emoji as structure.

---

## 3. Project rules

1. Push with normal `git push` to the connected remote `origin`. The user retired the `push_personal.py` token flow on 2026-11-09 because the token file is outdated. Push only once, after all granular commits are complete.
2. Make one separate commit for every file creation and for every file modification. Do not combine several files into one commit. This is the granular commit rule. When one file holds changes for several features, make one commit for each feature change in that file. Name the feature and the file in the commit message. One revert must undo one feature change.
3. Keep `SKELETON.md` up to date. It is the source of truth for this repository.
4. Follow the Intention Engineering methodology in the global rules file. Load the `intention-engineering` skill first when that skill is available in the session.
5. Compare the databases with a Topological Query Language (TQL) query. A TQL query is a declarative query over a cellular complex.
6. Give the TQL text, the native equivalent, and the measured time for each engine in the comparison.
7. State a TQL optimizer cost as an estimate. Do not state it as an execution time. The TQL-to-fiber execution bridge (OPT-004) is not built.

---

## 3b. Next-generation stack rules (user instruction, 2026-11-09)

These rules govern the SheafDB next-generation implementation on the `combined-stack-feature-improvement` branch.

1. No code file exceeds 500 lines.
2. Add new features as new code files. Do not grow existing files to hold new features.
3. Use an object-oriented, SOLID structure. Separate interfaces from implementations.
4. Use the right language at the right tier: C++23 for math kernels, Zig for physical storage, C11 for the WAL and hardware paths, Rust for orchestration and ingress, Lean 4 for proofs. Add procedural macros and Rust metaprogramming only for compile-time specialization of those tiers. Do not move native math, storage, or hardware work into Rust because a macro generates it.
5. Network agreement must use sheaf gluing consensus. Do not use Raft voting loops.
6. Keep the folder structure logical and navigatable. Write comment lines that explain the logic.
7. Write native code even when its toolchain is not installed yet. The user installs the toolchains later. Keep the Rust workspace buildable without them through fallback backends.

## 3c. Feature completion and test rule (user instruction, 2026-11-15)

These rules apply to every feature on every branch. They also apply to every global and project instruction file.

1. Write a test for each feature. The test must call the production code path with real input.
2. The test must compare the real output with a stated expected result.
3. Do not hard-code an output value in the feature code or in the test. A constant must not produce the expected result.
4. Do not add a silent fallback. A declared fallback must report its use and must return an explicit result.
5. A platform-gated feature must report `unsupported` with a clear error. It must not return a fake result.
6. Record the test command and the test output in the commit message or in the evidence file.
7. A feature with no passing test is not complete. Do not count it as done.

## 3d. Artifact naming and provenance (user instruction, 2026-11-22)

These rules apply to every document, plan, report, and evidence file in this repository.

1. Put a timestamp in every new file name. The format is `YYYY-MM-DD_HH-MM-SS`.
2. The timestamp gives the year, month, date, hour, minute, and second.
3. Put the same timestamp in a header line inside the file, with the local time and the UTC offset.
4. Name the repository remote in full. The remote for this repository is `origin https://github.com/nithin12342/shelf-db.git`.
5. Name the branch. The active branch is `combined-stack-feature-improvement`, unless the user selects another branch.
6. Name the full 40-character commit hash that is the top of that branch when the file is written.
7. Read the hash from `git rev-parse HEAD` before you write it. Do not name a commit that does not exist.
8. Source code files keep their language-required names. Put the timestamp in the header comment of each new source file.
9. The granular commit rule still applies. Make one commit for one file creation or one file modification.
10. A result that does not name its remote, branch, and top commit is not reproducible. Do not count it as evidence.

**Repository identity at this writing.** Remote `origin https://github.com/nithin12342/shelf-db.git`. Branch `combined-stack-feature-improvement`. Top commit `1aae55de9bbd3e75fdb2a63b26cff7de4b17669b` on 2026-11-22 at 09:02:56 +05:30.

---

## 4. The document set

Start with `FEATURE_DOCUMENT_CHRONOLOGY/chronology.json` for the ordered database reading path and the excluded neural-network sections.
Read requirements before implementation records. Read the performance plan of record before performance changes.

`SKELETON.md` holds the source of truth for the code. These documents hold the analysis. Every
number in every one of them names a committed result file, or a derivation shown in full.

| Document | What it answers |
|---|---|
| `FEATURE_DOCUMENT_CHRONOLOGY/chronology.json` | Ordered agent reading index, canonical sources, and excluded sections. |
| `SheafDB_Next_Gen_Technical_Specification.md` | Restored database requirements. Neural-network sections are excluded. |
| `Sheaf_Markdown_Documentation_Bundle (1)/Cellular_Sheaf_Complete_System_Architecture.md` | Mainly a neural-system proposal. Database storage and shared mathematics are background only. |
| `SHELFDB_SCHEMA_DESIGN_AND_QUERY_PATTERNS.md` | Restored database schema and query guidance. Training sections are excluded. |
| `ARCHITECTURAL_IMPROVEMENT_DISCUSSION_2026-10-27.md` | Database architecture decisions and historical context. |
| `PERFORMANCE_FIX_PLAN_2026-10-22.md` | **The plan of record.** The eight nodes that remove the measured performance waste, with the order as a dependency. |
| `PERFORMANCE_WASTE_ANALYSIS.md` | Where the time goes, for both comparison tables, with the measured cause of each waste. |
| `CODE_CHANGES_AND_EXPECTED_GAINS.md` | The six code changes with their measured gains, their files and their gates. |
| `SEVEN_DATABASE_AREA_COMPARISON.md` | The shelf database against the seven, area by area, current and projected. |
| `RUNTIME_COMPARISON.md` | The existing seven-database comparison, with the correction on the setup column. |
| `ADVERSARIAL_ANALYSIS.md` | The three adversarial attacks on the shelf architecture. |
| `ARCHITECTURE_GAINS.md` | The improvement candidates and the timestamped analysis record. |
| `SHELF_DATA_REPRESENTATION_REPORT.md` | The read and write cost of the shelf representation. |
| `VON_NEUMANN_LIMITS.md` and `VON_NEUMANN_LIMITS_STORAGE.md` | The 18 physical limits, measured against derived. |
| `PROJECTION_THEORETICAL_MAXIMUM.md` | The earlier projection, superseded, kept as the record. |
| `FIBER_LIFECYCLE_PLAN.md` | The fiber bundle lifecycle. Part C holds the performance-waste nodes. |
| `CONVERSATION_LOG_2026-10-21.md` and `CONVERSATION_LOG_2026-10-22.md` | The session records, with every correction filed. |
| `CODE_CHANGE_MAP.md` | Every change mapped to its files, its size and its state. |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | The live application comparison, with the TQL text, the native equivalent, and the real number for each engine. |
| `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Kdb+ installation, license gate, and time-series comparison against ShelfDB and DuckDB. |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Native CLI manual execution benchmark across the five failure workloads and the 50 test cases. |
| `TEST_CASES_CONSOLIDATION_2026-10-28.md` | Complete consolidation and inventory of unit tests, E2E suites, and microbenchmarks. |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Native CLI fresh benchmark execution across 50 high-variance test cases and five problem workloads. |
| `CONTINUOUS_STREAMING_READ_WRITE_BENCHMARK_2026-10-28.md` | Continuous concurrent streaming read/write benchmark and incremental fiber build comparison across eight databases. |
| `SHELFDB_WRITE_PATH_FUNDAMENTAL_ARCHITECTURE_AND_OPTIMIZATION.md` | Complete context injection and technical roadmap for external agents to optimize the ShelfDB write path. |
| `MISSING_FEATURE_IMPLEMENTATION_PLAN_2026-11-15.md` | The 17-node plan for the open audit gaps, with the test gate for each node. |
| `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md` | The derived feature count: 48 in scope, 30 implemented, 11 partial, 7 absent. |
| `IMPLEMENTATION_PLAN_MISSING_FEATURES_2026-11-22_09-02-56.md` | The 16-node plan that closes the 18 open features from the inventory. |
| `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` | The on-laptop run plan: compile, load a data set, run SQL, convert to TQL, optimize, and measure. |

**Two rules that came out of the sessions.**

1. Every row in a comparison table names its dataset, and every projected value names the
   measured floor it rests on.
2. A claim about a production code path comes from a measurement of that path. The 7.14x
   whole-file read waste was measured in an analysis tool, and the production reader was
   already section-scoped.
