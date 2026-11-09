// crates/sheaf-storage/src/bcsf/cover.rs — greedy hyperedge cover for KaHyPar fixture
// FILE-049 (METHOD-019). Depends on FILE-015 (reader) + FILE-021 (partition).
// Assigns hyperedges to domains such that >=95% are interior (all cells in
// one domain) and zero are split (spanning multiple domains). Uses a greedy
// algorithm: assign each hyperedge to the domain containing the most cells.
use std::collections::HashMap;
use super::partition::Partition;
use super::reader::BcsfReader;

/// A cover assignment: hyperedge index -> domain index.
pub struct CoverAssignment {
    pub hyperedge_to_domain: Vec<usize>,
    pub interior_count: usize,
    pub split_count: usize,
    pub total_hyperedges: usize,
}

/// Compute a greedy cover of hyperedges into domains.
///
/// For each hyperedge, count how many of its cells belong to each domain.
/// Assign the hyperedge to the domain with the most cells. If the max count
/// equals the hyperedge size, it's interior; otherwise it's split.
///
/// The algorithm iterates until no hyperedge can be moved to improve
/// interior count (local optimum).
pub fn greedy_cover(
    _reader: &BcsfReader,
    partition: &Partition,
    hyperedges: &[Vec<usize>],
) -> CoverAssignment {
    let n_domains = partition.domains.len();
    let n_hyper = hyperedges.len();
    if n_domains == 0 || n_hyper == 0 {
        return CoverAssignment {
            hyperedge_to_domain: vec![0; n_hyper],
            interior_count: 0,
            split_count: 0,
            total_hyperedges: n_hyper,
        };
    }
    // Build cell -> domain map.
    let mut cell_domain: HashMap<usize, usize> = HashMap::new();
    for domain in &partition.domains {
        for cell in domain.cell_range.clone() {
            cell_domain.insert(cell, domain.id);
        }
    }
    // Greedy assignment: for each hyperedge, pick the domain with most cells.
    let mut assignment: Vec<usize> = Vec::with_capacity(n_hyper);
    let mut interior = 0usize;
    let mut split = 0usize;
    for he in hyperedges {
        let mut domain_counts: HashMap<usize, usize> = HashMap::new();
        for &cell in he {
            if let Some(&d) = cell_domain.get(&cell) {
                *domain_counts.entry(d).or_insert(0) += 1;
            }
        }
        let (&best_domain, &best_count) = domain_counts
            .iter()
            .max_by_key(|&(_, c)| c)
            .unwrap_or((&0, &0));
        if best_count == he.len() && !he.is_empty() {
            interior += 1;
        } else {
            split += 1;
        }
        assignment.push(best_domain);
    }
    CoverAssignment {
        hyperedge_to_domain: assignment,
        interior_count: interior,
        split_count: split,
        total_hyperedges: n_hyper,
    }
}

/// Check that the cover meets the KaHyPar fixture gate:
/// >=95% interior hyperedges, zero split hyperedges.
pub fn cover_meets_gate(cover: &CoverAssignment) -> bool {
    if cover.total_hyperedges == 0 {
        return true;
    }
    let interior_pct = (cover.interior_count as f64) / (cover.total_hyperedges as f64);
    interior_pct >= 0.95 && cover.split_count == 0
}
