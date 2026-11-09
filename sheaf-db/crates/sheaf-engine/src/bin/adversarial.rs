// crates/sheaf-engine/src/bin/adversarial.rs — out-of-box adversarial E2E.
// Attacks Phase-2 paths the happy-path bench never covers (A1 remap holes,
// A2 delete×merge races, A3 isolation, A4 WAL bit-flips, A5 txn matrix, A6
// shadow fuzz, A7 prune churn, A8 degenerates). Deterministic fixtures;
// every section asserts exact state vs an independent model.
use std::collections::{BTreeSet, HashMap, HashSet}; use std::sync::Arc;
use sheaf_engine::transaction::manager::{CommitOutcome, TxnManager};
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::lsp::compactor::Compactor;
use sheaf_storage::lsp::mem_complex::MemComplex;
use sheaf_storage::lsp::wal::{self, WalWriter};

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state; x ^= x << 13; x ^= x >> 7; x ^= x << 17; *state = x; x
}
fn d64(v: u64) -> [u8; 16] {
    let mut b = [0u8; 16]; b[..8].copy_from_slice(&v.to_le_bytes()); b
}
fn orig_of(payload: &[u8]) -> u64 { u64::from_le_bytes(payload[..8].try_into().unwrap()) }
const HEMPTY: [f32; 0] = [];

// Shadow model mirroring engine semantics: inserts always land (even over
// dead faces); deletes kill currently-indexed hypers containing f.
#[derive(Default)]
struct Model {
    live: HashMap<u64, Vec<u64>>,
    dead_faces: HashSet<u64>,
}
impl Model {
    fn insert(&mut self, tx: u64, faces: &[u64]) {
        let mut f = faces.to_vec(); f.sort_unstable(); self.live.insert(tx, f);
    }
    fn delete_face(&mut self, f: u64) {
        self.dead_faces.insert(f); self.live.retain(|_, fs| !fs.contains(&f));
    }
}
type Reader = sheaf_storage::bcsf::reader::BcsfReader; type Snap = sheaf_storage::lsp::snapshot::Snapshot;
// Exact state of a HELD snapshot: (tx, sorted orig faces) over LIVE dim-2
// cells in base + batches (batch faces resolve via the snapshot's own base).
fn dump_snap(snap: &Snap) -> BTreeSet<(u64, Vec<u64>)> {
    let mut out = BTreeSet::new();
    let mut walk = |r: &Arc<Reader>, live: &[bool], res: &Reader| {
        for (i, &lv) in live.iter().enumerate() {
            if lv && r.dim(i) == 2 {
                let mut fs: Vec<u64> = r.boundary(i).iter().map(|id| orig_of(res.discrete(*id as usize))).collect();
                fs.sort_unstable();
                out.insert((orig_of(r.discrete(i)), fs));
            }
        }
    };
    // B4: liveness is resolved from the versioned tombstones as of the
    // snapshot's generation (no copied bitmap).
    let base_live: Vec<bool> = (0..snap.base.len()).map(|i| snap.base_alive(i)).collect();
    walk(&snap.base, &base_live, &snap.base);
    for b in &snap.batches {
        let base = snap.base.clone();
        let live: Vec<bool> = (0..b.reader.len()).map(|l| snap.batch_alive(b, l)).collect();
        walk(&b.reader, &live, &base);
    }
    out
}
// Exact engine state via fresh snapshot. Caller seals first for full equality
// cells are invisible by design). Caller seals first for full equality.
fn dump_state(mem: &MemComplex) -> BTreeSet<(u64, Vec<u64>)> {
    dump_snap(&mem.snapshot())
}
fn model_set(m: &Model) -> BTreeSet<(u64, Vec<u64>)> {
    m.live.iter().map(|(t, f)| (*t, f.clone())).collect()
}
// Orig-id -> current positional index from a fresh discrete scan (positional
// ids go stale across dropping merges; translate immediately before use).
fn face_map(mem: &MemComplex) -> HashMap<u64, usize> {
    let snap = mem.snapshot();
    (0..snap.base.len())
        .filter(|i| snap.base_alive(*i) && snap.base.dim(*i) == 0)
        .map(|i| (orig_of(snap.base.discrete(i)), i))
        .collect()
}
// Diff-reporting equality (caller reports the canonical adv_ line).
fn assert_eq_sets(name: &'static str, engine: &BTreeSet<(u64, Vec<u64>)>, model: &BTreeSet<(u64, Vec<u64>)>) -> bool {
    if engine == model {
        return true;
    }
    println!("  {} mismatch: engine {} vs model {} cells", name, engine.len(), model.len());
    for e in engine.difference(model).take(5) {
        println!("  {} engine-only: tx={} faces={:?}", name, e.0, e.1);
    }
    for e in model.difference(engine).take(5) {
        println!("  {} model-only:  tx={} faces={:?}", name, e.0, e.1);
    }
    false
}
// Race-robust invariant: every LIVE hypercell references only LIVE faces
// (violated iff a tombstone was lost on ANY path). Returns violations.
fn check_integrity(mem: &MemComplex) -> usize {
    let snap = mem.snapshot();
    // B4: liveness from the versioned tombstones (no copied bitmap).
    let live: HashSet<u64> = (0..snap.base.len())
        .filter(|i| snap.base_alive(*i) && snap.base.dim(*i) == 0)
        .map(|i| orig_of(snap.base.discrete(i)))
        .collect();
    let mut bad = 0;
    let mut walk = |r: &Arc<Reader>, lv: &[bool]| {
        for (i, &l) in lv.iter().enumerate() {
            if l && r.dim(i) == 2 {
                for id in r.boundary(i) {
                    bad += !live.contains(&orig_of(snap.base.discrete(*id as usize))) as usize;
                }
            }
        }
    };
    let base_live: Vec<bool> = (0..snap.base.len()).map(|i| snap.base_alive(i)).collect();
    walk(&snap.base, &base_live);
    for b in &snap.batches {
        let live: Vec<bool> = (0..b.reader.len()).map(|l| snap.batch_alive(b, l)).collect();
        walk(&b.reader, &live);
    }
    bad
}
fn fresh_mem(tag: &str, base_cells: usize, shards: usize) -> (Arc<MemComplex>, Arc<Reader>) {
    let mut b = BcsfBuilder::new();
    for i in 0..base_cells {
        b.push(0, Vec::new(), &d64(i as u64), &[i as f32, 0.0, 0.0, 0.0], &HEMPTY);
    }
    let base = Arc::new(b.finish());
    let p = std::env::temp_dir().join(format!("sheaf_adv_{}_{}.wal", std::process::id(), tag));
    let _ = std::fs::remove_file(&p);
    (Arc::new(MemComplex::new(base.clone(), shards, &p).unwrap()), base)
}
fn merge_all(mem: &Arc<MemComplex>) {
    let (comp, h) = Compactor::spawn(mem.clone());
    comp.merge().expect("adversarial merge failed");
    drop(comp);
    h.join().unwrap();
}

