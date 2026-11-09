// crates/sheaf-storage/src/bcsf/indices.rs — store sorted boundary identifiers
// FILE-013. Must never hold floats.
#[derive(Debug)]
pub struct IncidenceIndices {
    /// Flat contiguous boundary ids; each cell slice is sorted.
    pub indices: Vec<u64>,
}

impl IncidenceIndices {
    pub fn slice(&self, start: usize, end: usize) -> &[u64] {
        &self.indices[start..end]
    }
}
