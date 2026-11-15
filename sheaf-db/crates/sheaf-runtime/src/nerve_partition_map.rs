// nerve_partition_map.rs - an ordered partition index built as a B+ tree (MF-01).
//
// The pinning policy in affinity.rs spreads partitions with a modulo rule. A
// clustered layout needs the other shape: a partition belongs to the core that
// owns its range. Ranges are ordered data, so the index is a B+ tree.
//
// Properties of this B+ tree:
//   - every key lives in a leaf and leaves are linked in key order, so a range
//     scan follows the leaf links and never revisits an internal node;
//   - an internal node holds separator keys only, so an internal split copies
//     one key up and keeps the leaf data in place;
//   - a node holds at most `order` keys and splits in half on overflow.
//
// The index answers two questions. `lookup` finds an exact range start.
// `floor` finds the range that contains a partition, which is the routing
// question.

use crate::affinity::PinningPolicy;

/// Why an index operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexError {
    /// The key is already present.
    DuplicateKey(u64),
    /// The order is below four, which cannot split.
    BadOrder(usize),
    /// The ranges are not in ascending order of start.
    UnsortedRange(u64),
    /// The range list has no entry that starts at partition 0.
    MissingOrigin,
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::DuplicateKey(k) => write!(f, "duplicate range start {k}"),
            IndexError::BadOrder(o) => write!(f, "order {o} is below the minimum of 4"),
            IndexError::UnsortedRange(k) => write!(f, "range start {k} is out of order"),
            IndexError::MissingOrigin => write!(f, "no range starts at partition 0"),
        }
    }
}

impl std::error::Error for IndexError {}

/// One B+ tree node.
enum Node {
    /// A leaf holds keys and their values. `next` links the next leaf.
    Leaf { keys: Vec<u64>, values: Vec<usize>, next: Option<usize> },
    /// An internal node holds separators. `children[i]` holds keys below
    /// `keys[i]`, and `children[i + 1]` holds keys at or above it.
    Internal { keys: Vec<u64>, children: Vec<usize> },
}

/// A B+ tree that maps a partition range start to a core.
pub struct PartitionIndex {
    nodes: Vec<Node>,
    root: usize,
    order: usize,
    entries: usize,
}

impl PartitionIndex {
    /// The default node order. A node holds at most eight keys.
    pub const DEFAULT_ORDER: usize = 8;

    /// Build an empty index with the default order.
    pub fn new() -> Self {
        // The default order is four or more, so this cannot fail.
        Self::with_order(Self::DEFAULT_ORDER).expect("default order is valid")
    }

    /// Build an empty index with the given node order.
    pub fn with_order(order: usize) -> Result<Self, IndexError> {
        if order < 4 {
            return Err(IndexError::BadOrder(order));
        }
        Ok(Self {
            nodes: vec![Node::Leaf { keys: Vec::new(), values: Vec::new(), next: None }],
            root: 0,
            order,
            entries: 0,
        })
    }

    /// Number of stored ranges.
    pub fn len(&self) -> usize {
        self.entries
    }

    /// True when no range is stored.
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    /// Number of nodes, which shows the split work.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Add one range start. A repeated start is rejected.
    pub fn insert(&mut self, start: u64, core: usize) -> Result<(), IndexError> {
        // Descend to the owning leaf and remember the path for a split.
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut cur = self.root;
        while let Node::Internal { keys, children } = &self.nodes[cur] {
            let idx = keys.partition_point(|k| *k <= start);
            path.push((cur, idx));
            cur = children[idx];
        }

        let needs_split = match &mut self.nodes[cur] {
            Node::Leaf { keys, values, .. } => {
                match keys.binary_search(&start) {
                    Ok(_) => return Err(IndexError::DuplicateKey(start)),
                    Err(pos) => {
                        keys.insert(pos, start);
                        values.insert(pos, core);
                    }
                }
                keys.len() > self.order
            }
            Node::Internal { .. } => false,
        };

        let mut split = if needs_split { Some(self.split_leaf(cur)) } else { None };

        // A split copies one separator up and can split the parent in turn.
        while let Some((sep, right)) = split.take() {
            match path.pop() {
                Some((parent, child_idx)) => {
                    let parent_overflow = match &mut self.nodes[parent] {
                        Node::Internal { keys, children } => {
                            keys.insert(child_idx, sep);
                            children.insert(child_idx + 1, right);
                            keys.len() > self.order
                        }
                        Node::Leaf { .. } => false,
                    };
                    if parent_overflow {
                        split = Some(self.split_internal(parent));
                    }
                }
                None => {
                    // The root split, so the tree grows by one level.
                    let old_root = self.root;
                    let new_root = self.nodes.len();
                    self.nodes.push(Node::Internal {
                        keys: vec![sep],
                        children: vec![old_root, right],
                    });
                    self.root = new_root;
                }
            }
        }

        self.entries += 1;
        Ok(())
    }

