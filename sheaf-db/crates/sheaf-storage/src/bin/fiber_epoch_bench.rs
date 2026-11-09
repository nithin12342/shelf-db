// crates/sheaf-storage/src/bin/fiber_epoch_bench.rs — N3 + N4 gates.
//
// N3: a fiber built at epoch E, loaded against epoch E+1 FAILS LOUD (never
//     answers); the scan fallback returns the exact same aggregate.
// N4: 1,000 writes re-sort only the DIRTY domains (re-sort count ==
//     dirty domains), and the post-write answers are exact.
//
// Rows = gate, Columns = result.
use sheaf_storage::bcsf::fiber_domain::FiberDomains;
use sheaf_storage::lsp::fiber_epoch::{decide, Epoch, EpochMismatch, FiberDecision};

const KEYS: usize = 200_000;
const WRITES: usize = 1_000;
// The production domain size is 64 MB; the test scales it down so the
// fixture actually spans MANY domains (a single-domain fixture would make
// the re-sort gate trivially true).
const DOMAIN_BYTES: usize = 1 << 20; // 1 MB scaled domains (64 MB in production)

fn main() {
    // ---------------- N3: epoch stamp ----------------
    let built = Epoch::new(7, 0xABCD_0001);
    let same = Epoch::new(7, 0xABCD_0001);
    let advanced = Epoch::new(8, 0xABCD_0001);
    let changed = Epoch::new(7, 0xABCD_0002);

    let use_ok = matches!(decide(&built, &same), FiberDecision::Use);
    let adv = decide(&built, &advanced);
    let chg = decide(&built, &changed);
    let fails_loud = matches!(
        adv,
        FiberDecision::FallbackToScan(EpochMismatch::EpochAdvanced { .. })
    ) && matches!(
        chg,
        FiberDecision::FallbackToScan(EpochMismatch::SourceChanged { .. })
    );
    let adv_msg = match adv {
        FiberDecision::Use => "use".to_string(),
        FiberDecision::FallbackToScan(e) => e.to_string(),
    };
    let chg_msg = match chg {
        FiberDecision::Use => "use".to_string(),
        FiberDecision::FallbackToScan(e) => e.to_string(),
    };
    println!(
        "gate_epoch_mismatch_fails_loud={} (advanced={adv_msg} | source_changed={chg_msg})",
        if fails_loud { "PASS" } else { "FAIL" }
    );
    println!(
        "gate_epoch_same_usable={}",
        if use_ok { "PASS" } else { "FAIL" }
    );

    // the fallback (a scan) must return the exact same aggregate
    let entries: Vec<(u64, u32)> = (0..KEYS as u64).map(|k| (k, (k % 1000) as u32)).collect();
    let mut fiber = FiberDomains::build(&entries, DOMAIN_BYTES);
    let fiber_agg = fiber.aggregate();
    let scan_agg: u64 = entries.iter().map(|(_, v)| *v as u64).sum();
    let same_agg = fiber_agg == scan_agg;
    println!(
        "gate_epoch_fallback_aggregate_exact={} (fiber={fiber_agg} scan={scan_agg})",
        if same_agg { "PASS" } else { "FAIL" }
    );

    // ---------------- N4: per-domain incremental update ----------------
    let domains = fiber.domains.len();
    // 1,000 writes spread over the FIRST TWO domains only: the other
    // domains stay clean and must NOT be re-sorted.
    let writes_in = 2usize;
    // stride chosen so every write lands inside the first `writes_in`
    // domains (the rest must stay clean)
    let span = fiber.domains[writes_in - 1].key_hi - fiber.domains[0].key_lo;
    let stride = (span / WRITES as u64).max(1);
    for w in 0..WRITES {
        let key = fiber.domains[0].key_lo + (w as u64) * stride;
        fiber.write(key, 999_000 + w as u32);
    }
    let dirty = fiber.dirty_domains();
    let resorts = fiber.flush();
    println!("fiber_update: writes={WRITES} domains={domains} dirty_domains={dirty} resorts={resorts}");
    let count_ok = resorts == dirty as u64
        && resorts == writes_in as u64
        && resorts < WRITES as u64
        && domains > writes_in;
    println!(
        "gate_fiber_resorts_only_dirty_domains={} (resorts={resorts} dirty={dirty} writes={WRITES})",
        if count_ok { "PASS" } else { "FAIL" }
    );

    // post-write answers are exact: the fiber lookup == the scan lookup
    let mut exact = true;
    for w in 0..WRITES {
        let key = fiber.domains[0].key_lo + (w as u64) * stride;
        let a = fiber.lookup(key);
        let b = fiber.scan_lookup(key);
        if a != b || a != Some(999_000 + w as u32) {
            exact = false;
            println!("  MISMATCH key={key} fiber={a:?} scan={b:?}");
            break;
        }
    }
    println!(
        "gate_fiber_post_write_answers_exact={}",
        if exact { "PASS" } else { "FAIL" }
    );

    // the aggregate after the writes still matches a full scan
    let mut expect: std::collections::HashMap<u64, u32> =
        entries.iter().copied().collect();
    for w in 0..WRITES {
        let key = fiber.domains[0].key_lo + (w as u64) * stride;
        expect.insert(key, 999_000 + w as u32);
    }
    let scan_agg2: u64 = expect.values().map(|v| *v as u64).sum();
    let agg_ok = fiber.aggregate() == scan_agg2;
    println!(
        "gate_fiber_post_write_aggregate_exact={} (fiber={} scan={scan_agg2})",
        if agg_ok { "PASS" } else { "FAIL" },
        fiber.aggregate()
    );

    let all = fails_loud && use_ok && same_agg && count_ok && exact && agg_ok;
    println!("FIBER_EPOCH_BENCH: {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}
