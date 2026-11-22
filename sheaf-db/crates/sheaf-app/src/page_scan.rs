// page_scan.rs - the stored page container and the pruned scan (NF-12).
//
// A table is a list of pages. Each page holds a packed spine and a payload of
// length-prefixed JSON rows. The page carries its column summary, so the scan
// can test a predicate against the summary and skip the page before any row is
// decoded.
//
// The scan always reports what it did: the page count, the skipped count, the
// decoded row count, and the number of summary clauses. Nothing is hidden.

use crate::model::*;
use crate::page_stats::{PagePlan, PageStats};
use crate::storage::validate_row;
use sheaf_native::storage::{StorageBackend, SPINE_BYTES};

/// One stored page: the spine, the payload, and the column summary.
pub struct Page {
    pub spine: [u8; SPINE_BYTES],
    pub payload: Vec<u8>,
    pub stats: PageStats,
}

impl Page {
    /// Decode every row of this page. The row hash and the schema are checked.
    pub fn decode(
        &self,
        backend: &dyn StorageBackend,
        columns: &[Column],
        rows: &mut Vec<Row>,
        bytes: &mut usize,
    ) -> Result<()> {
        for slot in 0..backend.spine_count(&self.spine) {
            let entry = backend.spine_get(&self.spine, slot).ok_or("invalid spine entry")?;
            let offset = usize::try_from(entry.byte_offset).map_err(|_| "invalid spine offset")?;
            let size = self
                .payload
                .get(offset..offset.checked_add(4).ok_or("offset overflow")?)
                .ok_or("invalid row offset")?;
            let len = u32::from_le_bytes(size.try_into().unwrap()) as usize;
            let end = offset
                .checked_add(4)
                .and_then(|value| value.checked_add(len))
                .ok_or("row length overflow")?;
            let raw = self.payload.get(offset + 4..end).ok_or("invalid row length")?;
            if blake3::hash(raw).as_bytes()[..16] != entry.stalk_hash {
                return Err("row checksum mismatch".into());
            }
            let row: Row = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
            validate_row(columns, &row)?;
            *bytes += row.iter().map(Value::bytes).sum::<usize>();
            if *bytes > MEMORY_LIMIT {
                return Err("decoded table memory limit exceeded".into());
            }
            rows.push(row);
        }
        Ok(())
    }
}

/// Scan every page the summary keeps, and report the page decisions.
pub fn scan(
    pages: &[Page],
    plan: &PagePlan,
    backend: &dyn StorageBackend,
    columns: &[Column],
    count: usize,
) -> Result<(Vec<Row>, ScanStats)> {
    let mut rows = Vec::with_capacity(count);
    let mut bytes = 0usize;
    let mut stats = ScanStats { pages_total: pages.len(), page_clauses: plan.clauses(), ..ScanStats::default() };
    for page in pages {
        if !plan.might_match(&page.stats) {
            stats.pages_skipped += 1;
            continue;
        }
        page.decode(backend, columns, &mut rows, &mut bytes)?;
    }
    stats.rows_decoded = rows.len();
    Ok((rows, stats))
}
