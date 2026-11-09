// crates/sheaf-storage/src/bcsf/catalog.rs — table/catalog-level meta map
// (B11 / METHOD-048, REQ-053 / SPEC-053).
//
// A table is a NAME plus a SET of segments plus the column offsets of the
// discrete slab plus a schema version. Until now a table could only be
// reached by remembering its path; there was no catalog to open a table by
// name, and nothing that gated a plan against the schema it was written
// for.
//
// The catalog persists to a self-describing sidecar, so a table survives
// process exit: open the catalog, ask for the table by name, get its
// segments back.

use std::path::{Path, PathBuf};

/// One segment of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRef {
    /// Segment file path (as recorded in the catalog).
    pub path: PathBuf,
    /// Generation of the segment (the compaction epoch it was produced in).
    pub generation: u64,
    /// Cell count in the segment.
    pub ncells: u64,
    /// Fingerprint of the source data the segment was built from.
    pub src_fingerprint: u64,
}

/// Per-table metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableMeta {
    pub name: String,
    /// Schema version. Every plan must state the version it was written
    /// for; a mismatch is refused (never silently executed).
    pub schema_version: u64,
    /// Column name -> offset in the 16-byte discrete slab.
    pub column_offsets: Vec<(String, usize)>,
    /// The table's segments, in order.
    pub segments: Vec<SegmentRef>,
}

impl TableMeta {
    /// Offset of a column in the discrete slab.
    pub fn column_offset(&self, column: &str) -> Result<usize, String> {
        self.column_offsets
            .iter()
            .find(|(n, _)| n == column)
            .map(|(_, o)| *o)
            .ok_or_else(|| format!("table '{}': unknown column '{column}'", self.name))
    }
}

/// The catalog: a name -> table map plus its own generation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub tables: Vec<TableMeta>,
    /// Catalog generation (bumped on every structural change).
    pub generation: u64,
    /// Fingerprint of the source data the catalog describes.
    pub src_fingerprint: u64,
}

/// Catalog sidecar magic ("NIMXCATL").
pub const CATALOG_MAGIC: u64 = 0x4E49_4D58_4341_544C;
/// Catalog sidecar format version.
pub const CATALOG_FMT_VER: u32 = 1;
/// Catalog sidecar header length: magic|ver|generation|fingerprint|body_len|crc.
pub const CATALOG_HEADER_LEN: usize = 8 + 4 + 8 + 8 + 8 + 4;

impl Catalog {
    /// Build a catalog from tables.
    pub fn new(tables: Vec<TableMeta>, generation: u64, src_fingerprint: u64) -> Self {
        Catalog { tables, generation, src_fingerprint }
    }

