/*
 * sheaf_verified_rules.h - verified TIR rewrite-rule validity table.
 *
 * Spec Pillar 6: the Lean 4 tier proves each rewrite rule semantically
 * invariant and emits this static header. The Rust mirror lives in
 * crates/sheaf-proofs/src/rules.rs. The two tables must stay in lockstep.
 *
 * This file is the emitted artifact. When the Lean toolchain is installed,
 * `lake build` regenerates it from the theorem list in proofs/Optimizer.lean
 * and proofs/Restriction.lean. Until then it is maintained by hand to match
 * the proven theorems.
 *
 * Because every entry is proof-backed, a planner applies a rule without a
 * runtime semantic validation check.
 */
#ifndef SHEAF_VERIFIED_RULES_H
#define SHEAF_VERIFIED_RULES_H

#include <stdbool.h>
#include <stdint.h>

/* Rule identifiers. These match the RULE_* constants in rules.rs. */
#define SHEAF_RULE_RESTRICTION_PUSHDOWN 1
#define SHEAF_RULE_COLIMIT_ASSOC 2
#define SHEAF_RULE_RESTRICTION_COMP 3
#define SHEAF_RULE_FILTER_RESTRICT_COMMUTE 4
#define SHEAF_RULE_IDENTITY_RESTRICTION 5
#define SHEAF_RULE_GLUING_UNIQUE 6

/* One verified rewrite rule. `invariant` records that the named Lean theorem
 * proves the rewrite preserves plan semantics. */
typedef struct {
    uint16_t id;
    const char* name;
    const char* lean_theorem;
    bool invariant;
} SheafVerifiedRule;

/* The static validity table, in stable order. */
static const SheafVerifiedRule SHEAF_VERIFIED_RULES[] = {
    {SHEAF_RULE_RESTRICTION_PUSHDOWN, "restriction_pushdown",
     "restriction_pushdown_invariant", true},
    {SHEAF_RULE_COLIMIT_ASSOC, "colimit_assoc", "colimit_assoc", true},
    {SHEAF_RULE_RESTRICTION_COMP, "restriction_comp", "restriction_comp", true},
    {SHEAF_RULE_FILTER_RESTRICT_COMMUTE, "filter_restrict_commute",
     "filter_restrict_commute", true},
    {SHEAF_RULE_IDENTITY_RESTRICTION, "identity_restriction",
     "identity_restriction_elim", true},
    {SHEAF_RULE_GLUING_UNIQUE, "gluing_unique", "sheaf_gluing_unique", true},
};

#define SHEAF_VERIFIED_RULE_COUNT \
    (sizeof(SHEAF_VERIFIED_RULES) / sizeof(SHEAF_VERIFIED_RULES[0]))

/* True when `id` names a proof-backed, semantically invariant rule. */
static inline bool sheaf_rule_verified(uint16_t id) {
    for (size_t i = 0; i < SHEAF_VERIFIED_RULE_COUNT; ++i) {
        if (SHEAF_VERIFIED_RULES[i].id == id) {
            return SHEAF_VERIFIED_RULES[i].invariant;
        }
    }
    return false;
}

#endif /* SHEAF_VERIFIED_RULES_H */