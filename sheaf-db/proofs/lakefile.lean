import Lake
open Lake DSL

/-!
Build configuration for the SheafDB formal verification tier (spec Pillar 6).

Install path (the Lean toolchain is not installed yet):
  1. Install elan, the Lean version manager.
  2. `lake update` to fetch Mathlib at the pinned toolchain.
  3. `lake exe cache get` to download the Mathlib build cache.
  4. `lake build` to compile the proofs.

The library roots are the five proof modules at this directory root. Module
names are bare (`Poset`, `Restriction`, ...); the namespaces inside are `Sheaf`.
-/

package sheafProofs where
  leanOptions := #[⟨`autoImplicit, false⟩]

require mathlib from git "https://github.com/leanprover-community/mathlib4"

lean_lib Sheaf where
  srcDir := "."
  roots := #[`Poset, `Restriction, `Gluing, `Cohomology, `Optimizer]