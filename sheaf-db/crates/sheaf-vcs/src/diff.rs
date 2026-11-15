// diff.rs - reversible time-travel diff and cohomological merge (Pillar 8).
//
// Time-travel diff. The difference between historical states s_t1 and s_t2 is
// computed algebraically:
//     delta_diff = delta(s_t2 - s_t1)
// This isolates the exact cell coordinates that changed, without a full row
// comparison.
//
// Cohomological merge. A commit must be a zero-coboundary global section
// (delta x = 0). Two branches carry proposed 0-cochains a and b over a common
// base. The engine composes them additively, m = a + b - base, and tests the
// zero-coboundary rule on m. On a 1-complex there are no 2-cells, so
// H^1 = C^1 / im(delta^0). A nonzero residual delta m is exactly a 1-cochain
// outside im(delta^0): the cohomological obstruction the spec calls a merge
// conflict. A zero residual means the branches compose commutatively.

use crate::complex::Complex;
use crate::section::Section;

/// The vertex indices whose stalk differs between two sections. This isolates
/// the changed cells for a character-level or entity-level diff.
pub fn changed_cells(c: &Complex, before: &Section, after: &Section) -> Vec<usize> {
    let mut changed = Vec::new();
    for i in 0..c.vertex_count() {
        if c.vertex_stalk(&before.values, i) != c.vertex_stalk(&after.values, i) {
            changed.push(i);
        }
    }
    changed
}

/// The difference 0-cochain after - before.
pub fn diff_section(before: &Section, after: &Section) -> Section {
    after.sub(before)
}

/// The algebraic time-travel diff delta(after - before) as a 1-cochain.
pub fn coboundary_diff(c: &Complex, before: &Section, after: &Section) -> Vec<f32> {
    c.coboundary(&diff_section(before, after).values)
}

/// A merge conflict: the composed state has a nonzero coboundary residual.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeConflict {
    /// The Dirichlet energy ||delta m||^2 of the composed candidate.
    pub energy: f32,
    /// The residual 1-cochain delta m, the cohomological obstruction.
    pub residual: Vec<f32>,
}

/// Compose two branches over a common base and test the global-section rule.
/// Returns the merged section when the residual is within `tol`, or a conflict
/// carrying the obstruction when it is not.
pub fn merge(
    c: &Complex,
    base: &Section,
    a: &Section,
    b: &Section,
    tol: f32,
) -> Result<Section, MergeConflict> {
    // Additive composition of the two branch deltas relative to the base.
    let merged = a.add(b).sub(base);
    let residual = c.coboundary(&merged.values);
    let energy: f32 = residual.iter().map(|v| v * v).sum();
    if energy <= tol {
        Ok(merged)
    } else {
        Err(MergeConflict { energy, residual })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Restriction;

    fn line() -> Complex {
        let mut c = Complex::new(vec![1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c
    }

    fn sec(c: &Complex, v: Vec<f32>) -> Section {
        Section::new(c, v).unwrap()
    }

    #[test]
    fn changed_cells_isolates_modified_vertices() {
        let c = line();
        let before = sec(&c, vec![1.0, 2.0]);
        let after = sec(&c, vec![1.0, 9.0]);
        assert_eq!(changed_cells(&c, &before, &after), vec![1]);
    }

    #[test]
    fn coboundary_diff_matches_delta_of_difference() {
        let c = line();
        let before = sec(&c, vec![1.0, 2.0]);
        let after = sec(&c, vec![4.0, 8.0]);
        // difference = [3, 6], delta = 6 - 3 = 3.
        assert_eq!(coboundary_diff(&c, &before, &after), vec![3.0]);
        assert_eq!(diff_section(&before, &after).values, vec![3.0, 6.0]);
    }

    #[test]
    fn compatible_branches_merge_to_a_global_section() {
        let c = line();
        let base = sec(&c, vec![0.0, 0.0]);
        let a = sec(&c, vec![1.0, 1.0]);
        let b = sec(&c, vec![2.0, 2.0]);
        let merged = merge(&c, &base, &a, &b, 1e-6).unwrap();
        assert_eq!(merged.values, vec![3.0, 3.0]);
        assert!(merged.is_global(&c, 1e-6));
    }

    #[test]
    fn incompatible_branches_report_a_cohomological_conflict() {
        let c = line();
        let base = sec(&c, vec![0.0, 0.0]);
        let a = sec(&c, vec![1.0, 1.0]);
        // b is a local edit that breaks the gluing condition on the edge.
        let b = sec(&c, vec![0.0, 1.0]);
        let conflict = merge(&c, &base, &a, &b, 1e-6).unwrap_err();
        // composed m = [1, 2], delta m = [1], energy = 1.
        assert_eq!(conflict.residual, vec![1.0]);
        assert_eq!(conflict.energy, 1.0);
    }

    #[test]
    fn two_global_commits_always_compose() {
        // Global sections form the kernel of delta, a vector space. The
        // additive compose of two kernels over a kernel base stays a kernel.
        let c = line();
        let base = Section::constant(&c, 1.0);
        let a = Section::constant(&c, 2.0);
        let b = Section::constant(&c, 3.0);
        let merged = merge(&c, &base, &a, &b, 1e-6).unwrap();
        assert!(merged.is_global(&c, 1e-6));
        assert_eq!(merged.values, vec![4.0, 4.0]);
    }
}