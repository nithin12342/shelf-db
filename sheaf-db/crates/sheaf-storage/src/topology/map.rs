// topology/map.rs - maps a BCSF partition to the L3 domain that holds it.
//
// A partition domain owns a contiguous cell range and holds ghost replicas of
// neighbouring cells. The L3 domain that holds most of those cells is the
// domain that should run the partition, because then the working set stays in
// one shared cache.
//
// The map is deterministic. Cell counts are compared against domain ids, and a
// tie goes to the lowest domain id, so the answer never depends on iteration
// order.

use super::{L3Domain, TopologyError};
use crate::bcsf::partition::Domain as PartitionDomain;

/// The port that names the core that owns a cell.
pub trait CellOwner {
    /// The core that owns `cell`, or None when the cell is unknown.
    fn core_of(&self, cell: u64) -> Option<usize>;
}

impl<F> CellOwner for F
where
    F: Fn(u64) -> Option<usize>,
{
    fn core_of(&self, cell: u64) -> Option<usize> {
        self(cell)
    }
}

/// A dense table from cell number to core number.
#[derive(Clone, Debug, Default)]
pub struct CellCoreTable {
    cores: Vec<usize>,
}

impl CellCoreTable {
    /// Build a table. Cell `i` is owned by `cores[i]`.
    pub fn new(cores: Vec<usize>) -> Self {
        Self { cores }
    }

    /// The number of cells in the table.
    pub fn len(&self) -> usize {
        self.cores.len()
    }

    /// True when the table holds no cell.
    pub fn is_empty(&self) -> bool {
        self.cores.is_empty()
    }
}

impl CellOwner for CellCoreTable {
    fn core_of(&self, cell: u64) -> Option<usize> {
        usize::try_from(cell).ok().and_then(|index| self.cores.get(index).copied())
    }
}

/// Maps cell sets to L3 domains.
pub struct PartitionLocator<'a> {
    domains: &'a [L3Domain],
    /// Core number to domain index. A core in no domain holds None.
    core_domain: Vec<Option<usize>>,
}

impl<'a> PartitionLocator<'a> {
    /// Bind the locator to a discovered domain list.
    pub fn new(domains: &'a [L3Domain]) -> Result<Self, TopologyError> {
        let max_core = domains.iter().flat_map(|d| d.cpus.iter().copied()).max();
        let max_core = match max_core {
            Some(core) => core,
            None => return Err(TopologyError::NoL3Cache),
        };
        let mut core_domain = vec![None; max_core + 1];
        for (index, domain) in domains.iter().enumerate() {
            for &cpu in &domain.cpus {
                if core_domain[cpu].is_none() {
                    core_domain[cpu] = Some(index);
                }
            }
        }
        Ok(Self { domains, core_domain })
    }

    /// The number of domains in the map.
    pub fn domains(&self) -> usize {
        self.domains.len()
    }

    /// The domain id that holds the most of `cells`. A tie goes to the lowest
    /// domain id. An empty cell set is an error, because it has no answer.
    pub fn domain_for<I>(&self, cells: I, owner: &dyn CellOwner) -> Result<usize, TopologyError>
    where
        I: IntoIterator<Item = u64>,
    {
        let mut counts = vec![0usize; self.domains.len()];
        let mut seen = 0usize;
        for cell in cells {
            let core = owner.core_of(cell).ok_or(TopologyError::UnmappedCell(cell))?;
            let slot = self
                .core_domain
                .get(core)
                .copied()
                .flatten()
                .ok_or(TopologyError::CoreNotInAnyDomain(core))?;
            counts[slot] += 1;
            seen += 1;
        }
        if seen == 0 {
            return Err(TopologyError::EmptyPartition);
        }

        // Pick the highest count. A tie keeps the lower domain id.
        let mut best: Option<(usize, usize)> = None;
        for (index, domain) in self.domains.iter().enumerate() {
            let count = counts[index];
            match best {
                None => best = Some((domain.id, count)),
                Some((best_id, best_count)) => {
                    if count > best_count || (count == best_count && domain.id < best_id) {
                        best = Some((domain.id, count));
                    }
                }
            }
        }
        best.map(|(id, _)| id).ok_or(TopologyError::NoL3Cache)
    }

    /// The domain id that holds the most of a BCSF partition domain. The cell
    /// set is the owned range plus the ghost replicas of neighbours.
    pub fn domain_for_partition(
        &self,
        partition: &PartitionDomain,
        owner: &dyn CellOwner,
    ) -> Result<usize, TopologyError> {
        let owned = partition.cell_range.clone().map(|cell| cell as u64);
        let ghosts = partition.ghosts.iter().map(|(_, remote)| *remote as u64);
        self.domain_for(owned.chain(ghosts), owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domains() -> Vec<L3Domain> {
        vec![
            L3Domain::new(0, vec![0, 1], 1024),
            L3Domain::new(1, vec![2, 3], 1024),
            L3Domain::new(2, vec![4, 5], 1024),
        ]
    }

    #[test]
    fn the_largest_overlap_wins() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        // Cores 2, 2, 2, 0 put three cells in domain 1 and one in domain 0.
        let owner = CellCoreTable::new(vec![2, 2, 2, 0]);
        assert_eq!(locator.domain_for(0..4u64, &owner), Ok(1));
    }

    #[test]
    fn a_tie_goes_to_the_lowest_domain_id() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        // Domain 0 holds cells 0 and 1. Domain 1 holds cells 2 and 3.
        let owner = CellCoreTable::new(vec![0, 0, 2, 2]);
        assert_eq!(locator.domain_for(0..4u64, &owner), Ok(0));
    }

    #[test]
    fn a_partition_maps_by_owned_cells_and_ghosts() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        // Owned range 0..2 is on core 0. The ghost remote 4 is on core 4,
        // which is domain 2. Domain 0 still holds the larger share.
        let partition = PartitionDomain { id: 0, cell_range: 0..2, ghosts: vec![(2, 4)] };
        let owner = CellCoreTable::new(vec![0, 0, 0, 0, 4]);
        assert_eq!(locator.domain_for_partition(&partition, &owner), Ok(0));
    }

    #[test]
    fn an_unknown_cell_is_reported() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        let owner = CellCoreTable::new(vec![0]);
        assert_eq!(locator.domain_for(0..2u64, &owner), Err(TopologyError::UnmappedCell(1)));
    }

    #[test]
    fn a_core_in_no_domain_is_reported() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        let owner = CellCoreTable::new(vec![9]);
        assert_eq!(locator.domain_for(0..1u64, &owner), Err(TopologyError::CoreNotInAnyDomain(9)));
    }

    #[test]
    fn an_empty_cell_set_is_rejected() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        let owner = CellCoreTable::new(vec![0]);
        assert_eq!(locator.domain_for(Vec::new(), &owner), Err(TopologyError::EmptyPartition));
    }

    #[test]
    fn an_empty_domain_list_is_rejected() {
        let empty: Vec<L3Domain> = Vec::new();
        assert_eq!(PartitionLocator::new(&empty).err(), Some(TopologyError::NoL3Cache));
    }

    #[test]
    fn a_closure_is_a_cell_owner() {
        let domains = domains();
        let locator = PartitionLocator::new(&domains).expect("locator");
        let owner = |cell: u64| Some((cell % 6) as usize);
        // Cells 0..6 cycle through every core, so each domain holds two cells
        // and the tie falls to the lowest id.
        assert_eq!(locator.domain_for(0..6u64, &owner), Ok(0));
    }
}
