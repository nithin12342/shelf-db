// topology/mod.rs - physical L3 cache discovery (MF-04, spec Pillar 2).
//
// The engine keeps one partition inside one shared L3 domain, so a query stage
// does not cross a socket boundary. The domain layout is physical, so it comes
// from the operating system and never from a fixed cores-per-domain constant.
//
// The port is CacheTopology. Three adapters implement it:
//   windows.rs  GetLogicalProcessorInformationEx with RelationCache
//   linux.rs    the sysfs cache index files
//   this file   SystemCacheTopology, which dispatches at compile time
//
// Discovery validates its own result. A logical CPU that appears in two
// domains, or in none, is an error, not a guess.

use std::collections::BTreeMap;
use std::fmt;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;
#[cfg(windows)]
mod windows;

pub mod map;
pub mod sysfs_parse;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(not(any(windows, target_os = "linux")))]
use unsupported as platform;
#[cfg(windows)]
use windows as platform;

/// One shared L3 cache domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct L3Domain {
    /// The domain number, assigned in ascending order of the first CPU.
    pub id: usize,
    /// The logical CPUs that share this cache, in ascending order.
    pub cpus: Vec<usize>,
    /// The cache size in bytes. Zero means the platform did not report it.
    pub size_bytes: u64,
}

impl L3Domain {
    /// Build a domain. The CPU list is sorted and deduplicated.
    pub fn new(id: usize, mut cpus: Vec<usize>, size_bytes: u64) -> Self {
        cpus.sort_unstable();
        cpus.dedup();
        Self { id, cpus, size_bytes }
    }

    /// True when the domain holds `cpu`.
    pub fn contains(&self, cpu: usize) -> bool {
        self.cpus.binary_search(&cpu).is_ok()
    }

    /// The number of logical CPUs in the domain.
    pub fn width(&self) -> usize {
        self.cpus.len()
    }
}

/// Why cache discovery failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopologyError {
    /// The target has no cache topology API.
    Unsupported(String),
    /// The operating system reported no level 3 unified cache.
    NoL3Cache,
    /// An operating system call failed.
    OsError { what: &'static str, code: i32 },
    /// A cache file could not be read or parsed.
    Io { path: String, detail: String },
    /// A logical CPU appears in more than one domain.
    CpuInTwoDomains(usize),
    /// An allowed logical CPU appears in no domain.
    CpuInNoDomain(usize),
    /// A domain holds a CPU that the process may not use.
    CpuNotAllowed(usize),
    /// A cell has no owning core in the supplied table.
    UnmappedCell(u64),
    /// A core is in no L3 domain.
    CoreNotInAnyDomain(usize),
    /// A partition holds no cell, so it has no domain.
    EmptyPartition,
}

impl fmt::Display for TopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopologyError::Unsupported(target) => {
                write!(f, "cache topology is not supported on {target}")
            }
            TopologyError::NoL3Cache => write!(f, "the host reports no level 3 unified cache"),
            TopologyError::OsError { what, code } => {
                write!(f, "{what} failed with error {code}")
            }
            TopologyError::Io { path, detail } => write!(f, "{path}: {detail}"),
            TopologyError::CpuInTwoDomains(cpu) => {
                write!(f, "cpu {cpu} appears in more than one L3 domain")
            }
            TopologyError::CpuInNoDomain(cpu) => {
                write!(f, "cpu {cpu} appears in no L3 domain")
            }
            TopologyError::CpuNotAllowed(cpu) => {
                write!(f, "cpu {cpu} is outside the allowed set")
            }
            TopologyError::UnmappedCell(cell) => write!(f, "cell {cell} has no owning core"),
            TopologyError::CoreNotInAnyDomain(core) => {
                write!(f, "core {core} is in no L3 domain")
            }
            TopologyError::EmptyPartition => write!(f, "the partition holds no cell"),
        }
    }
}

impl std::error::Error for TopologyError {}

/// The cache topology port. Callers depend on this trait, so a new platform is
/// a new adapter.
pub trait CacheTopology {
    /// The L3 domains of this host, in ascending order of the first CPU.
    fn l3_domains(&self) -> Result<Vec<L3Domain>, TopologyError>;
}

/// The operating system adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCacheTopology;

impl SystemCacheTopology {
    pub fn new() -> Self {
        Self
    }
}

impl CacheTopology for SystemCacheTopology {
    fn l3_domains(&self) -> Result<Vec<L3Domain>, TopologyError> {
        let domains = platform::l3_domains()?;
        if domains.is_empty() {
            return Err(TopologyError::NoL3Cache);
        }
        check_exactly_once(&domains)?;
        Ok(domains)
    }
}