    /// Open a table BY NAME.
    pub fn open_table(&self, name: &str) -> Result<&TableMeta, String> {
        self.tables
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("catalog: no table named '{name}'"))
    }

    /// Open a table by name AND gate it against the schema version the
    /// plan was written for. A mismatch is refused, never executed.
    pub fn plan_table(&self, name: &str, plan_schema_version: u64) -> Result<&TableMeta, String> {
        let t = self.open_table(name)?;
        if t.schema_version != plan_schema_version {
            return Err(format!(
                "catalog: plan for table '{}' targets schema v{plan_schema_version} but the table is v{}",
                name, t.schema_version
            ));
        }
        Ok(t)
    }

    /// Serialize the catalog into a sidecar image.
    pub fn to_sidecar(&self) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&(self.tables.len() as u32).to_le_bytes());
        for t in &self.tables {
            let nb = t.name.as_bytes();
            body.extend_from_slice(&(nb.len() as u32).to_le_bytes());
            body.extend_from_slice(nb);
            body.extend_from_slice(&t.schema_version.to_le_bytes());
            body.extend_from_slice(&(t.column_offsets.len() as u32).to_le_bytes());
            for (cn, off) in &t.column_offsets {
                let cb = cn.as_bytes();
                body.extend_from_slice(&(cb.len() as u32).to_le_bytes());
                body.extend_from_slice(cb);
                body.extend_from_slice(&(*off as u64).to_le_bytes());
            }
            body.extend_from_slice(&(t.segments.len() as u32).to_le_bytes());
            for s in &t.segments {
                let pb = s.path.to_string_lossy().into_owned();
                let pb = pb.as_bytes();
                body.extend_from_slice(&(pb.len() as u32).to_le_bytes());
                body.extend_from_slice(pb);
                body.extend_from_slice(&s.generation.to_le_bytes());
                body.extend_from_slice(&s.ncells.to_le_bytes());
                body.extend_from_slice(&s.src_fingerprint.to_le_bytes());
            }
        }
        let crc = crate::lsp::ring_wal::crc_hw(&body);
        let mut out = Vec::with_capacity(CATALOG_HEADER_LEN + body.len());
        out.extend_from_slice(&CATALOG_MAGIC.to_le_bytes());
        out.extend_from_slice(&CATALOG_FMT_VER.to_le_bytes());
        out.extend_from_slice(&self.generation.to_le_bytes());
        out.extend_from_slice(&self.src_fingerprint.to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Load a catalog from a sidecar image. Fails loud on any mismatch.
    pub fn from_sidecar(bytes: &[u8]) -> Result<Catalog, String> {
        if bytes.len() < CATALOG_HEADER_LEN {
            return Err("catalog: truncated header".to_string());
        }
        if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != CATALOG_MAGIC {
            return Err("catalog: bad magic".to_string());
        }
        let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if ver != CATALOG_FMT_VER {
            return Err(format!("catalog: format version {ver} != {CATALOG_FMT_VER}"));
        }
        let generation = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let fingerprint = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let body_len = u64::from_le_bytes(bytes[28..36].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        let body = bytes
            .get(CATALOG_HEADER_LEN..CATALOG_HEADER_LEN + body_len)
            .ok_or("catalog: body truncated")?;
        if crate::lsp::ring_wal::crc_hw(body) != crc {
            return Err("catalog: body crc mismatch (corrupt file)".to_string());
        }
        let mut p = 0usize;
        let u32_at = |p: &mut usize| -> Result<u32, String> {
            let s = *p;
            let e = s + 4;
            if e > body.len() {
                return Err("catalog: truncated u32".to_string());
            }
            *p = e;
            Ok(u32::from_le_bytes(body[s..e].try_into().unwrap()))
        };
        let u64_at = |p: &mut usize| -> Result<u64, String> {
            let s = *p;
            let e = s + 8;
            if e > body.len() {
                return Err("catalog: truncated u64".to_string());
            }
            *p = e;
            Ok(u64::from_le_bytes(body[s..e].try_into().unwrap()))
        };
        let bytes_at = |p: &mut usize| -> Result<&[u8], String> {
            let n = u32_at(p)? as usize;
            let s = *p;
            let e = s + n;
            if e > body.len() {
                return Err("catalog: truncated bytes".to_string());
            }
            *p = e;
            Ok(&body[s..e])
        };
        let n_tables = u32_at(&mut p)? as usize;
        let mut tables: Vec<TableMeta> = Vec::with_capacity(n_tables);
        for _ in 0..n_tables {
            let name = String::from_utf8(bytes_at(&mut p)?.to_vec())
                .map_err(|_| "catalog: table name is not utf8".to_string())?;
            let schema_version = u64_at(&mut p)?;
            let n_cols = u32_at(&mut p)? as usize;
            let mut column_offsets: Vec<(String, usize)> = Vec::with_capacity(n_cols);
            for _ in 0..n_cols {
                let cn = String::from_utf8(bytes_at(&mut p)?.to_vec())
                    .map_err(|_| "catalog: column name is not utf8".to_string())?;
                let off = u64_at(&mut p)? as usize;
                column_offsets.push((cn, off));
            }
            let n_segs = u32_at(&mut p)? as usize;
            let mut segments: Vec<SegmentRef> = Vec::with_capacity(n_segs);
            for _ in 0..n_segs {
                let pb = bytes_at(&mut p)?;
                let path = PathBuf::from(String::from_utf8_lossy(pb).to_string());
                let generation = u64_at(&mut p)?;
                let ncells = u64_at(&mut p)?;
                let src_fingerprint = u64_at(&mut p)?;
                segments.push(SegmentRef { path, generation, ncells, src_fingerprint });
            }
            tables.push(TableMeta { name, schema_version, column_offsets, segments });
        }
        if p != body.len() {
            return Err("catalog: trailing bytes in body".to_string());
        }
        Ok(Catalog { tables, generation, src_fingerprint: fingerprint })
    }

    /// Write the catalog to disk.
    pub fn write_sidecar(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_sidecar()).map_err(|e| format!("{path:?}: {e}"))
    }

    /// Read a catalog from disk.
    pub fn read_sidecar(path: &Path) -> Result<Catalog, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
        Catalog::from_sidecar(&bytes)
    }
}
