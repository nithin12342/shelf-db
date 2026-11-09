// crates/sheaf-storage/src/lsp/colimit.rs — background canonical projection
// MATH-UPGRADES Stage M2. Depends on graded_log (sealed frames) + builder.
// The colimit map q: Free(P) -> P/~: replays sealed graded logs in dim
// order, assigns stable gids, resolves boundary refs (all lower-dim, hence
// already assigned — down-closure by construction), and packs immutable
// BCSF segments via BcsfBuilder. Runs at compaction, never on ingest.
// Frame format: [dim u8][id u64LE][nboun u32LE][(dim u8,id u64LE) x nboun]
// [nstalk u32LE][stalk f32LE x nstalk][discrete 16B, zero-padded].
use std::collections::HashMap;

use super::graded_log::GradedWal;
use crate::bcsf::builder::BcsfBuilder;
use crate::bcsf::reader::BcsfReader;

pub fn encode_cell(dim: u8, id: u64, boundary: &[(u8, u64)], stalk: &[f32], discrete: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.push(dim);
    b.extend_from_slice(&id.to_le_bytes());
    b.extend_from_slice(&(boundary.len() as u32).to_le_bytes());
    for (d, i) in boundary {
        b.push(*d);
        b.extend_from_slice(&i.to_le_bytes());
    }
    b.extend_from_slice(&(stalk.len() as u32).to_le_bytes());
    for v in stalk {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let mut db = [0u8; 16];
    let n = discrete.len().min(16);
    db[..n].copy_from_slice(&discrete[..n]);
    b.extend_from_slice(&db);
    b
}

/// Decoded cell frame: (dim, id, boundary refs, stalk, discrete slab).
pub type DecodedCell = (u8, u64, Vec<(u8, u64)>, Vec<f32>, [u8; 16]);

pub fn decode_cell(f: &[u8]) -> Option<DecodedCell> {
    if f.len() < 13 {
        return None;
    }
    let dim = f[0];
    let id = u64::from_le_bytes(f[1..9].try_into().ok()?);
    let nb = u32::from_le_bytes(f[9..13].try_into().ok()?) as usize;
    let mut pos = 13usize;
    let mut boundary = Vec::with_capacity(nb);
    for _ in 0..nb {
        if pos + 9 > f.len() {
            return None;
        }
        boundary.push((f[pos], u64::from_le_bytes(f[pos + 1..pos + 9].try_into().ok()?)));
        pos += 9;
    }
    if pos + 4 > f.len() {
        return None;
    }
    let ns = u32::from_le_bytes(f[pos..pos + 4].try_into().ok()?) as usize;
    pos += 4;
    if pos + ns * 4 + 16 > f.len() {
        return None;
    }
    let mut stalk = Vec::with_capacity(ns);
    for k in 0..ns {
        stalk.push(f32::from_le_bytes(f[pos + k * 4..pos + k * 4 + 4].try_into().ok()?));
    }
    pos += ns * 4;
    let mut db = [0u8; 16];
    db.copy_from_slice(&f[pos..pos + 16]);
    Some((dim, id, boundary, stalk, db))
}

/// Project sealed graded logs into one immutable BCSF segment.
/// Panics loud on dangling boundary refs (a sealed lower dim must contain
/// every referenced face — otherwise the seal lied about down-closure).
pub fn project(g: &GradedWal) -> std::io::Result<BcsfReader> {
    let sealed = g.replay_sealed()?;
    let mut builder = BcsfBuilder::new();
    let mut gid_of: HashMap<(u8, u64), usize> = HashMap::new();
    let mut gid = 0usize;
    // Pass 1: assign gids in graded replay order (dims ascend by BTreeMap).
    let mut decoded: Vec<DecodedCell> = Vec::new();
    for (dim, count) in &sealed {
        let _ = count;
        let path = g.dir_for(*dim);
        let (recs, truncated) = super::wal::replay(&path)?;
        assert!(!truncated, "sealed log must replay clean");
        for r in recs {
            let (d, id, b, s, db) = decode_cell(&r).expect("colimit frame corrupt");
            gid_of.insert((d, id), gid);
            gid += 1;
            decoded.push((d, id, b, s, db));
        }
    }
    // Pass 2: push with resolved boundary gids (all lower-dim: assigned).
    for (d, id, b, s, db) in &decoded {
        let mut bnd = Vec::with_capacity(b.len());
        for (bd, bi) in b {
            bnd.push(*gid_of.get(&(*bd, *bi)).unwrap_or_else(|| {
                panic!("dangling boundary ({},{}) of ({},{}) — seal lied", bd, bi, d, id)
            }) as u64);
        }
        builder.push(*d, bnd, db, s, &[]);
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok(), "colimit output must validate");
    Ok(reader)
}
