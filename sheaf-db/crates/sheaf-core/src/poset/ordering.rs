// crates/sheaf-core/src/poset/ordering.rs — enforce poset order acyclicity
// FILE-004. Must never execute linear algebra.
/// Returns true if dim(a) <= dim(b) and a != b can be an incidence edge.
/// Full cycle detection happens at compaction; this is the cheap check.
pub fn can_be_face(face_dim: u8, cell_dim: u8) -> bool {
    face_dim < cell_dim
}
