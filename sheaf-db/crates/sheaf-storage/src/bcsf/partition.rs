// crates/sheaf-storage/src/bcsf/partition.rs — split hypergraph into L3-sized domains
// FILE-021 (METHOD-015). Depends on FILE-015 (reader).
// Partitions cells into contiguous domains of ~64MB (L3 cache size) for
// cache-efficient local solves. Ghost replicas serve cross-socket boundary
// reads locally instead of fetching remote memory.
use super::reader::BcsfReader;

/// A partition domain: contiguous cell range + ghost replicas of boundary cells.
pub struct Domain {
    pub id: usize,
    pub cell_range: std::ops::Range<usize>,
    /// Ghost replicas: (local_cell_id, remote_cell_id) pairs.
    pub ghosts: Vec<(usize, usize)>,
}

/// A partitioned hypergraph with L3-sized domains.
pub struct Partition {
    pub domains: Vec<Domain>,
    pub domain_bytes: Vec<usize>,
}

/// Partition a BCSF reader into domains of at most `target_bytes` each.
/// Uses a greedy contiguous split: accumulate cells until the domain would
/// exceed the target, then start a new domain. Ghost replicas are cells
/// referenced by a domain's boundary but owned by another domain.
pub fn partition_reader(reader: &BcsfReader, target_bytes: usize) -> Partition {
    let n = reader.len();
    if n == 0 {
        return Partition { domains: Vec::new(), domain_bytes: Vec::new() };
    }
    // Estimate per-cell bytes from the reader.
    let total_bytes = reader.resident_bytes();
    let avg_cell_bytes = total_bytes / n;
    let cells_per_domain = (target_bytes / avg_cell_bytes.max(1)).max(1);
    let mut domains: Vec<Domain> = Vec::new();
    let mut domain_bytes: Vec<usize> = Vec::new();
    let mut start = 0usize;
    let mut domain_id = 0usize;
    while start < n {
        let end = (start + cells_per_domain).min(n);
        // Find ghost replicas: boundary cells of this domain that belong
        // to other domains.
        let mut ghosts: Vec<(usize, usize)> = Vec::new();
        for cell in start..end {
            for &b in reader.boundary(cell) {
                let b = b as usize;
                if b < start || b >= end {
                    // Ghost: local alias for remote cell.
                    let local_alias = end + ghosts.len();
                    ghosts.push((local_alias, b));
                }
            }
        }
        domains.push(Domain {
            id: domain_id,
            cell_range: start..end,
            ghosts,
        });
        domain_bytes.push((end - start) * avg_cell_bytes);
        start = end;
        domain_id += 1;
    }
    Partition { domains, domain_bytes }
}

/// Check that all domains are within the target byte budget.
pub fn domains_fit_l3(partition: &Partition, target_bytes: usize) -> bool {
    partition.domain_bytes.iter().all(|&b| b <= target_bytes)
}

/// Check that every boundary reference resolves to either a local cell or
/// a ghost replica (no dangling cross-domain references).
pub fn ghosts_cover_boundaries(reader: &BcsfReader, partition: &Partition) -> bool {
    for domain in &partition.domains {
        for cell in domain.cell_range.clone() {
            for &b in reader.boundary(cell) {
                let b = b as usize;
                let is_local = domain.cell_range.contains(&b);
                let is_ghost = domain.ghosts.iter().any(|(_, remote)| *remote == b);
                if !is_local && !is_ghost {
                    return false;
                }
            }
        }
    }
    true
}

/// MAX-PERF P6: ghost membership as a dense bitset over the global cell id
/// space. O(1) branchless membership test vs linear scan of the ghost vec.
/// Space: n_cells bits per domain (vs 16 B per ghost pair).
pub fn ghost_bitmask(domain: &Domain, n_cells: usize) -> Vec<u64> {
    let mut mask = vec![0u64; n_cells.div_ceil(64)];
    for &(_, remote) in &domain.ghosts {
        if remote < n_cells {
            mask[remote / 64] |= 1u64 << (remote % 64);
        }
    }
    mask
}

