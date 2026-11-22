// Timestamp: 2026-11-22_11-53-08 +05:30
// assign.rs - deterministic cell placement (NF-08).

/// Assign a cell to a shard. The rule is stable and needs no shared state.
pub fn assign_vertex(cell_id: u64, shard_count: usize) -> usize {
    (cell_id as usize) % shard_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_is_stable() {
        assert_eq!(assign_vertex(7, 4), 3);
        assert_eq!(assign_vertex(7, 4), assign_vertex(7, 4));
    }
}
