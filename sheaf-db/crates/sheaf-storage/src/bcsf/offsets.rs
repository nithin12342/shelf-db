// crates/sheaf-storage/src/bcsf/offsets.rs — index cell to incidence slice
// FILE-012. Must never mutate on write path.
#[derive(Debug)]
pub struct CellOffsets {
    /// offsets[i] = start of cell i in indices; len = ncells + 1.
    pub offsets: Vec<u32>,
}

impl CellOffsets {
    pub fn range(&self, cell: usize) -> (usize, usize) {
        (self.offsets[cell] as usize, self.offsets[cell + 1] as usize)
    }
}
