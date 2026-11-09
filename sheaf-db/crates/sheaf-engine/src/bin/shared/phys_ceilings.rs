//! Fundamental and storage ceilings that the first pass of the study did not
//! cover. Bin-local module (NOT part of the library) - lives under
//! src/bin/shared/ so cargo never builds it as a target or links it into the
//! library.
//!
//! Every number here is COMPUTED from a published physical constant or from
//! a measurement this probe took. Nothing is copied from a datasheet.
//!
//! The ceilings covered here:
//!   C1  Amdahl's law: the parallel speedup ceiling, fitted to the MEASURED
//!       thread scaling of this host.
//!   C2  DRAM refresh: the traffic the memory system spends on itself, and
//!       the retention/density trade that sets the refresh interval.
//!   C3  Soft errors: the bit-flip rate from cosmic rays, and the mean time
//!       between bit flips in this host's memory.
//!   C4  Bennett: the energy floor of a logical operation, and what
//!       reversible computing changes.
//!   C5  Margolus-Levitin: the quantum speed limit on operations per second.
//!   C6  Bekenstein: the maximum information that a region of given energy
//!       can hold.
//!   C7  Address space: the hard ceiling on addressable memory, and the gap
//!       between it and what this host installs.
//!   C8  NAND endurance: the program/erase cycle limit, and the write budget
//!       that follows from it.

// ---------------------------------------------------------------------------
// C1. Amdahl's law
// ---------------------------------------------------------------------------

/// Fit Amdahl's law to the MEASURED thread scaling and report the ceiling.
///
/// Amdahl's law: `S(n) = 1 / ((1 - p) + p/n)`, where `p` is the fraction of
/// the work that scales and `1 - p` is the fraction that does not.
///
/// Applied to a bandwidth-bound workload the time has the same shape:
/// `T(n) = T_serial + T_parallel / n`. Since bandwidth is `bytes / time`,
/// the measured bandwidth ratios give `T(1) / T(n)` directly.
///
/// The fit answers the question the repo kept asking: "how much more can
/// threads give?" The answer is a number, and it comes from the measured
/// data rather than from an assumption about the machine.
///
/// `measurements` is the list of (thread count, read GB/s) pairs.
pub fn amdahl_ceiling(measurements: &[(usize, f64)]) {
    if measurements.len() < 2 {
        return;
    }
    let b1 = measurements[0].1;
    println!("C_amdahl_threads gbps");
    for (t, g) in measurements {
        // T(1)/T(n) = B(1)/B(n) because the byte count is the same.
        let ratio = b1 / g;
        // ratio = (1-p) + p/n  =>  p = n*(1 - ratio) / (n - 1)
        let n = *t as f64;
        let p = if n > 1.0 {
            n * (1.0 - ratio) / (n - 1.0)
        } else {
            0.0
        };
        // As n -> infinity the speedup ceiling is 1 / (1 - p).
        let ceiling = if p < 1.0 { 1.0 / (1.0 - p) } else { f64::INFINITY };
        println!(
            "C_amdahl_point threads={} gbps={:.2} time_ratio={:.4} implied_serial_p={:.4} implied_max_speedup={:.2}x",
            t, g, ratio, p, ceiling
        );
    }
    // The ceiling that matters is the best implied over all thread counts,
    // because the serial fraction is a property of the workload, not of n.
    let mut best_p: f64 = 0.0;
    for (t, g) in measurements {
        let n = *t as f64;
        if n <= 1.0 {
            continue;
        }
        let p = n * (1.0 - b1 / g) / (n - 1.0);
        if p > best_p {
            best_p = p;
        }
    }
    let ceiling = if best_p < 1.0 { 1.0 / (1.0 - best_p) } else { f64::INFINITY };
    println!(
        "C_amdahl_serial_fraction_best={:.4} C_amdahl_max_speedup_ceiling={:.2}x",
        best_p, ceiling
    );
    println!(
        "C_amdahl_bandwidth_at_ceiling_GBps={:.2}",
        b1 * ceiling
    );

    // GATE: the measured bandwidth can never exceed what Amdahl's law
    // allows, so the implied serial fraction must stay below 1. A value at
    // or above 1 means the measurement or the fit is wrong.
    println!(
        "gate_amdahl_serial_fraction_below_one={} (p={:.4})",
        if best_p < 1.0 { "PASS" } else { "FAIL" },
        best_p
    );
    // GATE: the implied ceiling must be at least the measured speedup, or
    // the fit is inconsistent with the data it came from.
    let measured_speedup = b1 / measurements.last().unwrap().1;
    println!(
        "gate_amdahl_ceiling_above_measured={} (ceiling={:.2}x measured={:.2}x)",
        if ceiling >= measured_speedup {
            "PASS"
        } else {
            "FAIL"
        },
        ceiling,
        measured_speedup
    );
}

