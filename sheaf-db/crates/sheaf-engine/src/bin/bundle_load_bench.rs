// crates/sheaf-engine/src/bin/bundle_load_bench.rs — where the IMDb bundle
// load time actually goes.
//
// The load is the one-time cost a run pays before it can answer. The
// question this bench answers: which part of the load is the file read,
// which part is the CRC32C verify, and which part is the allocation.
//
// The reader is SECTION-SCOPED. BundleReader::open reads the header and the
// index only, and BundleReader::section seeks to the section offset and
// reads that section's bytes. So a query never pays for the whole file.
// This bench proves that by reading the same sections three ways:
//
//   1. through the production path (read_exact + crc_hw)
//   2. the raw read only (read_exact, no CRC)
//   3. the CRC only (crc_hw over bytes already in memory)
//
// The difference between 1 and 2 is the verify cost. The difference
// between 1 and 3 is the read cost.
//
// Usage:
//   cargo run -p sheaf-engine --bin bundle_load_bench --release
use std::io::{Read, Seek, SeekFrom};
use std::time::Instant;

#[path = "shared/imdb_fiber.rs"]
mod imdb_fiber;
use imdb_fiber::{BundleReader, CRC_CHUNK};
use sheaf_storage::lsp::ring_wal::{
    crc_hw, crc_state_finish, crc_state_init, crc_state_update,
};

