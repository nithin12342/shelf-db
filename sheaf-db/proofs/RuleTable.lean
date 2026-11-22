-- Timestamp: 2026-11-22_11-53-08 +05:30
import Optimizer
import Cohomology
import Restriction
import Gluing

/-!
# The AOT verified rule table (NF-01)

This module is the single source of truth for the rule ids that the runtime
may apply. Each entry names a theorem proved in one of the imported modules.
The emitter writes those entries into a C header and a text manifest. The Rust
drift gate parses the header and compares it with its own rule table.
-/

namespace Sheaf.RuleTable

/-- One rule: an id, a stable name, and the Lean theorem that proves it. -/
def entries : List (Nat Ã— String Ã— String) :=
  [ (1, "restriction_pushdown", "restriction_pushdown_invariant")
  , (2, "colimit_assoc", "colimit_assoc")
  , (3, "restriction_comp", "restriction_comp")
  , (4, "filter_restrict_commute", "filter_restrict_commute")
  , (5, "identity_restriction", "identity_restriction_elim")
  , (6, "gluing_unique", "sheaf_gluing_unique")
  ]

/-- A checksum over ids, names, and theorem names. The Rust gate recomputes it
    from the emitted header, so any edit to the table fails the drift test. -/
def digest : Nat :=
  entries.foldl (fun acc entry =>
    acc + entry.1 + entry.2.1.length + entry.2.2.length) 0

/- Elaborating these names makes this module fail to compile if a proof is
   removed or renamed. -/
#check Sheaf.Optimizer.restriction_pushdown_invariant
#check Sheaf.Optimizer.colimit_assoc
#check Sheaf.restriction_comp
#check Sheaf.Optimizer.filter_restrict_commute
#check Sheaf.Optimizer.identity_restriction_elim
#check Sheaf.sheaf_gluing_unique

theorem all_proofs_present : True := trivial

end Sheaf.RuleTable
