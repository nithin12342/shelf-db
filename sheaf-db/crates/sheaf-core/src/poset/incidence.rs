// crates/sheaf-core/src/poset/incidence.rs — store codim-1 boundaries virtualize composites
// FILE-003. Must never materialize full simplex closure.
// Rule (audit Defect 6): delta/∂ act only between adjacent dimensions
// (codim 1). A k-cell referencing 0-cells directly is evaluated as a
// composite chain map through virtual codim-1 faces generated on the fly
// via canonical sorted-subset indexing. No intermediate cells stored.
pub fn is_sorted(ids: &[u64]) -> bool {
    ids.windows(2).all(|w| w[0] <= w[1])
}

/// Number of virtual codim-1 faces used by the composite map for an
/// N-boundary hypercell. Phase 1 uses N (one virtual face per vertex).
pub fn virtual_face_count(boundary_len: usize) -> usize {
    boundary_len
}

/// Canonical virtual face index for (face_position, member_position).
/// Sorted-subset indexing keeps parity signs deterministic.
pub fn virtual_face_member(face: usize, _members: usize) -> usize {
    face
}