/// Branchless ghost test: 1 bit, no pointer chase.
pub fn is_ghost(mask: &[u64], cell: usize) -> bool {
    match mask.get(cell / 64) {
        Some(w) => (w >> (cell % 64)) & 1 == 1,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// B10: persisted partition-map sidecar. The L3-domain assignment and the
// ghost map used to live only in RAM and were recomputed on every process
// start. They now serialize to a self-describing sidecar so the plan
// SURVIVES process exit.
//
// Layout: [MAGIC u64][fmt_ver u32][generation u64][src_fingerprint u64]
//         [body_len u64][crc32c u32][body]
// body:   [n_domains u64][n_cells u64][target_bytes u64]
//         per domain: [id u64][range_start u64][range_end u64]
//                     [domain_bytes u64][n_ghosts u64]
//                     per ghost: [local u64][remote u64]
// ---------------------------------------------------------------------------

/// Sidecar magic ("NIMXPMAP").
pub const PARTITION_SIDECAR_MAGIC: u64 = 0x4E49_4D58_504D_4150;
/// Sidecar format version.
pub const PARTITION_SIDECAR_FMT_VER: u32 = 1;
/// Sidecar header length: magic|ver|generation|fingerprint|body_len|crc.
pub const PARTITION_SIDECAR_HEADER_LEN: usize = 8 + 4 + 8 + 8 + 8 + 4;

impl Partition {
    /// Serialize the partition map into a sidecar image.
    pub fn to_sidecar(&self, generation: u64, src_fingerprint: u64) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        let total_cells: usize = self.domains.iter().map(|d| d.cell_range.len()).sum();
        let max_bytes = self.domain_bytes.iter().copied().max().unwrap_or(0);
        body.extend_from_slice(&(self.domains.len() as u64).to_le_bytes());
        body.extend_from_slice(&(total_cells as u64).to_le_bytes());
        body.extend_from_slice(&(max_bytes as u64).to_le_bytes());
        for d in &self.domains {
            body.extend_from_slice(&(d.id as u64).to_le_bytes());
            body.extend_from_slice(&(d.cell_range.start as u64).to_le_bytes());
            body.extend_from_slice(&(d.cell_range.end as u64).to_le_bytes());
            body.extend_from_slice(&(
                self.domain_bytes
                    .get(d.id)
                    .copied()
                    .unwrap_or(0) as u64
            )
            .to_le_bytes());
            body.extend_from_slice(&(d.ghosts.len() as u64).to_le_bytes());
            for (local, remote) in &d.ghosts {
                body.extend_from_slice(&(*local as u64).to_le_bytes());
                body.extend_from_slice(&(*remote as u64).to_le_bytes());
            }
        }
        let crc = crate::lsp::ring_wal::crc_hw(&body);
        let mut out = Vec::with_capacity(PARTITION_SIDECAR_HEADER_LEN + body.len());
        out.extend_from_slice(&PARTITION_SIDECAR_MAGIC.to_le_bytes());
        out.extend_from_slice(&PARTITION_SIDECAR_FMT_VER.to_le_bytes());
        out.extend_from_slice(&generation.to_le_bytes());
        out.extend_from_slice(&src_fingerprint.to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Load a partition map from a sidecar image (the process-restart path).
    /// Fails loud on any mismatch - never a partial load.
    pub fn from_sidecar(bytes: &[u8]) -> Result<(Partition, u64, u64), String> {
        if bytes.len() < PARTITION_SIDECAR_HEADER_LEN {
            return Err("partition sidecar: truncated header".to_string());
        }
        if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != PARTITION_SIDECAR_MAGIC {
            return Err("partition sidecar: bad magic".to_string());
        }
        let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if ver != PARTITION_SIDECAR_FMT_VER {
            return Err(format!(
                "partition sidecar: format version {ver} != {PARTITION_SIDECAR_FMT_VER}"
            ));
        }
        let generation = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let fingerprint = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let body_len = u64::from_le_bytes(bytes[28..36].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        let body = bytes
            .get(PARTITION_SIDECAR_HEADER_LEN..PARTITION_SIDECAR_HEADER_LEN + body_len)
            .ok_or("partition sidecar: body truncated")?;
        if crate::lsp::ring_wal::crc_hw(body) != crc {
            return Err("partition sidecar: body crc mismatch (corrupt file)".to_string());
        }
        let mut p = 0usize;
        let u64_at = |p: &mut usize| -> Result<u64, String> {
            let s = *p;
            let e = s + 8;
            if e > body.len() {
                return Err("partition sidecar: truncated u64".to_string());
            }
            *p = e;
            Ok(u64::from_le_bytes(body[s..e].try_into().unwrap()))
        };
        let n_domains = u64_at(&mut p)? as usize;
        let _total_cells = u64_at(&mut p)?;
        let _max_bytes = u64_at(&mut p)?;
        let mut domains: Vec<Domain> = Vec::with_capacity(n_domains);
        let mut domain_bytes: Vec<usize> = Vec::with_capacity(n_domains);
        for _ in 0..n_domains {
            let id = u64_at(&mut p)? as usize;
            let start = u64_at(&mut p)? as usize;
            let end = u64_at(&mut p)? as usize;
            let bytes = u64_at(&mut p)? as usize;
            let n_ghosts = u64_at(&mut p)? as usize;
            let mut ghosts: Vec<(usize, usize)> = Vec::with_capacity(n_ghosts);
            for _ in 0..n_ghosts {
                let local = u64_at(&mut p)? as usize;
                let remote = u64_at(&mut p)? as usize;
                ghosts.push((local, remote));
            }
            domains.push(Domain { id, cell_range: start..end, ghosts });
            domain_bytes.push(bytes);
        }
        if p != body.len() {
            return Err("partition sidecar: trailing bytes in body".to_string());
        }
        Ok((Partition { domains, domain_bytes }, generation, fingerprint))
    }

    /// Write the partition sidecar to disk.
    pub fn write_sidecar(
        &self,
        path: &std::path::Path,
        generation: u64,
        src_fingerprint: u64,
    ) -> Result<(), String> {
        let bytes = self.to_sidecar(generation, src_fingerprint);
        std::fs::write(path, &bytes).map_err(|e| format!("{path:?}: {e}"))
    }

    /// Read a partition sidecar from disk (the process-restart path).
    pub fn read_sidecar(
        path: &std::path::Path,
    ) -> Result<(Partition, u64, u64), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
        Partition::from_sidecar(&bytes)
    }
}