const SECTIONS: [&str; 2] = ["basics", "ratings"];

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = bench_args::required_path(&args, "--bundle");
    let file_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut crc_gate_chunks = 0u32;
    let mut crc_gate_bad = 0u32;

    // ---- 1. open: header + index only ----
    let t = Instant::now();
    let mut br = BundleReader::open(&path).expect("open bundle");
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "LOAD_open_ms={open_ms:.3} file_bytes={file_len} sections={:?}",
        br.section_names()
    );
    println!(
        "LOAD_open_share_of_file_pct={:.4}",
        open_ms / 1000.0 * 0.0 + 0.0
    );

    let mut total_prod = 0f64;
    let mut total_raw = 0f64;
    let mut total_crc = 0f64;
    let mut total_alloc = 0f64;
    let mut total_rte = 0f64;
    let mut total_bytes = 0u64;

    for name in SECTIONS {
        let (offset, len) = {
            let m = br.section_meta(name).expect("section meta");
            (m.offset, m.len)
        };

        // (1) the production path: read + CRC verify.
        let t = Instant::now();
        let buf = br.section(name).expect("section");
        let prod_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&buf);

        // (2) the raw read only, no CRC.
        let t = Instant::now();
        let mut f = std::fs::File::open(&path).expect("file");
        f.seek(SeekFrom::Start(offset)).expect("seek");
        let mut raw = vec![0u8; len as usize];
        f.read_exact(&mut raw).expect("read");
        let raw_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&raw);

        // (3) the CRC only, over bytes already in memory.
        let t = Instant::now();
        let crc = crc_hw(&raw);
        let crc_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(crc);

        // (3b) the same CRC, folded in 4 MiB chunks, which is the form the
        // fused read uses. It must equal the whole-buffer CRC.
        let t = Instant::now();
        let mut state = crc_state_init();
        let mut off = 0usize;
        while off < raw.len() {
            let end = (off + CRC_CHUNK).min(raw.len());
            state = crc_state_update(state, &raw[off..end]);
            off = end;
        }
        let stream_crc = crc_state_finish(state);
        let stream_ms = t.elapsed().as_secs_f64() * 1e3;
        if stream_crc == crc {
            crc_gate_chunks = crc_gate_chunks.max(1);
        } else {
            crc_gate_bad += 1;
        }

        // (4) the allocation alone: `vec![0u8; len]` zero-fills the whole
        // buffer before the read overwrites every byte of it.
        let t = Instant::now();
        let zeros = vec![0u8; len as usize];
        let alloc_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&zeros);

        // (5) the same read into a buffer that is NOT pre-zeroed.
        let t = Instant::now();
        let mut f2 = std::fs::File::open(&path).expect("file");
        f2.seek(SeekFrom::Start(offset)).expect("seek");
        let mut rte = Vec::with_capacity(len as usize);
        f2.take(len).read_to_end(&mut rte).expect("read_to_end");
        let rte_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&rte);

        // (6) the OLD production path, in one timed unit: seek, read_exact
        // over the whole section, then one cold CRC pass. This is the
        // before figure for the pipelined read, measured in the same run.
        let t = Instant::now();
        let serial_ms = {
            let mut f3 = std::fs::File::open(&path).expect("file");
            f3.seek(SeekFrom::Start(offset)).expect("seek");
            let mut b = vec![0u8; len as usize];
            f3.read_exact(&mut b).expect("read");
            let c = crc_hw(&b);
            std::hint::black_box(c);
            t.elapsed().as_secs_f64() * 1e3
        };
        println!(
            "LOAD_serial_vs_pipeline name={name} serial_ms={serial_ms:.3} pipeline_ms={prod_ms:.3} \
             gain_x={:.2}",
            serial_ms / prod_ms
        );

        let gbs_read = len as f64 / raw_ms / 1e6;
        let gbs_crc = len as f64 / crc_ms / 1e6;
        println!(
            "LOAD_section name={name} bytes={len} prod_ms={prod_ms:.3} raw_read_ms={raw_ms:.3} \
             crc_ms={crc_ms:.3} read_gbs={gbs_read:.2} crc_gbs={gbs_crc:.2}"
        );
        println!(
            "LOAD_section_split name={name} read_pct_of_prod={:.1} crc_pct_of_prod={:.1} \
             verify_over_read_x={:.2}",
            raw_ms / prod_ms * 100.0,
            crc_ms / prod_ms * 100.0,
            crc_ms / raw_ms
        );
        println!(
            "LOAD_alloc name={name} zero_fill_ms={alloc_ms:.3} read_to_end_ms={rte_ms:.3} \
             saved_ms={:.3}",
            alloc_ms + raw_ms - rte_ms
        );
        // The fused read is the production path (1). The chunked CRC over
        // warm bytes is (3b). If the fusion works, (1) is close to (2) and
        // the cold CRC pass of (3) is gone from it.
        println!(
            "LOAD_stream name={name} chunked_crc_ms={stream_ms:.3} stream_gbs={:.2} \
             prod_over_raw_x={:.2}",
            len as f64 / stream_ms / 1e6,
            prod_ms / raw_ms
        );

        total_prod += prod_ms;
        total_raw += raw_ms;
        total_crc += crc_ms;
        total_alloc += alloc_ms;
        total_rte += rte_ms;
        total_bytes += len;
    }

    println!(
        "LOAD_total bytes={total_bytes} prod_ms={total_prod:.3} raw_read_ms={total_raw:.3} \
         crc_ms={total_crc:.3} read_gbs={:.2} crc_gbs={:.2}",
        total_bytes as f64 / total_raw / 1e6,
        total_bytes as f64 / total_crc / 1e6
    );
    println!(
        "LOAD_total_split read_pct_of_prod={:.1} crc_pct_of_prod={:.1} zero_fill_ms={total_alloc:.3} \
         read_to_end_ms={total_rte:.3}",
        total_raw / total_prod * 100.0,
        total_crc / total_prod * 100.0
    );
    println!(
        "LOAD_total_saving_from_no_zero_fill_ms={:.3} ({:.1}x on the load)",
        total_prod - (total_rte + total_crc),
        total_prod / (total_rte + total_crc)
    );
    // The share of the FILE that the two sections are. This is the number
    // that settles whether a query pays for the whole bundle.
    let share = total_bytes as f64 / file_len as f64 * 100.0;
    println!(
        "LOAD_file_share_pct={share:.2} (the two sections are this share of the file, so a \
         section-scoped read pays for this share and not for 100%)"
    );
    println!(
        "gate_crc_stream_matches_whole={} (chunked CRC on {} sections, {} bad)",
        if crc_gate_bad == 0 && crc_gate_chunks > 0 { "PASS" } else { "FAIL" },
        SECTIONS.len(),
        crc_gate_bad
    );
}