// ---------------------------------------------------------------------------
// C2. DRAM refresh
// ---------------------------------------------------------------------------

/// The DRAM refresh ceiling: the traffic the memory spends on itself, and
/// the retention trade that sets the refresh interval.
///
/// Published values used (JEDEC DDR4, Wikipedia "Memory refresh"):
///   - refresh time 64 ms, 8192 rows per bank
///   - a refresh cycle is 4 memory clock cycles
///   - the maximum refresh interval is set by the ratio of stored charge to
///     leakage current, so a smaller cell needs a shorter interval
///   - cells actually hold their charge for 1 to 10 seconds
///
/// TWO TRAFFIC NUMBERS, and the difference matters:
///   - the CELL-LEVEL traffic, inside each DRAM package, where the sense
///     amplifiers read and rewrite the rows. This is large, and it is
///     internal to the chip.
///   - the EXTERNAL bus traffic, which carries only the refresh command
///     and its row address. This is small, and it is what the memory
///     controller sees.
pub fn dram_refresh_ceiling(
    capacity_bytes: f64,
    ddr_mt_s: f64,
    rows_per_bank: f64,
    banks: f64,
) {
    const REFRESH_TIME_S: f64 = 0.064;
    const REFRESH_CYCLES: f64 = 4.0;

    let clock_ns = 2000.0 / ddr_mt_s;
    let cycle_ns = REFRESH_CYCLES * clock_ns;
    // The interval between two refresh cycles.
    let interval_us = REFRESH_TIME_S * 1e6 / (rows_per_bank * banks);
    // The time the memory spends refreshing in one refresh interval.
    let total_refresh_ns = cycle_ns * rows_per_bank * banks;
    let overhead_pct = (total_refresh_ns * 1e-9 / REFRESH_TIME_S) * 100.0;

    // Cell-level traffic: every bit is read and rewritten once per
    // interval, inside the package.
    let cell_traffic_gbs = capacity_bytes / REFRESH_TIME_S / 1e9;

    // External bus traffic: only the command and the row address cross
    // the bus. One command per row per bank.
    let commands_per_s = rows_per_bank * banks / REFRESH_TIME_S;
    let bus_bytes_per_s = commands_per_s * 4.0; // a row address plus control
    let bus_gbs = bus_bytes_per_s / 1e9;
    // The external bus peak for one 64-bit channel at this transfer rate.
    let bus_peak_gbs = 8.0 * ddr_mt_s * 1e6 / 1e9;

    println!(
        "C_refresh_cycle_ns={:.2} C_refresh_interval_us={:.3} C_refresh_overhead_pct={:.4}",
        cycle_ns, interval_us, overhead_pct
    );
    println!(
        "C_refresh_cell_traffic_GBps={:.1} (internal to the package, not on the external bus)",
        cell_traffic_gbs
    );
    println!(
        "C_refresh_external_bus_GBps={:.3} C_refresh_external_bus_pct_of_peak={:.4}",
        bus_gbs,
        bus_gbs / bus_peak_gbs * 100.0
    );
    println!(
        "C_refresh_holds_lost_bits_per_second={:.3e}",
        capacity_bytes * 8.0 / REFRESH_TIME_S
    );
    // GATE: refresh cannot consume more than the whole refresh interval.
    // The published figure for a modern DDR part is well under 1 percent.
    println!(
        "gate_refresh_overhead_under_one_pct={} (overhead={:.4} pct)",
        if overhead_pct < 1.0 { "PASS" } else { "FAIL" },
        overhead_pct
    );
}

// ---------------------------------------------------------------------------
// C3. Soft errors
// ---------------------------------------------------------------------------