/// Check that every reported CPU appears in exactly one domain.
pub fn check_exactly_once(domains: &[L3Domain]) -> Result<(), TopologyError> {
    let mut seen: BTreeMap<usize, usize> = BTreeMap::new();
    for domain in domains {
        for &cpu in &domain.cpus {
            let count = seen.entry(cpu).or_insert(0);
            *count += 1;
            if *count > 1 {
                return Err(TopologyError::CpuInTwoDomains(cpu));
            }
        }
    }
    Ok(())
}

/// Check the domains against the CPU set that the process may use.
///
/// The union must equal `allowed` and the domains must be disjoint. This is the
/// gate that a partition planner uses before it assigns work to a domain.
pub fn validate_against_allowed(
    domains: &[L3Domain],
    allowed: &[usize],
) -> Result<(), TopologyError> {
    check_exactly_once(domains)?;
    for domain in domains {
        for &cpu in &domain.cpus {
            if allowed.binary_search(&cpu).is_err() {
                return Err(TopologyError::CpuNotAllowed(cpu));
            }
        }
    }
    for &cpu in allowed {
        if !domains.iter().any(|d| d.contains(cpu)) {
            return Err(TopologyError::CpuInNoDomain(cpu));
        }
    }
    Ok(())
}

/// Build a domain list from cache records, keeping the first record of each
/// distinct CPU set. The platform adapters call this so the id rule lives in
/// one place: ascending order of the first CPU.
pub(crate) fn build_domains(mut records: Vec<(Vec<usize>, u64)>) -> Vec<L3Domain> {
    records.sort_by_key(|(cpus, _)| cpus.first().copied().unwrap_or(usize::MAX));
    let mut domains: Vec<L3Domain> = Vec::with_capacity(records.len());
    for (cpus, size) in records {
        if domains.iter().any(|d| d.cpus == cpus) {
            continue;
        }
        let id = domains.len();
        domains.push(L3Domain::new(id, cpus, size));
    }
    domains
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_runtime::pinning::{OsThreadPinner, ThreadPinner};

    #[test]
    fn discovery_returns_at_least_one_domain() {
        let domains = SystemCacheTopology::new().l3_domains().expect("l3 discovery");
        assert!(!domains.is_empty());
        for (index, domain) in domains.iter().enumerate() {
            assert_eq!(domain.id, index, "ids must be dense and ordered");
            assert!(!domain.cpus.is_empty());
        }
    }

    #[test]
    fn domains_are_disjoint_and_cover_the_allowed_set() {
        let mut allowed = OsThreadPinner::new().allowed_cores().expect("allowed cores");
        allowed.sort_unstable();
        let domains = SystemCacheTopology::new().l3_domains().expect("l3 discovery");
        validate_against_allowed(&domains, &allowed).expect("domains must match the allowed set");

        let mut union: Vec<usize> = domains.iter().flat_map(|d| d.cpus.iter().copied()).collect();
        union.sort_unstable();
        assert_eq!(union, allowed, "the union must equal the allowed CPU set");
    }

    #[test]
    fn a_cpu_in_two_domains_is_rejected() {
        let domains = vec![
            L3Domain::new(0, vec![0, 1], 1024),
            L3Domain::new(1, vec![1, 2], 1024),
        ];
        assert_eq!(check_exactly_once(&domains), Err(TopologyError::CpuInTwoDomains(1)));
    }

    #[test]
    fn a_cpu_outside_the_allowed_set_is_rejected() {
        let domains = vec![L3Domain::new(0, vec![0, 1], 1024)];
        assert_eq!(
            validate_against_allowed(&domains, &[0]),
            Err(TopologyError::CpuNotAllowed(1))
        );
        assert_eq!(
            validate_against_allowed(&domains, &[0, 1, 2]),
            Err(TopologyError::CpuInNoDomain(2))
        );
    }

    #[test]
    fn build_keeps_one_domain_per_cpu_set_in_first_cpu_order() {
        let records = vec![
            (vec![4, 5, 6, 7], 8192),
            (vec![0, 1, 2, 3], 8192),
            (vec![4, 5, 6, 7], 8192),
        ];
        let domains = build_domains(records);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].cpus, vec![0, 1, 2, 3]);
        assert_eq!(domains[1].cpus, vec![4, 5, 6, 7]);
        assert_eq!(domains[1].id, 1);
    }
}
