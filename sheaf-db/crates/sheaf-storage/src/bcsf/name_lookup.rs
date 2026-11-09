// crates/sheaf-storage/src/bcsf/name_lookup.rs — find cells by discrete field value
// FILE-020 (METHOD-011). Depends on FILE-015 (reader).
// Given a discrete field name and target value, return the contiguous slice
// of matching cell indices. Parity with full scan is verified by E2E.
// The discrete payload is a fixed 16-byte slab per cell; field extraction
// uses a simple columnar layout defined by the schema.
use super::reader::BcsfReader;

/// A name lookup index over the discrete payload slab.
/// Built once per snapshot; serves point queries without full scan.
pub struct NameIndex {
    /// Field name -> column offset in the 16-byte discrete slab.
    fields: Vec<(String, usize)>,
    /// For each field, a sorted map from value bytes to contiguous cell range.
    /// Value bytes are the raw little-endian encoding of the field.
    index: Vec<Vec<(Vec<u8>, std::ops::Range<usize>)>>,
    /// Sorted cell indices per field for binary search.
    sorted_cells: Vec<Vec<u32>>,
}

impl NameIndex {
    /// Build a name index from a reader + schema field definitions.
    /// `fields` is [(field_name, offset_in_discrete_slab)] in schema order.
    pub fn build(
        reader: &BcsfReader,
        fields: &[(String, usize)],
        cell_classes: &[(u64, String)], // (class_id, type_name) pairs
    ) -> Self {
        let n = reader.len();
        let mut field_names: Vec<String> = Vec::new();
        let mut offsets: Vec<usize> = Vec::new();
        for (name, off) in fields {
            field_names.push(name.clone());
            offsets.push(*off);
        }
        let mut sorted_cells: Vec<Vec<u32>> = vec![Vec::new(); fields.len()];
        let mut index: Vec<Vec<(Vec<u8>, std::ops::Range<usize>)>> =
            vec![Vec::new(); fields.len()];
        // Group cells by class, then by field value.
        for fid in 0..offsets.len() {
            let mut cells: Vec<u32> = Vec::with_capacity(n);
            for cell in 0..n {
                let class_bytes = &reader.discrete(cell)[..8];
                let class_id = u64::from_le_bytes(class_bytes.try_into().unwrap());
                // Only index cells whose class matches a known type.
                if cell_classes.iter().any(|(cid, _)| *cid == class_id) {
                    cells.push(cell as u32);
                }
            }
            // Sort by field value for binary search.
            cells.sort_by(|&a, &b| {
                let va = &reader.discrete(a as usize)[offsets[fid]..offsets[fid] + 8];
                let vb = &reader.discrete(b as usize)[offsets[fid]..offsets[fid] + 8];
                va.cmp(vb)
            });
            // Build value -> contiguous range map before moving cells.
            let mut i = 0;
            while i < cells.len() {
                let val = reader.discrete(cells[i] as usize)[offsets[fid]..offsets[fid] + 8].to_vec();
                let start = i;
                while i < cells.len()
                    && reader.discrete(cells[i] as usize)[offsets[fid]..offsets[fid] + 8] == val[..]
                {
                    i += 1;
                }
                index[fid].push((val, start..i));
            }
            sorted_cells[fid] = cells;
        }
        Self {
            fields: field_names.into_iter().zip(offsets).collect(),
            index,
            sorted_cells,
        }
    }

    /// Lookup cells by field name + value. Returns contiguous slice of cell indices.
    pub fn lookup(&self, field: &str, value: &[u8]) -> Option<&std::ops::Range<usize>> {
        let fid = self.fields.iter().position(|(n, _)| n == field)?;
        let idx = &self.index[fid];
        // Binary search on value bytes.
        let pos = idx.binary_search_by(|(v, _)| v.as_slice().cmp(value)).ok()?;
        Some(&idx[pos].1)
    }

    /// Get all cell indices for a field (sorted by value).
    pub fn field_cells(&self, field: &str) -> Option<&[u32]> {
        let fid = self.fields.iter().position(|(n, _)| n == field)?;
        Some(&self.sorted_cells[fid])
    }

