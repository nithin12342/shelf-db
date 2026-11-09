# TQL_COST_OPTIMIZER_PLAN_2026-10-23.md - operational plan for the TQL statistical cost optimizer

**Document date:** 2026-10-23.
**Status:** Plan of record for Topological Query Language statistical optimization.
**Target branch:** `fiber-bundle-replacement`.
**Writing standard:** ASD-STE100 Simplified Technical English.

---

## 1. Concept definitions

- **Topological Query Language (TQL):** The declarative query language for ShelfDB cellular complexes.
- **Topological Intermediate Representation (TIR):** The twelve-operation query plan format before execution.
- **Equality graph (E-graph):** A data structure that stores equivalent query expressions.
- **Statistical cost model:** A mathematical calculation that estimates query execution cost.
- **Partially Ordered Set (poset):** A mathematical set with an ordering relation.
- **Fiber bundle section:** A contiguous memory buffer that stores typed column vectors.

---

## 2. Problem statement

ShelfDB currently maintains two separate execution systems:
1. The declarative TQL parser and rule-based optimizer in `sheaf-query`.
2. The handwritten benchmark kernels in `sheaf-engine`.

The benchmark kernels bypass the TQL optimizer. This bypass causes four problems:
- A user cannot submit arbitrary ad-hoc queries at runtime.
- The TQL optimizer lacks data statistics and cardinality estimates.
- The engine cannot reorder joins or push filters down based on selectivity.
- Complex queries risk intermediate memory expansion when join order is sub-optimal.

This operational plan defines the nodes that replace the rule-based stub with a statistical cost optimizer.

---

## 3. Architecture of the unified optimizer

```
[ TQL Query String ]
         |
         v
[ Syntax Parser: ast.rs ]
         |
         v
[ TIR Lowering: tir.rs ]
         |
         v
[ Eulerian Validator: validator.rs ] (Rejects delta^2 != 0)
         |
         v
[ Statistical Cost Optimizer: optimizer.rs ]
  ├── 1. Poset Boundary Pruning (Reduces 2^N to 1 slice)
  ├── 2. Catalog Statistics (NDV, row counts, null fractions)
  ├── 3. Selectivity Estimation (Equality, range, AND, OR)
  └── 4. Cost Model (Scan, filter, join, and solve weights)
         |
         v
[ Costed Execution Plan ]
         |
         v
[ Fiber Bundle Execution Bridge ] (Direct zero-copy section evaluation)
```

---

## 4. Work breakdown nodes

| Node | Name | Files | Acceptance criteria | Gate status |
|---|---|---|---|---|
| OPT-001 | Statistical cost model and selectivity estimator | `sheaf-query/src/ir/optimizer.rs` | The optimizer computes operator costs and predicate selectivities; estimates monotonic plan cost; passes unit tests | Active |
| OPT-002 | Poset cost plan explain integration | `sheaf-query/src/ir/optimizer.rs`, `sheaf-engine/src/bin/phase5_bench.rs` | EXPLAIN displays total cost, estimated rows, and slice counts; backward compatible with phase5 bench | Active |
| OPT-003 | Catalog statistics metadata persistence | `sheaf-storage/src/bcsf/`, `sheaf-query/src/` | Storage headers record distinct value counts and boundary degrees per cell type | Planned |
| OPT-004 | Unified TQL to fiber kernel execution bridge | `sheaf-engine/src/api/`, `sheaf-engine/src/bin/` | TQL queries compile directly to fiber bundle section readers without manual Rust code | Planned |

---

## 5. Mathematical cost formulation

The cost model evaluates each operation in the plan with the formula:

$$\text{Total Cost} = C_{\text{scan}} + C_{\text{filter}} + C_{\text{join}} + C_{\text{solve}}$$

The operation costs use these formulas:

1. **Scan cost:**
   $$C_{\text{scan}} = N_{\text{rows}} \times W_{\text{io}}$$
   $N_{\text{rows}}$ is the cell count. $W_{\text{io}}$ is the input/output cost per row.

2. **Filter cost:**
   $$C_{\text{filter}} = N_{\text{rows}} \times W_{\text{cpu}}$$
   $W_{\text{cpu}}$ is the CPU evaluation cost per predicate.

3. **Poset join cost:**
   $$C_{\text{join}} = N_{\text{surviving}} \times S_{\text{boundary}} \times W_{\text{probe}}$$
   $N_{\text{surviving}}$ is $N_{\text{rows}} \times \text{selectivity}$. $S_{\text{boundary}}$ is the boundary slice count.

4. **Harmonic solve cost:**
   $$C_{\text{solve}} = N_{\text{matches}} \times K_{\text{iters}} \times D_{\text{stalk}} \times W_{\text{flop}}$$
   $K_{\text{iters}}$ is the iteration limit. $D_{\text{stalk}}$ is the continuous stalk dimension.

---

## 6. Selectivity estimation formulas

The selectivity engine estimates predicate selectivity $S$ with standard statistical formulas:

- **Equality ($A = c$):**
  $$S = \frac{1}{\text{NDV}(A)}$$
  $\text{NDV}(A)$ is the number of distinct values of attribute $A$.

- **Inequality Range ($A > c$ or $A < c$):**
  $$S = 0.3333$$
  Default range estimate when histogram boundaries are absent.

- **Conjunction ($P_1 \land P_2$):**
  $$S = S(P_1) \times S(P_2)$$

- **Disjunction ($P_1 \lor P_2$):**
  $$S = S(P_1) + S(P_2) - (S(P_1) \times S(P_2))$$

- **Negation ($\neg P$):**
  $$S = 1.0 - S(P)$$

---

## 7. Verification gates

1. **Unit tests in `sheaf-query`:**
   Must test selectivity calculation, operator costs, and plan reordering.
2. **Phase-5 regression gate:**
   `phase5_bench` and `phase5_adv` must continue to pass with zero failures.
3. **Line count limit gate:**
   All files must maintain `wc -l <= 500`.
4. **Clippy gate:**
   Workspace must compile clean with zero warnings under `-D warnings`.
5. **Dependency Inversion gate:**
   `dip_audit.sh` must pass clean.