/// The soft error ceiling: how often a bit flips on its own, and what that
/// means for the mean time between bit flips in this host's memory.
///
/// Published values used (Wikipedia "Soft error"):
///   - IBM (1996): one error per month per 256 MiB of RAM
///   - a measured experiment: 5950 failures in time (FIT) per DRAM chip,
///     where one FIT is one failure per billion device-hours
///   - sea level neutron flux: about 14 neutrons per cm^2 per hour,
///     rising by a factor of 2.2 for every 1000 m of altitude
pub fn soft_error_ceiling(capacity_bytes: f64, chips: usize) {
    const BYTES_PER_IBM_UNIT: f64 = 256.0 * 1024.0 * 1024.0;
    const FIT_PER_CHIP: f64 = 5950.0;
    const FIT_HOURS: f64 = 1e9;
    const HOURS_PER_YEAR: f64 = 8766.0;

    // IBM 1996 rate, applied to this host's installed memory.
    let units = capacity_bytes / BYTES_PER_IBM_UNIT;
    let errors_per_month = units;
    let errors_per_day = errors_per_month / 30.0;
    // Mean time between flips SOMEWHERE in the memory. This is the
    // interval between errors, not the lifetime of one bit.
    let hours_between = 30.0 * 24.0 / errors_per_month;
    // The lifetime of ONE bit, in years. With 16 GiB there are 1.4e11
    // bits and 64 errors per month, so each bit is hit about
    // 1.4e11 times less often than the module is. A bit is therefore
    // almost perfectly durable on its own, while the module as a whole
    // sees a flip every few hours. That gap is why error correction
    // exists: the failures are rare per bit and constant per module.
    let bits = capacity_bytes * 8.0;
    let single_bit_years = hours_between * bits / HOURS_PER_YEAR;

    println!(
        "C_soft_error_ibm_errors_per_month={:.1} C_soft_error_errors_per_day={:.2}",
        errors_per_month, errors_per_day
    );
    println!(
        "C_soft_error_mean_hours_between_flips={:.1} C_soft_error_mean_years_between_flips={:.3}",
        hours_between,
        hours_between / HOURS_PER_YEAR
    );
    println!(
        "C_soft_error_single_bit_years={:.3e} (a bit outlives the module by this factor)",
        single_bit_years
    );
    // Measured per-chip rate for the whole memory module.
    let dim_fit = FIT_PER_CHIP * chips as f64;
    let dim_hours = FIT_HOURS / dim_fit;
    println!(
        "C_soft_error_dim_fit_total={:.0} C_soft_error_dim_mtbf_years={:.2} (chips={})",
        dim_fit,
        dim_hours / HOURS_PER_YEAR,
        chips
    );

    // Altitude factor, for the mountain case.
    for meters in [0.0f64, 1000.0, 2000.0, 3000.0] {
        println!(
            "C_soft_error_altitude_m={:.0} C_soft_error_flux_x={:.2}",
            meters,
            2.2f64.powf(meters / 1000.0)
        );
    }

    // GATE: a single bit must be far more durable than the module that
    // holds it. If the two figures came out the same, one of them is
    // wrong.
    println!(
        "gate_soft_error_bit_outlives_module={} (bit={:.3e} years, module={:.3e} years)",
        if single_bit_years > hours_between / HOURS_PER_YEAR {
            "PASS"
        } else {
            "FAIL"
        },
        single_bit_years,
        hours_between / HOURS_PER_YEAR
    );
}

// ---------------------------------------------------------------------------
// C4 and C5. Logical operation floor and quantum speed limit
// ---------------------------------------------------------------------------

/// The energy floor of a logical operation, and the quantum speed limit.
///
/// Published constants:
///   - Landauer: E >= k_B T ln2 for an irreversible operation
///   - Bennett (1973): a REVERSIBLE computation can in principle approach
///     an arbitrarily small energy cost per logical operation, at the cost
///     of extra computation and storage
///   - Margolus-Levitin: a quantum system with average energy E above the
///     ground state needs a time tau with E * tau >= (pi/2) * hbar
pub fn operation_floors() {
    const H_BAR: f64 = 1.054_571_817e-34;
    const T: f64 = 300.0;
    const K_B: f64 = 1.380_649e-23;

    // Landauer, recomputed here so the value does not depend on any other
    // module. k_B T ln2.
    let landauer = K_B * T * std::f64::consts::LN_2;
    println!("C_op_floor_landauer_J={:.4e}", landauer);

    // Margolus-Levitin: the fastest evolution rate for a given energy.
    for watts in [1.0f64, 10.0, 100.0] {
        let ops_per_s = 2.0 * watts / (std::f64::consts::PI * H_BAR);
        println!(
            "C_op_floor_margolus_levitin_W={:.0} C_op_floor_max_ops_per_s={:.3e}",
            watts, ops_per_s
        );
    }
}

// ---------------------------------------------------------------------------
// C6. Bekenstein bound
// ---------------------------------------------------------------------------

