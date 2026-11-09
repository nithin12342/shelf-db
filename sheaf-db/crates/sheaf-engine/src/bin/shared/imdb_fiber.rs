// crates/sheaf-engine/src/bin/shared/imdb_fiber.rs Ã¢â‚¬â€ IMDb fiber bundle
// (B2 / METHOD-039, REQ-044 / SPEC-044).
//
// The IMDb fiber build used to cost 178.96 s per run (principals 102M rows +
// names 15.7M). This writes the SAME structures once into a sectioned
// container and loads only the sections a query needs.
//
// Container layout (little-endian):
//   [MAGIC u64][fmt_ver u32][n_sections u32][index_len u32][index_crc u32]
//   [index: n_sections x (name_len u16, name, offset u64, len u64, crc32c u32, gen u64)]
// B12: every section carries its own GENERATION (the producer's stamp for
// the input that produced it). A changed input bumps ONE section's
// generation, and only that section is rewritten - the rest of the bundle
// is copied byte-for-byte.
//   [payload: section bytes, sequentially written]
// A section is fetched by offset (Kafka-style fetch(offset)) and verified
// with its own CRC32C before use Ã¢â‚¬â€ a corrupt section fails loud, never
// half-loads.
// Each bench binary includes this module separately, so a helper used by
// one bench is dead code in another.
#![allow(dead_code)]
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use sheaf_storage::lsp::ring_wal::{crc_hw, crc_state_finish, crc_state_init, crc_state_update};
pub const MAGIC: u64 = 0x494D_4442_4649_424C; // "IMDBFIBL"
pub const FMT_VER: u32 = 3;
pub const HEADER_LEN: usize = 24;
/// The chunk a section load reads before it folds the CRC. 4 MiB keeps
/// the chunk in the last level cache, so the verify runs hot.
pub const CRC_CHUNK: usize = 4 << 20;
/// The smallest section that pays for the pipelined verify.
///
/// Measured 2026-10-22 on this host, same run, same box. On the 167 MB
/// basics section the pipeline is 1.26x to 1.46x faster than the serial
/// read plus verify. On the 34 MB ratings section it is 0.80x, because
/// the hand-off between the two threads costs about 7 to 9 ms of fixed
/// time, which is more than the 5 ms of CRC it hides. Below this size the
/// serial form is measured better, so the serial form is used.
pub const PIPELINE_MIN_BYTES: usize = 64 << 20;
/// A pointer that may cross a thread boundary.
///
/// The reader thread writes disjoint chunk ranges of one buffer and this
/// thread folds each range after the channel hands it over, so the ranges
/// are never touched by both sides at the same time.
struct SendPtr(*mut u8);
// SAFETY: the only accesses are the disjoint chunk writes in the reader
// thread and the reads of the ranges after their transfer, which the
// channel orders. No two threads touch one byte at the same time.
unsafe impl Send for SendPtr {}
impl SendPtr {
    /// The raw pointer. A method call captures the whole wrapper, so the
    /// closure takes the Send type and not the bare pointer.
    fn get(&self) -> *mut u8 {
        self.0
    }
}
/// One section's bytes + where it lives in the file.
#[derive(Clone)]
pub struct SectionMeta {
    pub name: String,
    pub offset: u64,
    pub len: u64,
    pub crc: u32,
    /// B12: per-section generation (producer's stamp for the input that
    /// produced this section). Unchanged sections keep their generation.
    pub gen: u64,
}
/// Writer: appends section payloads to a temp file, then writes
/// header + index + payload to the final file.
pub struct BundleWriter {
    payload: std::fs::File,
    path: String,
    offset: u64,
    metas: Vec<SectionMeta>,
}
impl BundleWriter {
    pub fn create(path: &str) -> std::io::Result<Self> {
        let tmp_path = format!("{path}.payload");
        let file = std::fs::File::create(&tmp_path)?;
        Ok(Self { payload: file, path: path.to_string(), offset: 0, metas: Vec::new() })
    }
    /// Append one section (generation 0).
    pub fn write_section(&mut self, name: &str, data: &[u8]) -> std::io::Result<()> {
        self.write_section_gen(name, data, 0)
    }
    /// Append one section with its GENERATION (B12).
    pub fn write_section_gen(&mut self, name: &str, data: &[u8], gen: u64) -> std::io::Result<()> {
        let crc = crc_hw(data);
        self.metas.push(SectionMeta {
            name: name.to_string(),
            offset: self.offset,
            len: data.len() as u64,
            crc,
            gen,
        });
        self.payload.write_all(data)?;
        self.offset += data.len() as u64;
        Ok(())
    }
    /// Write header + index + payload and close the file.
    pub fn finish(self) -> std::io::Result<()> {
        // The index length is known before writing (fixed per section), so
        // the shifted offsets can be computed in one pass.
        let index_len: usize = self
            .metas
            .iter()
            .map(|m| 2 + m.name.len() + 8 + 8 + 4 + 8)
            .sum();
        let head_len = HEADER_LEN as u64 + index_len as u64;
        let mut index: Vec<u8> = Vec::with_capacity(index_len);
        for m in &self.metas {
            index.extend_from_slice(&(m.name.len() as u16).to_le_bytes());
            index.extend_from_slice(m.name.as_bytes());
            index.extend_from_slice(&(m.offset + head_len).to_le_bytes());
            index.extend_from_slice(&m.len.to_le_bytes());
            index.extend_from_slice(&m.crc.to_le_bytes());
            index.extend_from_slice(&m.gen.to_le_bytes());
        }
        debug_assert_eq!(index.len(), index_len);
        let index_crc = crc_hw(&index);
        let tmp_path = format!("{}.payload", self.path);
        let mut src = std::fs::File::open(&tmp_path)?;
        let mut dst = std::fs::File::create(&self.path)?;
        dst.write_all(&MAGIC.to_le_bytes())?;
        dst.write_all(&FMT_VER.to_le_bytes())?;
        dst.write_all(&(self.metas.len() as u32).to_le_bytes())?;
        dst.write_all(&(index_len as u32).to_le_bytes())?;
        dst.write_all(&index_crc.to_le_bytes())?;
        dst.write_all(&index)?;
        let mut buf = vec![0u8; 8 << 20];
        let mut remaining = self.offset;
        use std::io::Read;
        while remaining > 0 {
            let want = std::cmp::min(remaining as usize, buf.len());
            let n = src.read(&mut buf[..want])?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])?;
            remaining -= n as u64;
        }
        drop(src);
        drop(dst);
        let _ = std::fs::remove_file(&tmp_path);
        Ok(())
    }
}
/// Reader: opens the header + index, fetches sections on demand.
pub struct BundleReader {
    file: std::io::BufReader<std::fs::File>,
    metas: HashMap<String, SectionMeta>,
    /// The bundle path, so a second handle can read a section on another
    /// thread while this thread verifies the chunk before it.
    path: String,
}
impl BundleReader {
    pub fn open(path: &str) -> Result<BundleReader, String> {
        let mut file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
        use std::io::Read;
        let mut head = [0u8; HEADER_LEN];
        file.read_exact(&mut head).map_err(|e| format!("{path}: {e}"))?;
        if u64::from_le_bytes(head[..8].try_into().unwrap()) != MAGIC {
            return Err("bad magic (not an imdb bundle)".to_string());
        }
        let ver = u32::from_le_bytes(head[8..12].try_into().unwrap());
        if ver != FMT_VER {
            return Err(format!("format version {ver} != {FMT_VER} (rebuild)"));
        }
        let n = u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize;
        let index_len = u32::from_le_bytes(head[16..20].try_into().unwrap()) as usize;
        let index_crc = u32::from_le_bytes(head[20..24].try_into().unwrap());
        let mut index = vec![0u8; index_len];
        file.read_exact(&mut index)
            .map_err(|e| format!("{path}: read index {e}"))?;
        let read = index_len;
        // trim to the real index length by re-parsing
        let mut metas = HashMap::new();
        let mut off = 0usize;
        for _ in 0..n {
            if off + 2 > read {
                break;
            }
            let nl = u16::from_le_bytes(index[off..off + 2].try_into().unwrap()) as usize;
            off += 2;
            if off + nl + 28 > read {
                break;
            }
            let name: String = String::from_utf8_lossy(&index[off..off + nl]).to_string();
            off += nl;
            let o = u64::from_le_bytes(index[off..off + 8].try_into().unwrap());
            off += 8;
            let l = u64::from_le_bytes(index[off..off + 8].try_into().unwrap());
            off += 8;
            let c = u32::from_le_bytes(index[off..off + 4].try_into().unwrap());
            off += 4;
            let g = u64::from_le_bytes(index[off..off + 8].try_into().unwrap());
            off += 8;
            metas.insert(name.clone(), SectionMeta { name, offset: o, len: l, crc: c, gen: g });
        }
        // verify the index itself against the stored crc (over the exact bytes)
        let index_bytes = &index[..off];
        if crc_hw(index_bytes) != index_crc {
            return Err("index crc mismatch (torn bundle?)".to_string());
        }
        // The index already stores FINAL file offsets (the writer shifted
        // them). Shifting again here seeked past the section and produced a
        // false "crc mismatch (torn write?)" Ã¢â‚¬â€ a real bug, caught by the gate.
        Ok(BundleReader { file: std::io::BufReader::new(file), metas, path: path.to_string() })
    }
    /// Generation of a section (B12), if present.
    pub fn section_gen(&self, name: &str) -> Option<u64> {
        self.metas.get(name).map(|m| m.gen)
    }
    /// Metadata of a section (offset/len/crc/gen), if present.
    pub fn section_meta(&self, name: &str) -> Option<&SectionMeta> {
        self.metas.get(name)
    }
    /// All section names (sorted, deterministic).
    pub fn section_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.metas.keys().cloned().collect();
        v.sort();
        v
    }
    /// Fetch one section, verify its CRC32C, return its bytes.
    ///
    /// The verify runs on a second thread, overlapped with the read. The
    /// measured split on this host: the raw read is 79 to 82 ms at 2.47 to
    /// 2.54 GB/s and the CRC is 30 to 33 ms at 6.02 to 6.81 GB/s. A single
    /// thread pays the sum, because the CRC is bound by the crc32
    /// instruction and not by memory, so warming the data does not make it
    /// faster (measured: 6.15 to 6.91 GB/s over a chunk already in cache).
    /// Two threads pay the larger of the two, so the load tends to the read.
    pub fn section(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let m = self
            .metas
            .get(name)
            .ok_or_else(|| format!("section {name} missing"))?
            .clone();
        let len = m.len as usize;
        let mut buf = vec![0u8; len];
        if len == 0 {
            if m.crc != crc_hw(&[]) {
                return Err(format!("{name}: crc mismatch (torn write?)"));
            }
            return Ok(buf);
        }
        let crc = if len >= PIPELINE_MIN_BYTES {
            self.read_and_verify(name, m.offset, &mut buf)?
        } else {
            self.read_and_verify_serial(name, m.offset, &mut buf)?
        };
        if crc != m.crc {
            return Err(format!("{name}: crc mismatch (torn write?)"));
        }
        Ok(buf)
    }
    /// The serial form: read the whole section, then one CRC pass. Used
    /// below `PIPELINE_MIN_BYTES`, where the hand-off costs more than the
    /// verify it hides.
    fn read_and_verify_serial(
        &mut self,
        name: &str,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<u32, String> {
        use std::io::Read;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("{name}: seek {e}"))?;
        self.file
            .read_exact(buf)
            .map_err(|e| format!("{name}: read {e}"))?;
        Ok(crc_hw(buf))
    }
    /// Read `buf` from the section offset on a reader thread and fold the
    /// CRC here, chunk by chunk, as each chunk lands.
    fn read_and_verify(&mut self, name: &str, offset: u64, buf: &mut [u8]) -> Result<u32, String> {
        use std::io::Read;
        let len = buf.len();
        // Two wrappers for one address: one moves into the reader thread,
        // one stays here for the folding side.
        let ptr_w = SendPtr(buf.as_mut_ptr());
        let ptr_r = SendPtr(buf.as_mut_ptr());
        let path = self.path.clone();
        std::thread::scope(|s| -> Result<u32, String> {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, usize)>(4);
            let reader = s.spawn(move || -> Result<(), String> {
                let mut f = std::fs::File::open(&path).map_err(|e| format!("{name}: open {e}"))?;
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| format!("{name}: seek {e}"))?;
                let mut off = 0usize;
                while off < len {
                    let end = (off + CRC_CHUNK).min(len);
                    // SAFETY: this is the only writer of [off, end), and it
                    // sends the range only after read_exact returns. The
                    // folding side reads a range only after it receives it,
                    // so the two sides never touch the same bytes at once.
                    let chunk = unsafe { std::slice::from_raw_parts_mut(ptr_w.get().add(off), end - off) };
                    f.read_exact(chunk).map_err(|e| format!("{name}: read {e}"))?;
                    if tx.send((off, end)).is_err() {
                        return Ok(());
                    }
                    off = end;
                }
                Ok(())
            });
            let mut state = crc_state_init();
            while let Ok((a, b)) = rx.recv() {
                // SAFETY: the range was written and sent by the reader, so
                // the write happens before this read.
                let chunk = unsafe { std::slice::from_raw_parts(ptr_r.get().add(a), b - a) };
                state = crc_state_update(state, chunk);
            }
            reader
                .join()
                .map_err(|_| format!("{name}: reader thread panicked"))??;
            Ok(crc_state_finish(state))
        })
    }
}
/// Fixed-width record helpers (all little-endian).
pub fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap())
}
/// Packed-record accessors. The generic u64_at/i32_at helpers assume an
/// 8-byte stride; the IMDb sections are PACKED (13/20/12 bytes), so each
/// record type gets its own accessor. Getting this wrong silently reads
/// the wrong fields (a real bug class in this bench).
pub fn basics_id(b: &[u8], i: usize) -> u64 {
    let o = i * 13;
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
pub fn basics_year(b: &[u8], i: usize) -> i32 {
    let o = i * 13 + 8;
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn basics_flags(b: &[u8], i: usize) -> u8 {
    b[i * 13 + 12]
}
pub fn ratings_id(b: &[u8], i: usize) -> u64 {
    let o = i * 20;
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
pub fn ratings_avg(b: &[u8], i: usize) -> f32 {
    let o = i * 20 + 8;
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn ratings_votes(b: &[u8], i: usize) -> i64 {
    let o = i * 20 + 12;
    i64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
pub fn names_id(b: &[u8], i: usize) -> u64 {
    let o = i * 12;
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
pub fn names_year(b: &[u8], i: usize) -> i32 {
    let o = i * 12 + 8;
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn pc_id(b: &[u8], i: usize) -> u64 {
    let o = i * 12;
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
pub fn pc_n(b: &[u8], i: usize) -> u32 {
    let o = i * 12 + 8;
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
// ---------------------------------------------------------------------------
// B12: incremental section rewrite. Rebuilding the whole bundle costs
// 40.6 MB even when ONE input changed. `rewrite_sections` writes a NEW
// bundle in which only the updated sections are re-encoded; every other
// section is copied byte-for-byte from the old bundle.
// ---------------------------------------------------------------------------
/// One section to replace in an incremental rewrite.
pub struct SectionUpdate {
    pub name: String,
    pub data: Vec<u8>,
    pub gen: u64,
}
/// What an incremental rewrite actually did.
#[derive(Clone, Copy, Debug, Default)]
pub struct RewriteStats {
    /// Sections in the resulting bundle.
    pub sections_total: usize,
    /// Sections that were re-encoded (the rest were copied).
    pub sections_rewritten: usize,
    /// Bytes re-encoded.
    pub bytes_rewritten: u64,
    /// Bytes copied verbatim from the old bundle.
    pub bytes_copied: u64,
    /// Total size of the resulting bundle.
    pub total_bytes: u64,
}

/// Write a new bundle where only `updates` are re-encoded; every other
/// section is copied byte-for-byte from `old_path`.
pub fn rewrite_sections(
    old_path: &str,
    new_path: &str,
    updates: &[SectionUpdate],
) -> Result<RewriteStats, String> {
    let reader = BundleReader::open(old_path)?;
    let names = reader.section_names();
    let mut metas: Vec<SectionMeta> = Vec::with_capacity(names.len());
    for n in &names {
        let m = reader
            .section_meta(n)
            .ok_or_else(|| format!("section {n} vanished"))?
            .clone();
        metas.push(m);
    }
    // New payload order: the old order, with updated sections in place.
    let mut payloads: Vec<Option<&SectionUpdate>> = Vec::with_capacity(metas.len());
    for m in &metas {
        payloads.push(updates.iter().find(|u| u.name == m.name));
    }
    for u in updates {
        if !metas.iter().any(|m| m.name == u.name) {
            return Err(format!("update for unknown section '{}'", u.name));
        }
    }
    let index_len: usize = metas
        .iter()
        .map(|m| 2 + m.name.len() + 8 + 8 + 4 + 8)
        .sum();
    let head_len = HEADER_LEN as u64 + index_len as u64;
    // Compute the new layout: every section's final offset.
    let mut new_offsets: Vec<u64> = Vec::with_capacity(metas.len());
    let mut cursor = head_len;
    for (i, m) in metas.iter().enumerate() {
        new_offsets.push(cursor);
        cursor += match &payloads[i] {
            Some(u) => u.data.len() as u64,
            None => m.len,
        };
    }
    // The index: same names, new offsets, new crc/gen for updated sections.
    let mut index: Vec<u8> = Vec::with_capacity(index_len);
    let mut stats = RewriteStats {
        sections_total: metas.len(),
        ..Default::default()
    };
    for (i, m) in metas.iter().enumerate() {
        let (crc, gen, len) = match &payloads[i] {
            Some(u) => {
                stats.sections_rewritten += 1;
                stats.bytes_rewritten += u.data.len() as u64;
                (crc_hw(&u.data), u.gen, u.data.len() as u64)
            }
            None => {
                stats.bytes_copied += m.len;
                (m.crc, m.gen, m.len)
            }
        };
        index.extend_from_slice(&(m.name.len() as u16).to_le_bytes());
        index.extend_from_slice(m.name.as_bytes());
        index.extend_from_slice(&new_offsets[i].to_le_bytes());
        index.extend_from_slice(&len.to_le_bytes());
        index.extend_from_slice(&crc.to_le_bytes());
        index.extend_from_slice(&gen.to_le_bytes());
    }
    debug_assert_eq!(index.len(), index_len);
    let index_crc = crc_hw(&index);

    let mut src = std::fs::File::open(old_path).map_err(|e| format!("{old_path}: {e}"))?;
    let mut dst = std::fs::File::create(new_path).map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&MAGIC.to_le_bytes())
        .map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&FMT_VER.to_le_bytes())
        .map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&(metas.len() as u32).to_le_bytes())
        .map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&(index_len as u32).to_le_bytes())
        .map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&index_crc.to_le_bytes())
        .map_err(|e| format!("{new_path}: {e}"))?;
    dst.write_all(&index)
        .map_err(|e| format!("{new_path}: {e}"))?;
    use std::io::Read;
    for (i, m) in metas.iter().enumerate() {
        let bytes: Vec<u8> = match &payloads[i] {
            Some(u) => u.data.clone(),
            None => {
                use std::io::Seek;
                src.seek(std::io::SeekFrom::Start(m.offset))
                    .map_err(|e| format!("{}: seek {e}", m.name))?;
                let mut v = vec![0u8; m.len as usize];
                src.read_exact(&mut v)
                    .map_err(|e| format!("{}: read {e}", m.name))?;
                v
            }
        };
        dst.write_all(&bytes).map_err(|e| format!("{new_path}: {e}"))?;
    }
    dst.flush().map_err(|e| format!("{new_path}: {e}"))?;
    stats.total_bytes = cursor;
    Ok(stats)
}
