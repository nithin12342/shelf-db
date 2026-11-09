// crates/sheaf-core/src/morphism/projection.rs — evaluate discrete projection masks
// FILE-009. Must never multiply float matrices.
pub fn project_field(payload: &[u8; 16], mask: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = payload[i] & mask[i];
    }
    out
}
