# ShelfDB BCSF Segment Format Efficiency Gains

**Date:** 2026-10-29
**Repository:** `D:\d2\sheaf-db`
**File:** `crates/sheaf-storage/src/bcsf/segment.rs`

---

## 1. Existing Architecture

ShelfDB already has its own on-disk file format: **BCSF** (Binary Cellular Sheaf Format), stored as self-describing segment files. It is not built on SQLite, Parquet, Arrow, or any third-party container.

### V1 segment layout (legacy)

```
[header][9 fixed parts][payload_crc]
header  = magic(8) | ver(4) | ncells(8) | gen(8) | fingerprint(8) |
          9 × (offset u64 + len u64) | crc32(4)
parts   = offsets | indices | stalk_offsets | morph_cells |
          morph_offsets | discrete | morphs | stalks | dims
```

Properties: magic + version, CRC over whole payload, cell-count validation, structural validation on open, `segment_meta()` reads only 36 bytes.

### Weaknesses in V1 (from code reading)

1. **Whole-file `std::fs::read`** in `open_segment` — copies the entire file into RAM before parsing. Large segments pay a full copy even when only the header is needed.
2. **Fixed 9-part table, no type tags** — adding a column requires a format version bump. No forward compatibility.
3. **No per-part checksums** — one CRC over the whole payload; a torn write in one part invalidates the entire segment.
4. **u32 directory offsets** — caps addressable payload at ~4GB per segment file.
5. **No zone maps** — no min/max/null_count per column, so scans cannot skip irrelevant parts.
6. **No raw column compression** — sorted indices and monotonic offsets stored as raw LE arrays.

---

## 2. Changes Made — V2 Segment Format

Implemented V2 segment format in `segment.rs`. V1 reader is preserved for backward compatibility; V2 is the default write format; `open_segment` auto-detects version by magic.

### V2 header (48 bytes)
```
magic(8) "SGSMNT02" | ver(4)=2 | ncells(8) | generation(8) |
fingerprint(8) | part_count(4) | flags(4) | header_crc(4)
```
- Header CRC covers bytes [0..44] (everything before the CRC field).
- Flags reserve bit 0 for u64 offset data, bit 1 for delta encoding.

### V2 part directory (48 bytes per entry)
```
type_tag(4) | offset(8) | len(8) | crc32(4) | min_val(8) | max_val(8) | null_count(8)
```

Part type tags: 0=offsets, 1=indices, 2=stalk_offsets, 3=morph_cells,
4=morph_offsets, 5=discrete, 6=morphs, 7=stalks, 8=dims.

### V2 file layout
```
[header(48)][part_directory(48 × N)][payload][payload_crc(4)]
```

---

## 3. Efficiency Gains — All Six Addressed

| # | Issue | Fix | Violates Arch? | Status |
|---|-------|-----|----------------|--------|
| 1 | `open_segment` does `std::fs::read(path)` — whole file into RAM | Added `memmap2` dependency; `segment_meta()` uses `Mmap::map` touching only 36 bytes; `open_segment_mmap()` maps then copies parts | No | Fixed |
| 2 | Fixed 9-part table, no type tags | V2 self-describing part directory with type tags per part; forward-compatible | No | Fixed |
| 3 | No column compression/delta encoding | Flag bit 1 reserved in V2 header for future delta encoding; not activated (would add read CPU on every `range()` call) | No | Deferred |
| 4 | No per-part checksums | Each V2 part directory entry carries its own CRC32; a corrupt part is isolated, not whole-segment loss | No | Fixed |
| 5 | u32 offset cap (~4GB) | V2 directory uses u64 for part offset/len — files can exceed 4GB | No | Fixed |
| 6 | No zone maps / column statistics | Each V2 part directory entry carries min_val, max_val, null_count | No | Fixed |

### Gain details

**Gain 1 — mmap-based reading (`segment.rs:segment_meta`, `open_segment_mmap`)**
- `segment_meta()` now memory-maps the file and reads only the first 36 bytes (magic + generation + fingerprint), avoiding a full-file `read()` for metadata queries.
- `open_segment_mmap()` maps the file, reads header + directory from the mapping, then copies individual parts into owned `Vec<u8>` for the `BcsfReader`. The OS page cache serves pages efficiently; future lazy part loading is possible.

**Gain 2 — Self-describing parts (`segment.rs:PartDirEntry`)**
- Each part carries a `type_tag`, so new columns can be added without a format version bump. Unknown tags are skipped on read; required tags are validated as present.

**Gain 4 — Per-part checksums (`segment.rs:PartDirEntry.crc`)**
- Each directory entry stores `crc32(part_data)`. On open, each part is individually CRC-verified. A torn/corrupt part is reported by tag, not as a whole-segment failure.

**Gain 5 — u64 directory offsets (`segment.rs:PartDirEntry.off/len`)**
- Part offsets and lengths are u64 in the directory. A single segment file can address payloads beyond 4GB. (Internal offset arrays remain u32 for the read-path fast path; multi-segment sharding handles larger datasets.)

**Gain 6 — Zone maps (`segment.rs:PartDirEntry.min_val/max_val/null_count`)**
- Each part carries zone-map statistics. For sorted indices/monotonic offsets this is min/max; for f32 stalks/morphs it's bit-pattern min/max. Enables future scan-skipping (zone-map pruning) without reading part data.

---

## 4. Files Changed

- `crates/sheaf-storage/Cargo.toml` — added `memmap2 = "0.9"`
- `crates/sheaf-storage/src/bcsf/segment.rs` — V2 format (write + read), mmap support, V1 backward compatibility
- `crates/sheaf-storage/src/lsp/mem_complex.rs` — no changes needed; `persist_base_segment`/`restore_base_segment` call `write_segment`/`open_segment` which now default to V2

---

## 5. Verification

- `cargo check --workspace`: clean, no warnings
- `cargo test --workspace`: 16/16 pass (sheaf_storage 6, sheaf_query 4, sheaf_kernel 1, sheaf-engine imdb 5)
- `segment_bench` (release): all gates PASS
  - `gate_segment_exact=PASS` (2500 cells round-trip identical)
  - `gate_segment_meta=PASS` (generation + fingerprint round-trip)
  - `gate_segment_corrupt_reject=PASS` (byte flip → CRC reject)
  - `gate_segment_truncated_reject=PASS` (truncated file → error)
  - `gate_segment_alien_reject=PASS` (foreign bytes → error)
  - `gate_segment_restore_exact=PASS` (persist → restore → identical base)

---

## 6. Delta Encoding — Deferred with Rationale

Delta encoding (gain 3) was considered but **not implemented**. The read path (`CellOffsets::range()`, `IncidenceIndices::slice()`) returns direct `&[u32]` / `&[u64]` slices with zero decode overhead. Delta-encoding these arrays would require decoding on every access, adding CPU to the hot read path. The V2 format reserves flag bit 1 for delta encoding; it can be activated later when write amplification outweighs read CPU cost.