    /// Split a full leaf. Returns the separator and the new right leaf.
    fn split_leaf(&mut self, node: usize) -> (u64, usize) {
        let new_idx = self.nodes.len();
        let (sep, right) = match &mut self.nodes[node] {
            Node::Leaf { keys, values, next } => {
                let mid = keys.len() / 2;
                let right_keys = keys.split_off(mid);
                let right_values = values.split_off(mid);
                let sep = right_keys[0];
                let right_next = *next;
                *next = Some(new_idx);
                (sep, Node::Leaf { keys: right_keys, values: right_values, next: right_next })
            }
            Node::Internal { .. } => unreachable!("split_leaf is called only on a leaf"),
        };
        self.nodes.push(right);
        (sep, new_idx)
    }

    /// Split a full internal node. The middle key moves up, not down.
    fn split_internal(&mut self, node: usize) -> (u64, usize) {
        let new_idx = self.nodes.len();
        let (sep, right) = match &mut self.nodes[node] {
            Node::Internal { keys, children } => {
                let mid = keys.len() / 2;
                let sep = keys[mid];
                let right_keys = keys.split_off(mid + 1);
                let right_children = children.split_off(mid + 1);
                keys.pop();
                (sep, Node::Internal { keys: right_keys, children: right_children })
            }
            Node::Leaf { .. } => unreachable!("split_internal is called only on an internal node"),
        };
        self.nodes.push(right);
        (sep, new_idx)
    }

    /// The node index of the leftmost leaf.
    fn leftmost_leaf(&self) -> usize {
        let mut cur = self.root;
        while let Node::Internal { children, .. } = &self.nodes[cur] {
            cur = children[0];
        }
        cur
    }

    /// Find an exact range start.
    pub fn lookup(&self, start: u64) -> Option<usize> {
        let mut cur = self.root;
        loop {
            match &self.nodes[cur] {
                Node::Internal { keys, children } => {
                    cur = children[keys.partition_point(|k| *k <= start)];
                }
                Node::Leaf { keys, values, .. } => {
                    return match keys.binary_search(&start) {
                        Ok(pos) => Some(values[pos]),
                        Err(_) => None,
                    };
                }
            }
        }
    }

    /// Find the greatest range start that is at or below `partition`. This is
    /// the routing question: which range contains the partition.
    pub fn floor(&self, partition: u64) -> Option<(u64, usize)> {
        let mut cur = self.root;
        loop {
            match &self.nodes[cur] {
                Node::Internal { keys, children } => {
                    cur = children[keys.partition_point(|k| *k <= partition)];
                }
                Node::Leaf { keys, values, next } => {
                    let pos = keys.partition_point(|k| *k <= partition);
                    if pos > 0 {
                        return Some((keys[pos - 1], values[pos - 1]));
                    }
                    match next {
                        Some(n) => cur = *n,
                        None => return None,
                    }
                }
            }
        }
    }

    /// The keys in ascending order. The walk follows the leaf links only.
    pub fn keys(&self) -> Vec<u64> {
        let mut out = Vec::with_capacity(self.entries);
        let mut cur = Some(self.leftmost_leaf());
        while let Some(idx) = cur {
            match &self.nodes[idx] {
                Node::Leaf { keys, next, .. } => {
                    out.extend_from_slice(keys);
                    cur = *next;
                }
                Node::Internal { .. } => break,
            }
        }
        out
    }
}

impl Default for PartitionIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// A pinning policy that maps a partition to the core that owns its range.
pub struct PartitionRanges {
    index: PartitionIndex,
    cores: usize,
    origin_core: usize,
}

impl PartitionRanges {
    /// Build the policy from ordered `(start, core)` pairs. The first pair must
    /// start at partition 0, so every partition has an owner.
    pub fn from_ranges(ranges: &[(u64, usize)]) -> Result<Self, IndexError> {
        let mut index = PartitionIndex::new();
        let mut previous: Option<u64> = None;
        let mut highest_core = 0usize;
        for &(start, core) in ranges {
            if let Some(prev) = previous {
                if start <= prev {
                    return Err(IndexError::UnsortedRange(start));
                }
            }
            index.insert(start, core)?;
            previous = Some(start);
            highest_core = highest_core.max(core);
        }
        let origin_core = index.lookup(0).ok_or(IndexError::MissingOrigin)?;
        Ok(Self { index, cores: highest_core + 1, origin_core })
    }