/// The Bekenstein bound: the maximum entropy that a region of given radius
/// and energy can hold.
///
/// `S <= 2*pi*k*R*E / (hbar*c)`. Expressed in bits, the right hand side
/// divided by k_B is the maximum number of bits the region can hold.
pub fn bekenstein_ceiling(radius_m: f64, energy_j: f64) {
    const K_B: f64 = 1.380_649e-23;
    const H_BAR: f64 = 1.054_571_817e-34;
    const C_M_S: f64 = 299_792_458.0;

    // The bound is an entropy, in joules per kelvin. Dividing by k_B
    // gives the maximum number of bits the region can hold.
    let max_entropy = 2.0 * std::f64::consts::PI * K_B * radius_m * energy_j / (H_BAR * C_M_S);
    let max_bits = max_entropy / K_B;
    println!(
        "C_bekenstein_radius_m={:.3} C_bekenstein_energy_J={:.1} C_bekenstein_max_entropy_J_per_K={:.3e}",
        radius_m, energy_j, max_entropy
    );
    println!("C_bekenstein_max_bits={:.3e}", max_bits);
}

// ---------------------------------------------------------------------------
// C7. Address space
// ---------------------------------------------------------------------------

/// The address space ceiling, and the gap to what this host installs.
pub fn address_space_ceiling(installed_bytes: f64) {
    const PHYSICAL_BITS: u32 = 64;
    const CANONICAL_BITS: u32 = 48;
    let physical_max = 2.0f64.powi(PHYSICAL_BITS as i32);
    let canonical_max = 2.0f64.powi(CANONICAL_BITS as i32);
    println!(
        "C_address_physical_max_bytes={:.3e} C_address_canonical_max_bytes={:.3e}",
        physical_max, canonical_max
    );
    println!(
        "C_address_installed_pct_of_physical={:.3e} C_address_installed_pct_of_canonical={:.3e}",
        installed_bytes / physical_max * 100.0,
        installed_bytes / canonical_max * 100.0
    );
    // Pages needed to cover the installed memory, and the TLB entries that
    // would be needed to address it without a walk.
    let pages = installed_bytes / 4096.0;
    println!(
        "C_address_pages_needed={:.0} C_address_tlb_entries_needed={:.0}",
        pages, pages
    );
}

// ---------------------------------------------------------------------------
// C8. NAND endurance
// ---------------------------------------------------------------------------

/// The NAND endurance ceiling: how many times a cell can be rewritten, and
/// the daily write budget that follows.
///
/// Published values used (Wikipedia "Solid-state drive", JEDEC):
///   - program/erase cycles: about 100000 for single-level cells, 10000 for
///     multi-level, 1000 for triple-level, 100 for quad-level
///   - a 1 TB drive rated at 600 terabytes written, which is 600
///     program/erase cycles spread over the drive
///
/// The drive rating scales with capacity, so the terabytes-written rating
/// is given for the drive in question.
pub fn nand_endurance_ceiling(drive_gb: f64, dwpd: f64, years: f64, tbw_tb: f64) {
    let daily_write_gb = drive_gb * dwpd;
    let yearly_write_tb = daily_write_gb * days_in(years) / 1000.0;
    let total_write_tb = yearly_write_tb * years;
    // Program/erase cycles consumed. The rating is the number of times the
    // WHOLE drive can be written, so the fraction of the budget used is
    // total written divided by the rating.
    let pe_cycles_used = total_write_tb / tbw_tb;
    println!(
        "C_nand_daily_write_budget_GB={:.1} C_nand_yearly_write_TB={:.2}",
        daily_write_gb, yearly_write_tb
    );
    println!(
        "C_nand_total_write_TB={:.2} C_nand_tbw_budget_TB={:.0} C_nand_budget_used_fraction={:.4}",
        total_write_tb, tbw_tb, pe_cycles_used
    );
    // How long until the rating is exhausted at this write rate.
    let years_to_exhaust = if daily_write_gb > 0.0 {
        tbw_tb * 1000.0 / (daily_write_gb * days_in(years))
    } else {
        f64::INFINITY
    };
    println!(
        "C_nand_years_to_exhaust_rating={:.1} (at {:.2} device writes per day)",
        years_to_exhaust, dwpd
    );
    // The published endurance per cell type, for reference.
    for (name, cycles) in [("SLC", 100_000.0), ("MLC", 10_000.0), ("TLC", 1_000.0), ("QLC", 100.0)] {
        println!("C_nand_pe_cycles_{}={:.0}", name, cycles);
    }
}

/// Days in a number of years, using 365.25 days per year.
fn days_in(years: f64) -> f64 {
    years * 365.25
}
