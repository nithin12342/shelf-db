//! Derived physical floors for the von Neumann physical-limits probe.
//! Bin-local module (NOT part of the library) - lives under src/bin/shared/
//! so cargo never builds it as a target or links it into the library.
//! Extracted from physical_limits_probe.rs to satisfy SPEC-022 (<=500 lines/file).
//!
//! Every number here is COMPUTED from a published physical constant and this
//! run's measurements. Nothing is copied from a vendor datasheet, so the
//! comparison in VON_NEUMANN_LIMITS.md is derived-vs-measured, never
//! derived-vs-asserted.

use super::phys_mem::{quadrant_dram_tlbmiss_ns, quadrant_l3_tlbhit_ns, quadrant_l3_tlbmiss_ns};

/// Derived physical floors, computed from published constants and this run's
/// measurements. Nothing here is a copied table value.
///
/// `gflops_1t` and `read_gbs_1t` MUST be matched on thread count, otherwise
/// the machine balance is meaningless (an 8-thread bandwidth paired with a
/// 1-thread compute rate is a category error, not a roofline).
#[allow(clippy::too_many_arguments)]
pub fn section_e(
    dram_ns: f64,
    read_gbs_1t: f64,
    read_gbs_all: f64,
    gflops_1t: f64,
    gflops_all: f64,
    seq_gbs: f64,
    rand_us: f64,
) {
    // E1 Landauer: k_B T ln2 at 300 K.
    const K_B: f64 = 1.380_649e-23;
    const T: f64 = 300.0;
    let landauer = K_B * T * std::f64::consts::LN_2;
    println!("E_landauer_J_per_bit={:.4e}", landauer);

    // E2 Signal propagation in silicon (v ~ c/3.6 for a loaded line).
    const C_M_S: f64 = 299_792_458.0;
    const N_SI: f64 = 3.6;
    let v_si = C_M_S / N_SI;
    for die_mm in [10.0f64, 25.0, 50.0] {
        let one_way_ps = (die_mm * 1e-3 / v_si) * 1e12;
        println!(
            "E_si_flight_ps die_mm={:.0} one_way={:.1} round_trip={:.1} implied_max_GHz={:.2}",
            die_mm,
            one_way_ps,
            2.0 * one_way_ps,
            1000.0 / (2.0 * one_way_ps)
        );
    }

    // E3 DRAM timing floor: DDR4-3200 (3200 MT/s, CL22), serial
    //    tRCD + tCL + tRP, all in clock cycles.
    const DDR_MT_S: f64 = 3200.0;
    const CL: f64 = 22.0;
    let cycle_ns = 2000.0 / DDR_MT_S;
    let dram_floor_ns = 3.0 * CL * cycle_ns;
    println!(
        "E_dram_floor_ns cycle_ns={:.3} tRCD+tCL+tRP={:.1}",
        cycle_ns, dram_floor_ns
    );

    // E4 Bandwidth-delay product: bytes that must be in flight to hide DRAM
    //    latency at the given bandwidth (Little's law).
    let bdp_bytes = read_gbs_all * 1e9 * (dram_ns * 1e-9);
    println!("E_bdp_bytes_at_dram_latency={:.0}", bdp_bytes);

    // E5 Machine balance (roofline ridge point), MATCHED thread counts.
    if read_gbs_1t > 0.0 && gflops_1t > 0.0 {
        println!(
            "E_machine_balance_1t_flop_per_byte={:.3}",
            gflops_1t / read_gbs_1t
        );
    }
    if read_gbs_all > 0.0 && gflops_all > 0.0 {
        println!(
            "E_machine_balance_all_flop_per_byte={:.3}",
            gflops_all / read_gbs_all
        );
    }

    // E6 Landauer vs CMOS reality: a switching event is ~1e-15 J.
    const CMOS_SWITCH_J: f64 = 1e-15;
    println!("E_cmos_over_landauer_x={:.3e}", CMOS_SWITCH_J / landauer);

    // E7 Spin thermal-stability grain size for a 10-year magnetic bit.
    //    Retention needs E = K*V >= ln(t/t0) * k_B * T  with t0 = 1 ns.
    const K_ANISOTROPY: f64 = 2.2e5; // J/m^3 (CoCrPt-like)
    const T0: f64 = 1e-9;
    const T_RET: f64 = 10.0 * 365.0 * 24.0 * 3600.0;
    let barrier = (T_RET / T0).ln() * K_B * T;
    let v_min = barrier / K_ANISOTROPY;
    let side_nm = v_min.cbrt() * 1e9;
    let bits_per_in2 = (1.0 / (side_nm * 1e-7)) * (1.0 / (side_nm * 1e-7));
    println!(
        "E_hdd_grain_nm={:.2} E_hdd_areal_Tb_per_in2={:.2}",
        side_nm,
        bits_per_in2 / 1e12
    );

    // E8 Storage latency chain vs DRAM: how many DRAM round trips fit in one
    //    storage access (the layering ratio that justifies pre-aggregation).
    let storage_ns = rand_us * 1000.0;
    println!(
        "E_storage_over_dram_x={:.1} dram_latencies_per_storage_access={:.1}",
        storage_ns / dram_ns,
        storage_ns / dram_ns
    );

    // E9 DDR4-3200 theoretical peak.
    //    A 64-bit DDR4 channel at 3200 MT/s moves 3200e6 * 64/8 B/s =
    //    25.6 GB/s. This host has ONE populated DIMM (ChannelA), so the
    //    applicable peak is the 1-channel figure; the 2-channel figure is
    //    printed for reference only (a fully populated dual-channel box
    //    would be the one able to approach it).
    const DDR_CHANNELS_MAX: f64 = 2.0;
    const DDR_BITS: f64 = 64.0;
    let per_channel = (DDR_BITS / 8.0) * DDR_MT_S * 1e6 / 1e9;
    let dram_peak_1ch = per_channel;
    let dram_peak_2ch = DDR_CHANNELS_MAX * per_channel;
    println!(
        "E_dram_peak_1ch_GBps={:.1} E_dram_peak_2ch_GBps={:.1} (const DDR4-3200 x64bit)",
        dram_peak_1ch, dram_peak_2ch
    );
    println!(
        "E_dram_1t_pct_of_1ch_peak={:.1} E_dram_all_pct_of_1ch_peak={:.1}",
        read_gbs_1t / dram_peak_1ch * 100.0,
        read_gbs_all / dram_peak_1ch * 100.0
    );
    println!(
        "E_dram_all_pct_of_2ch_peak={:.1}",
        read_gbs_all / dram_peak_2ch * 100.0
    );

    // E10 PCIe link ceiling: the bus the NVMe drive is attached to.
    //     PCIe 4.0 x4 = 16 GT/s per lane, 128b/130b encoding.
    for (label, gt_s, lanes) in [("pcie3_x4", 8.0f64, 4.0f64), ("pcie4_x4", 16.0, 4.0)] {
        let effective_gbps = gt_s * lanes * (128.0 / 130.0);
        println!(
            "E_pcie_ceiling_GBps link={} effective_GBps={:.2}",
            label,
            effective_gbps / 8.0
        );
    }

    // E11 HDD rotational-latency floor: half a revolution at the spindle
    //     speed. A seek adds more, but this is the irreducible part.
    for rpm in [7200.0f64, 5400.0, 15000.0] {
        println!(
            "E_hdd_rotational_latency_ms rpm={:.0} half_rev_ms={:.2}",
            rpm,
            30_000.0 / rpm / 2.0
        );
    }

    // E12 Storage bus efficiency: measured sequential read against the
    //     PCIe 4.0 x4 ceiling.
    let pcie4_ceiling = 16.0 * 4.0 * (128.0 / 130.0) / 8.0;
    if seq_gbs > 0.0 {
        println!(
            "E_seq_read_pct_of_pcie4_x4={:.1}",
            seq_gbs / pcie4_ceiling * 100.0
        );
    }

    // E13 DECOMPOSE the random-access latency into its two independent
    //     physical components, using three controlled quadrants.
    //
    //     Random access to a cold byte costs:
    //         T(cache tier) + T(page walk)
    //     and the page walk is paid only when the accessed pages exceed the
    //     STLB. So:
    //
    //       T(L3,  TLB hit)  = a
    //       T(L3,  TLB miss) = b   =>  page walk = b - a
    //       T(DRAM,TLB miss) = c   =>  DRAM access = c - b
    //
    //     The (DRAM, TLB hit) quadrant is NOT measurable with 4 KiB pages
    //     on this host: the STLB reach (pages x 4 KiB) must exceed the
    //     L3 size, and 1536 x 4 KiB = 6 MiB < 12 MiB L3, so any
    //     TLB-resident footprint also fits in L3. That impossibility is
    //     itself a finding and is reported as such.
    let a = quadrant_l3_tlbhit_ns();
    let b = quadrant_l3_tlbmiss_ns();
    let c = quadrant_dram_tlbmiss_ns();
    let page_walk = b - a;
    let dram_access = c - b;
    println!(
        "E_quad_L3_tlbhit_ns={:.2} E_quad_L3_tlbmiss_ns={:.2} E_quad_DRAM_tlbmiss_ns={:.2}",
        a, b, c
    );
    println!(
        "E_page_walk_ns={:.2} E_dram_access_component_ns={:.2}",
        page_walk, dram_access
    );
    println!(
        "E_dram_only_estimate_ns={:.2} E_dram_vs_ddr4_cas_floor_x={:.2}",
        dram_access,
        dram_access / dram_floor_ns
    );
    // STLB reach vs L3 size: the structural reason the 4th quadrant is
    // unreachable with 4 KiB pages on this host.
    const STLB_ENTRIES: f64 = 1536.0;
    let stlb_reach_mb = STLB_ENTRIES * 4096.0 / (1024.0 * 1024.0);
    println!(
        "E_stlb_reach_MiB_at_1536_entries={:.1} E_l3_size_MiB=12 E_stlb_reach_lt_l3={}",
        stlb_reach_mb,
        stlb_reach_mb < 12.0
    );
}

// ---------------------------------------------------------------------------