// storage.rs - the table catalog and the page store (MF-07, NF-12).
//
// A table validates its schema and its rows, then splits the rows into pages.
// Each page keeps a column summary, so a scan can skip a page before decode.
// The scan reports its page decisions in ScanStats; nothing is silent.

use crate::model::{
    Column, DataSource, DataType, Expr, Result, Row, ScanStats, Value, MAX_ROWS, MEMORY_LIMIT,
};
use crate::page_scan::{scan, Page};
use crate::page_stats::{PagePlan, PageStats};
use sheaf_native::storage::{SpineEntry, StorageBackend, SPINE_BYTES, SPINE_CELLS};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct Table {
    pub columns: Vec<Column>,
    pub pages: Vec<Page>,
    pub count: usize,
    pub bytes: usize,
}

pub type Catalog = BTreeMap<String, Arc<Table>>;

pub fn validate_row(columns: &[Column], row: &Row) -> Result<()> {
    if columns.len() != row.len() {
        return Err("row width does not match the table".into());
    }
    for (column, value) in columns.iter().zip(row) {
        let valid = match (column.data_type, value) {
            (_, Value::Null) => column.nullable,
            (DataType::Int, Value::Int(_)) => true,
            (DataType::Float, Value::Float(v)) => v.is_finite(),
            (DataType::Text, Value::Text(_)) => true,
            (DataType::Bool, Value::Bool(_)) => true,
            _ => false,
        };
        if !valid {
            return Err(format!(
                "invalid value for column {} ({:?})",
                column.name, column.data_type
            ));
        }
    }
    Ok(())
}

impl Table {
    pub fn build(columns: Vec<Column>, rows: &[Row], backend: &dyn StorageBackend) -> Result<Self> {
        if columns.is_empty() || columns.len() > 256 {
            return Err("table requires 1 to 256 columns".into());
        }
        let mut names = std::collections::HashSet::new();
        if columns
            .iter()
            .any(|c| c.name.is_empty() || !names.insert(&c.name))
        {
            return Err("empty or duplicate column name".into());
        }
        if rows.len() > MAX_ROWS {
            return Err("table row limit exceeded".into());
        }
        let mut pages = Vec::new();
        let mut bytes = columns
            .iter()
            .map(|c| std::mem::size_of::<Column>() + c.name.len())
            .sum::<usize>();
        if bytes > MEMORY_LIMIT {
            return Err("table metadata limit exceeded".into());
        }
        for (block, chunk) in rows.chunks(SPINE_CELLS).enumerate() {
            let mut encoded = Vec::new();
            for row in chunk {
                validate_row(&columns, row)?;
                let item = serde_json::to_vec(row).map_err(|e| e.to_string())?;
                if item.len() > crate::model::MAX_REQUEST_BYTES * 4 {
                    return Err("row exceeds byte limit".into());
                }
                encoded.push(item);
            }
            let capacity = encoded.iter().map(|r| r.len() + 4 + 63).sum::<usize>();
            bytes += capacity + SPINE_BYTES;
            if bytes > MEMORY_LIMIT {
                return Err("table memory limit exceeded".into());
            }
            let mut payload = vec![0u8; capacity];
            let mut used = 0;
            let mut entries = Vec::new();
            for (slot, raw) in encoded.iter().enumerate() {
                let offset = backend
                    .arena_alloc(&mut payload, &mut used, raw.len() + 4, 64)
                    .ok_or("storage arena exhausted")?;
                payload[offset..offset + 4].copy_from_slice(&(raw.len() as u32).to_le_bytes());
                payload[offset + 4..offset + 4 + raw.len()].copy_from_slice(raw);
                let mut stalk_hash = [0; 16];
                stalk_hash.copy_from_slice(&blake3::hash(raw).as_bytes()[..16]);
                entries.push(SpineEntry {
                    cell_id: (block * SPINE_CELLS + slot) as u32,
                    stalk_hash,
                    byte_offset: offset as u64,
                    flags_mode: 0,
                });
            }
            let spine = backend
                .spine_pack(&entries)
                .map_err(|e| format!("storage: {e:?}"))?;
            let stats = PageStats::build(&columns, chunk)?;
            pages.push(Page { spine, payload, stats });
        }
        Ok(Self {
            columns,
            pages,
            count: rows.len(),
            bytes,
        })
    }

    pub fn rows(&self, backend: &dyn StorageBackend) -> Result<Vec<Row>> {
        let mut rows = Vec::with_capacity(self.count);
        let mut bytes = 0usize;
        for page in &self.pages {
            page.decode(backend, &self.columns, &mut rows, &mut bytes)?;
        }
        if rows.len() != self.count {
            return Err("spine row count mismatch".into());
        }
        Ok(rows)
    }

    /// Read the table with page-level pruning (NF-12). The row filter still runs
    /// after this call, so a kept page is only a cost, never a wrong answer.
    pub fn scan(
        &self,
        backend: &dyn StorageBackend,
        columns: &[String],
        predicates: &[Expr],
    ) -> Result<(Vec<Row>, ScanStats)> {
        let plan = PagePlan::compile(predicates, columns);
        scan(&self.pages, &plan, backend, &self.columns, self.count)
    }
}

pub struct Snapshot<'a> {
    pub catalog: &'a Catalog,
    pub backend: &'a dyn StorageBackend,
}

impl DataSource for Snapshot<'_> {
    fn columns(&self, table: &str) -> Result<Vec<Column>> {
        Ok(self
            .catalog
            .get(table)
            .ok_or_else(|| format!("unknown table {table}"))?
            .columns
            .clone())
    }
    fn rows(&self, table: &str) -> Result<Vec<Row>> {
        self.catalog
            .get(table)
            .ok_or_else(|| format!("unknown table {table}"))?
            .rows(self.backend)
    }
    fn scan(
        &self,
        table: &str,
        columns: &[String],
        predicates: &[Expr],
    ) -> Result<(Vec<Row>, ScanStats)> {
        self.catalog
            .get(table)
            .ok_or_else(|| format!("unknown table {table}"))?
            .scan(self.backend, columns, predicates)
    }
}