    /// Number of stored ranges.
    pub fn ranges(&self) -> usize {
        self.index.len()
    }
}

impl PinningPolicy for PartitionRanges {
    fn cores(&self) -> usize {
        self.cores
    }

    fn core_for(&self, partition: u64) -> usize {
        match self.index.floor(partition) {
            Some((_, core)) => core,
            // The construction gate requires a range at partition 0, so this
            // arm means the index was emptied after construction. Report the
            // origin owner rather than an invented core.
            None => self.origin_core,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nerve_lockfree::LockFreeNerveRouter;
    use crate::nerve_port::{NerveMessage, NerveTransport};

    #[test]
    fn keys_stay_sorted_after_many_inserts() {
        let mut index = PartitionIndex::with_order(4).expect("index");
        // A spread order forces splits on both sides of the tree.
        let mut start = 0u64;
        let mut inserted = Vec::new();
        for i in 0..500u64 {
            start = start.wrapping_mul(31).wrapping_add(7) % 100_000;
            if index.insert(start, i as usize).is_ok() {
                inserted.push(start);
            }
        }
        inserted.sort_unstable();
        inserted.dedup();
        assert_eq!(index.len(), inserted.len());
        assert_eq!(index.keys(), inserted);
        assert!(index.node_count() > 1, "the tree must split");
    }

    #[test]
    fn floor_finds_the_owning_range_across_leaves() {
        let mut index = PartitionIndex::with_order(4).expect("index");
        for i in 0..200u64 {
            index.insert(i * 10, i as usize).expect("insert");
        }
        // Each range covers ten partitions.
        for i in 0..200u64 {
            assert_eq!(index.floor(i * 10), Some((i * 10, i as usize)));
            assert_eq!(index.floor(i * 10 + 9), Some((i * 10, i as usize)));
        }
        // A partition below the first start has no floor.
        assert_eq!(index.floor(0), Some((0, 0)));
        assert_eq!(index.lookup(1990), Some(199));
        assert_eq!(index.lookup(1991), None);
    }

    #[test]
    fn duplicate_start_is_rejected() {
        let mut index = PartitionIndex::new();
        index.insert(5, 1).expect("first insert");
        assert_eq!(index.insert(5, 2), Err(IndexError::DuplicateKey(5)));
    }

    #[test]
    fn order_below_four_is_rejected() {
        assert_eq!(PartitionIndex::with_order(3).err(), Some(IndexError::BadOrder(3)));
    }

    #[test]
    fn ranges_route_each_partition_to_its_owner() {
        let policy = PartitionRanges::from_ranges(&[(0, 0), (8, 1), (16, 2), (24, 3)]).expect("ranges");
        assert_eq!(policy.cores(), 4);
        assert_eq!(policy.ranges(), 4);
        assert_eq!(policy.core_for(0), 0);
        assert_eq!(policy.core_for(7), 0);
        assert_eq!(policy.core_for(8), 1);
        assert_eq!(policy.core_for(15), 1);
        assert_eq!(policy.core_for(16), 2);
        assert_eq!(policy.core_for(23), 2);
        assert_eq!(policy.core_for(24), 3);
        assert_eq!(policy.core_for(1_000_000), 3);
    }

    #[test]
    fn ranges_reject_a_bad_list() {
        assert_eq!(PartitionRanges::from_ranges(&[]).err(), Some(IndexError::MissingOrigin));
        assert_eq!(
            PartitionRanges::from_ranges(&[(4, 0), (8, 1)]).err(),
            Some(IndexError::MissingOrigin)
        );
        assert_eq!(
            PartitionRanges::from_ranges(&[(0, 0), (8, 1), (8, 2)]).err(),
            Some(IndexError::UnsortedRange(8))
        );
    }

    #[test]
    fn router_uses_the_b_tree_routing() {
        let policy = PartitionRanges::from_ranges(&[(0, 0), (100, 1), (200, 2)]).expect("ranges");
        let router = LockFreeNerveRouter::new(policy, 16);
        router.send(NerveMessage::new(0, 1)).expect("send p0");
        router.send(NerveMessage::new(150, 2)).expect("send p150");
        router.send(NerveMessage::new(250, 3)).expect("send p250");
        assert_eq!(router.pending(0), Ok(1));
        assert_eq!(router.pending(1), Ok(1));
        assert_eq!(router.pending(2), Ok(1));
        assert_eq!(router.recv(2).expect("recv").cell_id, 3);
    }
}