    /// Interval slice: position-ranges (into `field_cells`) whose field value
    /// lies in [lo, hi). Comparison is lexicographic on raw value bytes, so
    /// ordered fields must use an order-preserving (big-endian) encoding —
    /// the same memcomparable convention as real ordered indexes. (The
    /// pre-existing sort and equality lookup are byte-wise; LE-encoded
    /// numerics do NOT sort numerically, hence BE here.)
    pub fn range(&self, field: &str, lo: u64, hi: u64) -> Vec<std::ops::Range<usize>> {
        let fid = match self.fields.iter().position(|(n, _)| n == field) {
            Some(f) => f,
            None => return Vec::new(),
        };
        if lo >= hi {
            return Vec::new();
        }
        let (lo_b, hi_b) = (lo.to_be_bytes(), hi.to_be_bytes());
        let idx = &self.index[fid];
        let s = idx.partition_point(|(v, _)| v.as_slice() < &lo_b[..]);
        let e = idx.partition_point(|(v, _)| v.as_slice() < &hi_b[..]);
        idx[s..e].iter().map(|(_, r)| r.clone()).collect()
    }
}

/// Extract a u64 field value from a cell's discrete slab at the given offset.
pub fn extract_u64(reader: &BcsfReader, cell: usize, offset: usize) -> u64 {
    let bytes = &reader.discrete(cell)[offset..offset + 8];
    u64::from_le_bytes(bytes.try_into().unwrap())
}

/// Extract a bool field value from a cell's discrete slab at the given offset.
pub fn extract_bool(reader: &BcsfReader, cell: usize, offset: usize) -> bool {
    reader.discrete(cell)[offset] != 0
}

// ---------------------------------------------------------------------------
// B9: persisted sidecar. The value->cells index used to live only in RAM and
// was rebuilt on every process start. It now serializes to a self-describing
// sidecar file so it SURVIVES process exit and can be reloaded verbatim
// (no re-derivation, no rebuild pass).
//
// Layout: [MAGIC u64][fmt_ver u32][generation u64][src_fingerprint u64]
//         [body_len u64][crc32c u32][body]
// body:   [n_fields u32] then per field:
//           [name_len u32][name][offset u64][n_values u64]
//           per value: [len u32][value bytes][start u64][end u64]
//           [n_sorted u64][sorted cells u32...]
// ---------------------------------------------------------------------------

/// Sidecar magic ("NIMXSIDC").
pub const SIDECAR_MAGIC: u64 = 0x4E49_4D58_5349_4443;
/// Sidecar format version.
pub const SIDECAR_FMT_VER: u32 = 1;
/// Sidecar header length: magic|ver|generation|fingerprint|body_len|crc.
pub const SIDECAR_HEADER_LEN: usize = 8 + 4 + 8 + 8 + 8 + 4;