fn main() {
    let mut results: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| {
        println!("adv_{}={}", name, if ok { "PASS" } else { "FAIL" });
        results.push((name, ok));
    };

    // A1: remap identity across a merge with tombstone holes.
    {
        let (mem, _) = fresh_mem("a1", 500, 2);
        let mut model = Model::default();
        let mut st = 0xA1;
        for tx in 0..200u64 {
            let fs: Vec<u64> = (0..4).map(|_| xorshift(&mut st) % 500).collect();
            mem.insert_hypercell((tx % 2) as usize, &fs, tx, [0.2, -0.38]);
            model.insert(tx, &fs);
        }
        for f in (0..500u64).step_by(10).take(50) {
            mem.delete_base(f as usize);
            model.delete_face(f);
        }
        mem.seal_all_open();
        merge_all(&mem);
        check("a1_remap_identity", assert_eq_sets("a1", &dump_state(&mem), &model_set(&model)) && mem.snapshot().base.validate().is_ok());
    }

    // A2: delete x merge race stress, 10 seeds. Exact intent is unverifiable
    // under true concurrency, so the gate asserts race-robust invariants
    // (completion, validate, referential integrity, repeatability).
    {
        let mut ok_all = true;
        for seed in 0..10u64 {
            let (mem, _) = fresh_mem(&format!("a2_{}", seed), 200, 2);
            let mut st = 0xA200 + seed;
            for tx in 0..100u64 {
                let fs = vec![xorshift(&mut st) % 200, xorshift(&mut st) % 200];
                mem.insert_hypercell((tx % 2) as usize, &fs, tx, [0.1, 0.2]);
            }
            mem.seal_all_open();
            let pre = dump_state(&mem);
            let snap_held = mem.snapshot();
            let del_orig: Vec<u64> = {
                let mut s = 0xB00 + seed;
                (0..20).map(|_| xorshift(&mut s) % 200).collect::<HashSet<_>>().into_iter().collect()
            };
            let (mut evens, mut odds): (Vec<u64>, Vec<u64>) = (Vec::new(), Vec::new());
            for (i, &o) in del_orig.iter().enumerate() {
                if i % 2 == 0 {
                    evens.push(o);
                } else {
                    odds.push(o);
                }
            }
            let (m1, m2) = (mem.clone(), mem.clone());
            std::thread::scope(|s| {
                let h1 = s.spawn(|| {
                    for o in &evens {
                        if let Some(&cur) = face_map(&m1).get(o) {
                            m1.delete_base(cur);
                        }
                    }
                });
                let h2 = s.spawn(|| {
                    for o in &odds {
                        if let Some(&cur) = face_map(&m2).get(o) {
                            m2.delete_base(cur);
                        }
                    }
                });
                let m3 = mem.clone();
                let h3 = s.spawn(move || {
                    for _ in 0..3 {
                        let (c, h) = Compactor::spawn(m3.clone());
                        let _ = c.merge();
                        drop(c);
                        h.join().unwrap();
                    }
                });
                h1.join().unwrap();
                h2.join().unwrap();
                h3.join().unwrap();
            });
            ok_all &= dump_snap(&snap_held) == pre;
            drop(snap_held);
            mem.seal_all_open();
            merge_all(&mem);
            let viol = check_integrity(&mem);
            if viol != 0 {
                println!("  a2 seed {} integrity violations: {}", seed, viol);
            }
            ok_all &= viol == 0 && mem.snapshot().base.validate().is_ok();
        }
        check("a2_delete_merge_race", ok_all);
    }

    // A3: snapshot isolation across two merges (pinned vs final, split lines).
    {
        let (mem, _) = fresh_mem("a3", 100, 1);
        let mut model = Model::default();
        for tx in 0..50u64 {
            let fs = vec![tx % 100, (tx * 7) % 100];
            mem.insert_hypercell(0, &fs, tx, [0.3, 0.4]);
            model.insert(tx, &fs);
        }
        mem.seal_all_open();
        let s1 = mem.snapshot();
        let snap_dump = dump_state(&mem);
        for tx in 50..100u64 {
            let fs = vec![(tx * 3) % 100, (tx * 11) % 100];
            mem.insert_hypercell(0, &fs, tx, [0.5, 0.6]);
            model.insert(tx, &fs);
        }
        for f in [3usize, 17, 42] {
            mem.delete_base(f);
            model.delete_face(f as u64);
        }
        merge_all(&mem);
        merge_all(&mem); // empty second merge: must be a fixed point
        check("a3_snapshot_pinned", dump_snap(&s1) == snap_dump);
        drop(s1);
        mem.seal_all_open();
        merge_all(&mem);
        check("a3_final_equality", assert_eq_sets("a3_final", &dump_state(&mem), &model_set(&model)));
    }

    // A4: WAL bit-flips (payload / length / magic) -> exact prefix + truncated.
    {
        let mut ok_all = true;
        for (tag, flip_at, expect_n) in [("payload", 12 + 5 * 20 + 12 + 3, 5), ("len", 12 + 3 * 20 + 4, 3), ("magic", 12 + 7 * 20, 7)] {
            let p = std::env::temp_dir().join(format!("sheaf_adv_a4_{}_{}.wal", std::process::id(), tag));
            let _ = std::fs::remove_file(&p);
            {
                let mut w = WalWriter::create(&p).unwrap();
                for i in 0..10u64 {
                    w.append(&i.to_le_bytes()).unwrap();
                }
                w.flush().unwrap();
            }
            {
                use std::io::{Read, Seek, SeekFrom, Write};
                let mut f = std::fs::OpenOptions::new().read(true).write(true).open(&p).unwrap();
                let mut one = [0u8; 1];
                f.seek(SeekFrom::Start(flip_at as u64)).unwrap();
                f.read_exact(&mut one).unwrap();
                f.seek(SeekFrom::Start(flip_at as u64)).unwrap();
                f.write_all(&[one[0] ^ 0xFF]).unwrap();
                f.flush().unwrap();
            }
            let (recs, trunc) = wal::replay(&p).unwrap();
            ok_all &= recs.len() == expect_n && trunc;
            ok_all &= recs.iter().enumerate().all(|(i, r)| u64::from_le_bytes(r[..8].try_into().unwrap()) == i as u64);
        }
        check("a4_wal_bitflip", ok_all);
    }

    // A5: txn conflict matrix (barrier forces genuine overlap; without it a
    // serialized interleave lets both commit legitimately and XOR is a coin).
    {
        let (mem, _) = fresh_mem("a5", 300, 2);
        let mgr = Arc::new(TxnManager::new(mem.clone(), 64));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let mut oka = true;
        for r in 0..20u64 {
            let (m1, m2, b1, b2) = (mgr.clone(), mgr.clone(), barrier.clone(), barrier.clone());
            let f = vec![r * 10, r * 10 + 1];
            let outs = std::thread::scope(|s| {
                let h1 = s.spawn(|| {
                    let mut t = m1.begin();
                    m1.insert(&mut t, f.clone(), 100_000 + r, [0.1, 0.2]);
                    b1.wait();
                    m1.commit(t)
                });
                let h2 = s.spawn(|| {
                    let mut t = m2.begin();
                    m2.insert(&mut t, f.clone(), 200_000 + r, [0.1, 0.2]);
                    b2.wait();
                    m2.commit(t)
                });
                [h1.join().unwrap(), h2.join().unwrap()]
            });
            let wins = outs.iter().filter(|o| **o == CommitOutcome::Committed { cells: 1 }).count();
            if wins != 1 {
                println!("  a5a round {} outcomes: {:?} {:?}", r, outs[0], outs[1]);
            }
            oka &= wins == 1;
        }
        check("a5a_txn_overlap_xor", oka);
        let mut okb = true;
        {
            let mut t1 = mgr.begin();
            let mut t2 = mgr.begin();
            mgr.insert(&mut t1, vec![290, 291], 301, [0.1, 0.2]);
            mgr.insert(&mut t2, vec![292, 293], 302, [0.1, 0.2]);
            okb &= mgr.commit(t1) == CommitOutcome::Committed { cells: 1 };
            okb &= mgr.commit(t2) == CommitOutcome::Committed { cells: 1 };
        }
        check("a5b_txn_disjoint", okb);
        let mut okc = true;
        let fp0 = mem.footprint();
        let t0 = mgr.begin();
        okc &= mgr.commit(t0) == CommitOutcome::Committed { cells: 0 };
        okc &= mem.footprint() == fp0;
        check("a5c_txn_empty", okc);
        let mut okd = true;
        let mut ta = mgr.begin();
        let mut tb = mgr.begin();
        mgr.insert(&mut tb, vec![294, 295], 303, [0.1, 0.2]);
        assert_eq!(mgr.commit(tb), CommitOutcome::Committed { cells: 1 });
        let fp1 = mem.footprint();
        mgr.insert(&mut ta, vec![294, 295], 304, [0.1, 0.2]);
        okd &= mgr.commit(ta) == CommitOutcome::AbortedConflict;
        okd &= mem.footprint() == fp1;
        check("a5d_txn_abort_clean", okd);
    }

    // A6: randomized shadow-model fuzz (250 rounds). Every op translates
    // orig->current through a fresh map (rebuilt after merges AND deletes).
    {
        let (mem, _) = fresh_mem("a6", 64, 2);
        let mut model = Model::default();
        let mut st = 0xF00D;
        let mut tx = 0u64;
        let mut ok_all = true;
        let mut fmap = face_map(&mem);
        for round in 0..250 {
            let op = xorshift(&mut st) % 100;
            if op < 70 {
                let deg = 2 + (xorshift(&mut st) % 3) as usize;
                let orig: Vec<u64> = (0..deg).map(|_| xorshift(&mut st) % 64).collect();
                if let Some(cur) = orig.iter().map(|o| fmap.get(o).copied()).collect::<Option<Vec<_>>>() {
                    mem.insert_hypercell((tx % 2) as usize, &cur.iter().map(|c| *c as u64).collect::<Vec<_>>(), tx, [0.7, 0.8]);
                    model.insert(tx, &orig);
                    tx += 1;
                }
            } else if op < 85 {
                let o = xorshift(&mut st) % 64;
                if let Some(&cur) = fmap.get(&o) {
                    mem.delete_base(cur);
                    model.delete_face(o);
                    fmap = face_map(&mem);
                }
            } else if op < 95 {
                mem.seal_all_open();
                merge_all(&mem);
                fmap = face_map(&mem);
            } else {
                mem.seal_all_open();
                ok_all &= assert_eq_sets("a6_spot", &dump_state(&mem), &model_set(&model));
            }
            if round % 25 == 24 {
                mem.seal_all_open();
                ok_all &= assert_eq_sets("a6_gate", &dump_state(&mem), &model_set(&model));
                ok_all &= mem.snapshot().base.validate().is_ok();
            }
        }
        mem.seal_all_open();
        merge_all(&mem);
        ok_all &= assert_eq_sets("a6_final", &dump_state(&mem), &model_set(&model));
        ok_all &= mem.snapshot().base.validate().is_ok();
        check("a6_shadow_fuzz", ok_all);
    }

    // A7: merge-cycle churn + delete idempotency across prunes. Pre-drop
    // merges are identity remaps, so pre-delete ids are valid; the second
    // delete translates through a fresh map (orig 5 is dead -> clean no-op
    // on both sides; positional delete_base(5) here would kill an unrelated
    // cell — the stale-id footgun, covered by construction).
    {
        let (mem, _) = fresh_mem("a7", 120, 2);
        let mut ok_all = true;
        let mut model = Model::default();
        let mut st = 0xA7;
        for tx in 0..120u64 {
            let fs = vec![xorshift(&mut st) % 120, xorshift(&mut st) % 120];
            mem.insert_hypercell((tx % 2) as usize, &fs, tx, [0.9, 1.0]);
            model.insert(tx, &fs);
        }
        mem.seal_all_open();
        for _ in 0..10 {
            merge_all(&mem); // translations filed + pruned each cycle
        }
        for f in [5usize, 66, 119] {
            mem.delete_base(f);
            model.delete_face(f as u64);
        }
        mem.seal_all_open();
        merge_all(&mem);
        let before = dump_state(&mem);
        let fmap = face_map(&mem);
        assert!(!fmap.contains_key(&5), "orig 5 must be dead after first delete");
        model.delete_face(5);
        mem.seal_all_open();
        merge_all(&mem);
        ok_all &= assert_eq_sets("a7_idempotent", &dump_state(&mem), &before)
            && assert_eq_sets("a7_final", &dump_state(&mem), &model_set(&model));
        check("a7_prune_churn_idempotent", ok_all);
    }

    // A8: degenerates.
    {
        let mut ok_all = true;
        let (mem, _) = fresh_mem("a8a", 0, 1);
        merge_all(&mem);
        ok_all &= mem.snapshot().base.is_empty() && mem.snapshot().base.validate().is_ok();
        let (mem, _) = fresh_mem("a8b", 20, 1);
        for tx in 0..30u64 {
            mem.insert_hypercell(0, &[tx % 20, (tx + 1) % 20], tx, [0.1, 0.2]);
        }
        for f in 0..20usize {
            mem.delete_base(f);
        }
        mem.seal_all_open();
        merge_all(&mem);
        let s = mem.snapshot();
        ok_all &= s.base.is_empty() && s.base.validate().is_ok();
        let (mem, _) = fresh_mem("a8c", 30, 1);
        for tx in 0..40u64 {
            mem.insert_hypercell(0, &[tx % 30, (tx + 5) % 30], tx, [0.1, 0.2]);
        }
        mem.seal_all_open();
        merge_all(&mem);
        let d1 = dump_state(&mem);
        merge_all(&mem);
        ok_all &= dump_state(&mem) == d1;
        // Out-of-range delete documents the panic precondition (face <
        // base_len; TQL must range-check). Poisoned instance is dropped.
        {
            let (mem, _) = fresh_mem("a8d", 10, 1);
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mem.delete_base(999_999)));
            std::panic::set_hook(prev);
            ok_all &= r.is_err();
        }
        // Staging over a dead face then merging must FAIL LOUD (Err), never
        // silently propagate a u32::MAX sentinel into boundaries.
        {
            let (mem, _) = fresh_mem("a8e", 10, 1);
            mem.delete_base(3);
            mem.insert_hypercell(0, &[3, 4], 0, [0.1, 0.2]);
            mem.seal_all_open();
            let (comp, h) = Compactor::spawn(mem.clone());
            let r = comp.merge();
            drop(comp);
            h.join().unwrap();
            ok_all &= r.is_err();
        }
        check("a8_degenerates", ok_all);
    }

    println!("--- adversarial summary ---");
    let fail = results.iter().filter(|(_, ok)| !*ok).count();
    for (n, ok) in &results {
        println!("  adv_{}: {}", n, if *ok { "PASS" } else { "FAIL" });
    }
    if fail == 0 {
        println!("ADVERSARIAL: PASS ({} sections)", results.len());
    } else {
        println!("ADVERSARIAL: FAIL ({}/{})", fail, results.len());
        std::process::exit(1);
    }
}
