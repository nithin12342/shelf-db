# HORIZONTAL_SLICING_PLAN_2026-10-27.md

## 1. Verdict

Horizontal slicing can help throughput. It will not help a single query. The current bottleneck is random probe latency and text parsing, not stage boundaries. Fix those first. Then slice the pipeline for multi-query throughput.

The monolith in `crates/sheaf-engine/src/bin/sheaf_db_app.rs` runs five phases in one thread. Parse, build, match, solve, and emit all block each other. Slicing them into stages with bounded rings lets the CPU overlap work across queries.

---

## 2. Analysis of the monolith

### 2.1 Current phases

| Phase | Location | Cost driver |
|---|---|---|
| Parse and validate | `parse_program`, `validate_program` | Text extraction and UTF-8 validation |
| BCSF build | `BcsfBuilder::push`, `finish` | Allocation and sorting |
| Match | Loop over `reader.len()` | Random probes and hash lookups |
| Kernel solve | `harmonic_relax_target` | Dense float work |
| Emit | `SectionResult::render` | String formatting |

### 2.2 What slicing changes

- Each phase becomes a stage with its own thread.
- Stages communicate through bounded 64-byte work items.
- A ring of fixed size provides backpressure.
- One thread owns each stage, so no locks sit on the data path.

### 2.3 What slicing does not change

- It does not reduce bytes moved by one query.
- It does not remove the join in the match phase.
- It does not fix the text parsing cost.

### 2.4 Dependency

Finish the nodes in `PERFORMANCE_FIX_PLAN_2026-10-22.md` first. Slicing a slow pipeline only moves the slowness between threads.

---

## 3. Slice boundaries

### 3.1 Stage 0: Parse and validate

- Input: raw TQL text.
- Output: a validated `Program`.
- Owner: `sheaf-query`.
- Thread: pinned to core 0.

### 3.2 Stage 1: BCSF build

- Input: a validated `Program`.
- Output: a `BcsfReader`.
- Owner: `sheaf-storage`.
- Thread: pinned to core 1.

### 3.3 Stage 2: Match and scan

- Input: a `BcsfReader` and a query.
- Output: a list of matched hypercells with face ids.
- Owner: `sheaf-engine`.
- Thread: pinned to core 2.

### 3.4 Stage 3: Kernel solve

- Input: matched hypercells.
- Output: solved sections.
- Owner: `sheaf-kernel`.
- Thread: pinned to core 3.

### 3.5 Stage 4: Emit

- Input: solved sections.
- Output: rendered text and timing.
- Owner: `sheaf-engine`.
- Thread: pinned to core 0 or a dedicated core.

---

## 4. Work item design

### 4.1 WorkItem struct

```rust
#[repr(C, align(64))]
pub struct WorkItem {
    pub sequence: u64,
    pub query_id: u64,
    pub stage: u8,
    pub flags: u8,
    pub payload_len: u16,
    pub payload: [u8; 48],
}
```

### 4.2 Size check

- `size_of::<WorkItem>() == 64`.

### 4.3 Ring

- Power-of-two capacity.
- Single producer and single consumer per ring.
- Relaxed ordering on claim, release ordering on publish.

---

## 5. Phase plan

### Phase 0. Measure baseline

- Run `sheaf_db_app` on the fraud data set.
- Record parse, build, match, solve, and emit times.
- Exit: baseline numbers written to a results file.

### Phase 1. Extract stage functions

- Move each phase from `main` into a function in a new `pipeline` module.
- Keep the single-threaded flow.
- Exit: output identical to baseline.

### Phase 2. Add rings and threads

- Add one bounded ring between each pair of stages.
- Spawn one thread per stage.
- Exit: output identical to baseline.

### Phase 3. Add backpressure

- Block the producer when the ring is full.
- Exit: no dropped work items under load.

### Phase 4. Add metrics

- Record queue depth and stage time per item.
- Exit: metrics printed at shutdown.

### Phase 5. Core pinning

- Pin each stage thread to a fixed core.
- Exit: throughput improves over Phase 2 on a multi-core host.

---

## 6. Existing files to modify

| File | Change | Description |
|---|---|---|
| `crates/sheaf-engine/src/lib.rs` | Append module | Add `pub mod pipeline;` |
| `crates/sheaf-engine/src/bin/sheaf_db_app.rs` | Refactor | Replace inline phases with pipeline runner calls |
| `SKELETON.md` | Append | Add pipeline files and bounded context note |
| `ARCHITECTURE.md` | Append | Add the pipeline folder under EngineRuntime |

No existing file is deleted.

---

## 7. New files

- `crates/sheaf-engine/src/pipeline/mod.rs`
- `crates/sheaf-engine/src/pipeline/work_item.rs`
- `crates/sheaf-engine/src/pipeline/ring.rs`
- `crates/sheaf-engine/src/pipeline/stage_parse.rs`
- `crates/sheaf-engine/src/pipeline/stage_build.rs`
- `crates/sheaf-engine/src/pipeline/stage_match.rs`
- `crates/sheaf-engine/src/pipeline/stage_solve.rs`
- `crates/sheaf-engine/src/pipeline/stage_emit.rs`
- `crates/sheaf-engine/src/pipeline/scheduler.rs`
- `crates/sheaf-engine/src/pipeline/metrics.rs`
- `crates/sheaf-engine/tests/pipeline_e2e.rs`

---

## 8. Folder structure changes

```text
crates/sheaf-engine/src/
  pipeline/
    mod.rs
    work_item.rs
    ring.rs
    stage_parse.rs
    stage_build.rs
    stage_match.rs
    stage_solve.rs
    stage_emit.rs
    scheduler.rs
    metrics.rs
```

---

## 9. Traceability

| New ID | Plain-English name | Maps to |
|---|---|---|
| REQ-061 | Horizontal pipeline requirement | Monolith analysis |
| SPEC-061 | Five-stage pipeline specification | Slice boundaries and work item |
| FOLDER-022 | Pipeline folder | `crates/sheaf-engine/src/pipeline` |
| FILE-127 | Work item | `pipeline/work_item.rs` |
| FILE-128 | Ring | `pipeline/ring.rs` |
| FILE-129 | Parse stage | `pipeline/stage_parse.rs` |
| FILE-130 | Build stage | `pipeline/stage_build.rs` |
| FILE-131 | Match stage | `pipeline/stage_match.rs` |
| FILE-132 | Solve stage | `pipeline/stage_solve.rs` |
| FILE-133 | Emit stage | `pipeline/stage_emit.rs` |
| FILE-134 | Scheduler | `pipeline/scheduler.rs` |
| FILE-135 | Metrics | `pipeline/metrics.rs` |
| METHOD-073 | Baseline gate | Measured phase times |
| METHOD-074 | Equivalence gate | Output matches baseline |
| METHOD-075 | Throughput gate | Queries per second improves |

---

## 10. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Single-query latency increases | Keep a single-threaded mode for small inputs |
| Ring overhead exceeds gain | Measure per-item overhead before pinning cores |
| Stage 3 dominates | Solve stage runs on its own core with no contention |
| Work item too small | Store large payloads by pointer, not inline |
| Loss of debuggability | Keep stage functions callable directly in tests |

---

## 11. Rollback

Remove the `pipeline` module and restore `sheaf_db_app.rs` to the inline version. No data format changes. No migration needed.