impl NameIndex {
    /// Serialize the index into a sidecar image.
    pub fn to_sidecar(&self, generation: u64, src_fingerprint: u64) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&(self.fields.len() as u32).to_le_bytes());
        for (fid, (name, off)) in self.fields.iter().enumerate() {
            body.extend_from_slice(&(name.len() as u32).to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.extend_from_slice(&off.to_le_bytes());
            let vals = &self.index[fid];
            body.extend_from_slice(&(vals.len() as u64).to_le_bytes());
            for (v, r) in vals {
                body.extend_from_slice(&(v.len() as u32).to_le_bytes());
                body.extend_from_slice(v);
                body.extend_from_slice(&(r.start as u64).to_le_bytes());
                body.extend_from_slice(&(r.end as u64).to_le_bytes());
            }
            let cells = &self.sorted_cells[fid];
            body.extend_from_slice(&(cells.len() as u64).to_le_bytes());
            for c in cells {
                body.extend_from_slice(&c.to_le_bytes());
            }
        }
        let crc = crate::lsp::ring_wal::crc_hw(&body);
        let mut out = Vec::with_capacity(SIDECAR_HEADER_LEN + body.len());
        out.extend_from_slice(&SIDECAR_MAGIC.to_le_bytes());
        out.extend_from_slice(&SIDECAR_FMT_VER.to_le_bytes());
        out.extend_from_slice(&generation.to_le_bytes());
        out.extend_from_slice(&src_fingerprint.to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Load an index from a sidecar image (the ONLY state carried across a
    /// process restart). Fails loud on any mismatch - never a partial load.
    pub fn from_sidecar(bytes: &[u8]) -> Result<(NameIndex, u64, u64), String> {
        if bytes.len() < SIDECAR_HEADER_LEN {
            return Err("sidecar: truncated header".to_string());
        }
        if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != SIDECAR_MAGIC {
            return Err("sidecar: bad magic".to_string());
        }
        let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if ver != SIDECAR_FMT_VER {
            return Err(format!("sidecar: format version {ver} != {SIDECAR_FMT_VER}"));
        }
        let generation = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let fingerprint = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let body_len = u64::from_le_bytes(bytes[28..36].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        let body = bytes
            .get(SIDECAR_HEADER_LEN..SIDECAR_HEADER_LEN + body_len)
            .ok_or("sidecar: body truncated")?;
        if crate::lsp::ring_wal::crc_hw(body) != crc {
            return Err("sidecar: body crc mismatch (corrupt file)".to_string());
        }
        let mut p = 0usize;
        let u32_at = |p: &mut usize| -> Result<u32, String> {
            let s = *p;
            let e = s + 4;
            if e > body.len() {
                return Err("sidecar: truncated u32".to_string());
            }
            *p = e;
            Ok(u32::from_le_bytes(body[s..e].try_into().unwrap()))
        };
        let u64_at = |p: &mut usize| -> Result<u64, String> {
            let s = *p;
            let e = s + 8;
            if e > body.len() {
                return Err("sidecar: truncated u64".to_string());
            }
            *p = e;
            Ok(u64::from_le_bytes(body[s..e].try_into().unwrap()))
        };
        let n_fields = u32_at(&mut p)? as usize;
        let mut fields: Vec<(String, usize)> = Vec::with_capacity(n_fields);
        let mut index: Vec<Vec<(Vec<u8>, std::ops::Range<usize>)>> = Vec::with_capacity(n_fields);
        let mut sorted_cells: Vec<Vec<u32>> = Vec::with_capacity(n_fields);
        for _ in 0..n_fields {
            let nlen = u32_at(&mut p)? as usize;
            let ne = p + nlen;
            if ne > body.len() {
                return Err("sidecar: truncated field name".to_string());
            }
            let name = String::from_utf8(body[p..ne].to_vec())
                .map_err(|_| "sidecar: field name is not utf8".to_string())?;
            p = ne;
            let off = u64_at(&mut p)? as usize;
            let n_values = u64_at(&mut p)? as usize;
            let mut vals: Vec<(Vec<u8>, std::ops::Range<usize>)> = Vec::with_capacity(n_values);
            for _ in 0..n_values {
                let vlen = u32_at(&mut p)? as usize;
                let ve = p + vlen;
                if ve > body.len() {
                    return Err("sidecar: truncated value".to_string());
                }
                let v = body[p..ve].to_vec();
                p = ve;
                let start = u64_at(&mut p)? as usize;
                let end = u64_at(&mut p)? as usize;
                vals.push((v, start..end));
            }
            let n_sorted = u64_at(&mut p)? as usize;
            let se = p + n_sorted * 4;
            if se > body.len() {
                return Err("sidecar: truncated sorted cells".to_string());
            }
            let cells: Vec<u32> = body[p..se]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect();
            p = se;
            fields.push((name, off));
            index.push(vals);
            sorted_cells.push(cells);
        }
        if p != body.len() {
            return Err("sidecar: trailing bytes in body".to_string());
        }
        Ok((NameIndex { fields, index, sorted_cells }, generation, fingerprint))
    }

    /// Write the sidecar to disk.
    pub fn write_sidecar(
        &self,
        path: &std::path::Path,
        generation: u64,
        src_fingerprint: u64,
    ) -> Result<(), String> {
        let bytes = self.to_sidecar(generation, src_fingerprint);
        std::fs::write(path, &bytes).map_err(|e| format!("{path:?}: {e}"))
    }

    /// Read a sidecar from disk (the process-restart path).
    pub fn read_sidecar(path: &std::path::Path) -> Result<(NameIndex, u64, u64), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
        NameIndex::from_sidecar(&bytes)
    }
}
