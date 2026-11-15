// sharding.rs - multi-segment sharding foundation.
//
// A sharded segment set holds multiple files that together form one logical
// table. Cells are contiguous across the files, so cell lookup is a bound
// search over the cumulative counts.

/// A reference to one segment within a sharded dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRef {
    pub path: std::path::PathBuf,
    pub generation: u64,
    pub ncells: u64,
    pub src_fingerprint: u64,
}

/// A sharded segment set. `segment_bounds[i]` is the exclusive end cell index
/// of segment i, so the last bound equals the total cell count.
#[derive(Clone, Debug, Default)]
pub struct SegmentSet {
    pub segments: Vec<SegmentRef>,
    pub segment_bounds: Vec<u64>,
}

impl SegmentSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_cells(&self) -> u64 {
        self.segment_bounds.last().copied().unwrap_or(0)
    }

    /// Find the segment that holds `cell` and the local index inside it.
    pub fn locate(&self, cell: u64) -> Option<(usize, u64)> {
        if self.segments.is_empty() {
            return None;
        }
        let mut prev = 0u64;
        for (i, bound) in self.segment_bounds.iter().enumerate() {
            if cell < *bound {
                return Some((i, cell - prev));
            }
            prev = *bound;
        }
        None
    }

    /// Append a segment. The bound grows by the segment cell count.
    pub fn push_segment(&mut self, seg: SegmentRef) {
        let start = self.total_cells();
        self.segment_bounds.push(start + seg.ncells);
        self.segments.push(seg);
    }
}

impl Default for SegmentRef {
    fn default() -> Self {
        SegmentRef {
            path: std::path::PathBuf::new(),
            generation: 0,
            ncells: 0,
            src_fingerprint: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(cells: u64) -> SegmentRef {
        SegmentRef { ncells: cells, ..SegmentRef::default() }
    }

    #[test]
    fn locate_maps_cells_to_segment_and_local_index() {
        let mut set = SegmentSet::new();
        set.push_segment(seg(10));
        set.push_segment(seg(5));
        assert_eq!(set.total_cells(), 15);
        assert_eq!(set.locate(0), Some((0, 0)));
        assert_eq!(set.locate(9), Some((0, 9)));
        assert_eq!(set.locate(10), Some((1, 0)));
        assert_eq!(set.locate(14), Some((1, 4)));
        assert_eq!(set.locate(15), None);
    }

    #[test]
    fn empty_set_has_no_location() {
        let set = SegmentSet::new();
        assert_eq!(set.total_cells(), 0);
        assert_eq!(set.locate(0), None);
    }
}
