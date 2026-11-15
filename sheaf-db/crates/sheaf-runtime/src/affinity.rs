// affinity.rs - thread-per-core NUMA topology, spec Pillar 4.
//
// Workers are pinned to hardware threads. Each worker owns a core-local
// execution queue, so cache lines do not bounce across sockets. This module
// defines the topology and the pinning policy as a port (trait). The concrete
// adapter maps a partition to a core with a stable modulo rule. An OS-specific
// adapter can replace it to call core_affinity without changing callers.
//
// The spec also groups cores into shared L3 domains (the 64 MB nerve-routing
// boundary). The topology exposes that grouping so the router can keep
// intermediate references inside one domain.

/// The pinning policy port. Callers depend on this trait, not on a concrete
/// topology, so the scheduling rule can change without touching the router.
pub trait PinningPolicy {
    /// Number of hardware workers (cores) in the topology.
    fn cores(&self) -> usize;
    /// The core that owns `partition`. The mapping must be stable.
    fn core_for(&self, partition: u64) -> usize;
}

/// A flat thread-per-core topology with optional L3 domain grouping.
#[derive(Clone, Debug)]
pub struct CoreTopology {
    cores: usize,
    cores_per_domain: usize,
}

impl CoreTopology {
    /// Build a topology with `cores` workers. `cores_per_domain` groups cores
    /// into shared L3 domains. Both values clamp to at least one.
    pub fn new(cores: usize, cores_per_domain: usize) -> Self {
        Self { cores: cores.max(1), cores_per_domain: cores_per_domain.max(1) }
    }

    /// A single-domain topology with `cores` workers.
    pub fn single_domain(cores: usize) -> Self {
        let c = cores.max(1);
        Self { cores: c, cores_per_domain: c }
    }

    /// Number of L3 domains. The last domain can be partial.
    pub fn domains(&self) -> usize {
        self.cores.div_ceil(self.cores_per_domain)
    }

    /// The L3 domain that owns `core`.
    pub fn domain_for(&self, core: usize) -> usize {
        core / self.cores_per_domain
    }

    /// True when two cores share an L3 domain and can exchange references
    /// without crossing a socket boundary.
    pub fn same_domain(&self, a: usize, b: usize) -> bool {
        self.domain_for(a) == self.domain_for(b)
    }
}

impl PinningPolicy for CoreTopology {
    fn cores(&self) -> usize {
        self.cores
    }

    fn core_for(&self, partition: u64) -> usize {
        // Stable modulo mapping. Adjacent partitions spread across cores,
        // which balances load while keeping each partition on one worker.
        (partition % self.cores as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulo_mapping_is_stable_and_in_range() {
        let topo = CoreTopology::single_domain(4);
        assert_eq!(topo.cores(), 4);
        for p in 0u64..16 {
            let core = topo.core_for(p);
            assert!(core < 4);
            assert_eq!(core, topo.core_for(p));
        }
        assert_eq!(topo.core_for(0), 0);
        assert_eq!(topo.core_for(5), 1);
    }

    #[test]
    fn zero_cores_clamps_to_one() {
        let topo = CoreTopology::new(0, 0);
        assert_eq!(topo.cores(), 1);
        assert_eq!(topo.core_for(99), 0);
        assert_eq!(topo.domains(), 1);
    }

    #[test]
    fn domains_group_cores() {
        // 8 cores, 4 per domain -> 2 domains.
        let topo = CoreTopology::new(8, 4);
        assert_eq!(topo.domains(), 2);
        assert_eq!(topo.domain_for(0), 0);
        assert_eq!(topo.domain_for(3), 0);
        assert_eq!(topo.domain_for(4), 1);
        assert!(topo.same_domain(1, 2));
        assert!(!topo.same_domain(2, 5));
    }

    #[test]
    fn partial_last_domain_counts() {
        // 5 cores, 2 per domain -> 3 domains (2 + 2 + 1).
        let topo = CoreTopology::new(5, 2);
        assert_eq!(topo.domains(), 3);
        assert_eq!(topo.domain_for(4), 2);
    }
}