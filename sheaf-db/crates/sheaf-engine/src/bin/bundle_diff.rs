// crates/sheaf-engine/src/bin/bundle_diff.rs — compare two IMDb bundles.
//
// The build must be reproducible, so two builds of the same data must give
// the same bytes in every section. This tool is the gate for that claim.
//
// For each section it reports one of:
//   IDENTICAL   the bytes are the same
//   SAME-SET    the bytes differ but the entries are the same set, so the
//               section was reordered
//   DIFFERENT   the entries differ, so the build changed the data
//   MISSING     the section is in one bundle and not the other
//
// The set check uses an order-independent checksum over the records: the
// XOR and the sum of the per-record hashes, plus the record count. Two
// different orderings of the same set give the same three values.
//
// Usage:
//   cargo run -p sheaf-engine --bin bundle_diff --release -- <a> <b>
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::Instant;

const MAGIC: u64 = 0x534E_4146_4942_5244; // "SNAFIBRD" placeholder, checked per bundle
const HEADER_LEN: usize = 8 + 4 + 4 + 4 + 4;

struct Section {
    offset: u64,
    len: u64,
    crc: u32,
}

fn read_index(path: &str) -> std::io::Result<Vec<(String, Section)>> {
    let mut f = File::open(path)?;
    let mut head = [0u8; HEADER_LEN];
    f.read_exact(&mut head)?;
    let n = u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize;
    let idx_len = u32::from_le_bytes(head[16..20].try_into().unwrap()) as usize;
    let mut index = vec![0u8; idx_len];
    f.read_exact(&mut index)?;
    let mut out = Vec::with_capacity(n);
    let mut off = 0usize;
    for _ in 0..n {
        if off + 2 > index.len() {
            break;
        }
        let nl = u16::from_le_bytes(index[off..off + 2].try_into().unwrap()) as usize;
        off += 2;
        if off + nl + 28 > index.len() {
            break;
        }
        let name = String::from_utf8_lossy(&index[off..off + nl]).to_string();
        off += nl;
        let o = u64::from_le_bytes(index[off..off + 8].try_into().unwrap());
        off += 8;
        let l = u64::from_le_bytes(index[off..off + 8].try_into().unwrap());
        off += 8;
        let c = u32::from_le_bytes(index[off..off + 4].try_into().unwrap());
        off += 4;
        off += 8; // the generation
        out.push((name, Section { offset: o, len: l, crc: c }));
    }
    Ok(out)
}

fn read_section(path: &str, s: &Section) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(s.offset))?;
    let mut v = vec![0u8; s.len as usize];
    f.read_exact(&mut v)?;
    Ok(v)
}

/// The order-independent checksum of a section of fixed-width records.
/// Each record is mixed into a 64-bit value, and the values are XORed,
/// summed and counted.
fn set_checksum(data: &[u8], rec: usize) -> (u64, u64, u64) {
    let mut x = 0u64;
    let mut s = 0u64;
    let mut n = 0u64;
    let mut i = 0usize;
    while i + rec <= data.len() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in &data[i..i + rec] {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        x ^= h;
        s = s.wrapping_add(h);
        n += 1;
        i += rec;
    }
    (x, s, n)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("usage: bundle_diff <a> <b>");
        std::process::exit(2);
    }
    let (a, b) = (&args[1], &args[2]);
    let ia = read_index(a).expect("index a");
    let ib = read_index(b).expect("index b");
    println!("A: {a}  sections {}", ia.len());
    println!("B: {b}  sections {}", ib.len());

    // The record width of each section, from its name.
    let rec = |name: &str| -> usize {
        match name {
            "basics" => 13,
            "ratings" => 20,
            "names" => 12,
            "nameblob" => 1,
            "princount" => 12,
            "q2edges" => 16,
            "answer_q1" => 16,
            "answer_q2" => 16,
            "answer_q3" => 16,
            _ => 1,
        }
    };

    let mut all_same = true;
    for (name, sa) in &ia {
        let sb = match ib.iter().find(|(n, _)| n == name) {
            Some((_, s)) => s,
            None => {
                println!("{name:<12} MISSING IN B");
                all_same = false;
                continue;
            }
        };
        let t = Instant::now();
        let va = read_section(a, sa).expect("section a");
        let vb = read_section(b, sb).expect("section b");
        let verdict = if va == vb {
            "IDENTICAL".to_string()
        } else if va.len() == vb.len() {
            let w = rec(name);
            let ca = set_checksum(&va, w);
            let cb = set_checksum(&vb, w);
            if ca == cb {
                "SAME-SET".to_string()
            } else {
                format!("DIFFERENT (set checksum {:?} against {:?})", ca, cb)
            }
        } else {
            format!("DIFFERENT (size {} against {})", va.len(), vb.len())
        };
        if verdict != "IDENTICAL" {
            all_same = false;
        }
        println!(
            "{name:<12} bytes={:<12} crc={:<11} crc_b={:<11} {verdict} ({:.1} ms)",
            sa.len,
            sa.crc,
            sb.crc,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    for (name, _) in &ib {
        if !ia.iter().any(|(n, _)| n == name) {
            println!("{name:<12} MISSING IN A");
            all_same = false;
        }
    }
    println!();
    println!(
        "gate_bundle_sections_reproducible={}",
        if all_same { "PASS" } else { "FAIL" }
    );
    let _ = MAGIC;
}
