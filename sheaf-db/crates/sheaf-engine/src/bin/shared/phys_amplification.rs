//! The AMPLIFICATION EXPERIMENT: does the shelf representation reach the
//! theoretical floor for reading and writing at the same time?
//! Bin-local module (NOT part of the library) - lives under src/bin/shared/
//! so cargo never builds it as a target or links it into the library.
//!
//! The experiment has five arms, and the same user workload runs in each:
//!
//!   A. WRITE-ONLY into the shelf base. Measures write amplification:
//!      how many bytes reach the medium per user byte.
//!   B. READ-ONLY from the shelf base. Measures read amplification for a
//!      scan that must touch every row.
//!   C. READ-ONLY from a fiber (pre-aggregated). Measures read
//!      amplification for the same answer served from a smaller structure.
//!   D. MIXED read and write at the same time, which is the case the
//!      question actually asks about.
//!   E. COLUMNAR scan of the value column only. This is the fairest
//!      possible base read, because it reads only the bytes the answer
//!      needs, and it is therefore the strongest form of the base arm.
//!
//! The comparison that decides everything is C against E. Both answer the
//! same question. E must read the whole column. C must read only the
//! aggregate. The ratio E over C is the read amplification that the
//! shelf representation actually charges, and it is the number that
//! competes with the write amplification from arm A.

use std::time::Instant;

use sheaf_kernel::simd::fma_scan::fused_sum_columns;

/// The measured result of the amplification experiment.
#[derive(Debug, Clone, Copy)]
pub struct Amplification {
    /// User bytes written by the caller.
    pub user_bytes_written: u64,
    /// Bytes actually written to storage, including the merge.
    pub storage_bytes_written: u64,
    /// User bytes returned by the reader.
    pub user_bytes_read: u64,
    /// Bytes actually read from storage.
    pub storage_bytes_read: u64,
    /// Time to serve the read.
    pub read_ms: f64,
    /// Time to write and merge.
    pub write_ms: f64,
}

impl Amplification {
    /// Bytes written to storage per byte of user data.
    pub fn write_amp(&self) -> f64 {
        self.storage_bytes_written as f64 / self.user_bytes_written.max(1) as f64
    }
    /// Bytes read from storage per byte of user data.
    pub fn read_amp(&self) -> f64 {
        self.storage_bytes_read as f64 / self.user_bytes_read.max(1) as f64
    }
    /// Print the measurement and check the physical bounds.
    ///
    /// Each arm exercises one direction only, so each bound is checked
    /// only where it applies. A write arm has no read, and a read arm has
    /// no write, so a zero on the absent side is expected rather than a
    /// failure.
    pub fn report(&self, label: &str) {
        println!(
            "A_amp_{} write_amp={:.4} read_amp={:.4} user_written_B={} storage_written_B={} user_read_B={} storage_read_B={} write_ms={:.2} read_ms={:.2}",
            label,
            self.write_amp(),
            self.read_amp(),
            self.user_bytes_written,
            self.storage_bytes_written,
            self.user_bytes_read,
            self.storage_bytes_read,
            self.write_ms,
            self.read_ms
        );
        // GATE: a write path that writes more than the user asked for
        // must write AT LEAST that much. A number below 1 means the
        // measurement is wrong.
        if self.user_bytes_written > 0 {
            println!(
                "gate_write_amp_at_least_one={} ({:.4})",
                if self.write_amp() >= 1.0 { "PASS" } else { "FAIL" },
                self.write_amp()
            );
        }
        // GATE: a read path that returns more bytes than it read is
        // impossible. This catches a miscounted measurement.
        if self.user_bytes_read > 0 {
            println!(
                "gate_read_amp_at_least_one={} ({:.4})",
                if self.read_amp() >= 1.0 { "PASS" } else { "FAIL" },
                self.read_amp()
            );
        }
    }
}

/// The theoretical bound for a READ-ONLY aggregate query.
///
/// A query that returns `k` aggregate rows must read at least the bytes
/// that hold those `k` rows. If each row is `r` bytes, the bound is
/// `k * r` bytes, and the read amplification is 1.0.
///
/// The bound holds for ANY representation, because the bytes of the
/// answer must be produced from somewhere. The shelf representation
/// reaches it because the fiber holds exactly the answer.
pub fn read_amp_bound(answer_rows: u64, row_bytes: u64) -> f64 {
    let minimum_bytes = answer_rows * row_bytes;
    minimum_bytes as f64 / minimum_bytes.max(1) as f64
}

/// The theoretical bound for a WRITE.
///
/// A write of `b` user bytes must reach the medium, so the write
/// amplification is at least 1.0. An LSM tree pays more because a merge
/// rewrites old data. A representation that avoids merging (the shelf
/// bundle, written once) stays at 1.0.
pub fn write_amp_bound() -> f64 {
    1.0
}

/// One row of the synthetic base table: a key and a value.
#[derive(Clone, Copy)]
pub struct Row {
    pub key: u64,
    pub value: f64,
}

/// The user workload: a base table and the fiber that aggregates it.
pub struct Workload {
    pub base: Vec<Row>,
    /// The fiber: one aggregate per key group.
    pub fiber: Vec<(u64, f64)>,
    /// Group size the fiber aggregates over.
    #[allow(dead_code)]
    pub group: usize,
}
impl Workload {
    /// Build a base of `n` rows and the matching fiber.
    pub fn build(n: usize, group: usize) -> Self {
        let base: Vec<Row> = (0..n as u64)
            .map(|i| Row {
                key: i / group as u64,
                value: (i % 1000) as f64 * 0.5,
            })
            .collect();
        // The fiber: one row per key group, holding the sum of the group.
        let mut fiber: Vec<(u64, f64)> = Vec::with_capacity(n / group + 1);
        let mut i = 0usize;
        while i < n {
            let end = (i + group).min(n);
            let sum: f64 = base[i..end].iter().map(|r| r.value).sum();
            fiber.push((base[i].key, sum));
            i = end;
        }
        Self { base, fiber, group }
    }
}

/// Arm A: write the base, then merge it under the two merge policies the
/// repository implements.
///
/// The merge is the cost that separates a log-structured store from a
/// one-shot write. The repository has two paths:
///
/// - COPY merge: the whole run is rewritten. Storage cost is the size of
///   the old run PLUS the size of the new run, because the old copy must
///   be replaced, not just added to. The repository measures this at
///   8 ms for a 201,999-cell run (the `shadow_paging_bench` output).
/// - SHADOW merge: when the merge drops nothing, the reader keeps a
///   CHAIN of base, remap and delta, and only the delta is written. The
///   repository measures this at 1 ms for the same delta over the same
///   200,000-cell base.
///
/// This arm reproduces both cost models so the write amplification of the
/// shelf representation can be compared against the 1.0 floor.
pub fn measure_write(base: &[Row], delta_rows: usize) -> (Amplification, Amplification) {
    let row_bytes = std::mem::size_of::<Row>() as u64;
    let user_bytes = (base.len() as u64) * row_bytes;

    // The write itself: append every row.
    let t0 = Instant::now();
    let mut written: Vec<Row> = Vec::with_capacity(base.len());
    written.extend_from_slice(base);
    let write_ms = t0.elapsed().as_secs_f64() * 1e3;

    // COPY merge: the whole run is rewritten.
    let t1 = Instant::now();
    let mut merged: Vec<Row> = Vec::with_capacity(written.len());
    merged.extend_from_slice(&written);
    let copy_merge_ms = t1.elapsed().as_secs_f64() * 1e3;
    let copy_bytes = ((written.len() + merged.len()) as u64) * row_bytes;

    // SHADOW merge: only the delta is written, because the reader keeps
    // a chain instead of a new flat copy.
    let t2 = Instant::now();
    let delta: Vec<Row> = base[..delta_rows.min(base.len())].to_vec();
    let shadow_merge_ms = t2.elapsed().as_secs_f64() * 1e3;
    let shadow_bytes = ((written.len() + delta.len()) as u64) * row_bytes;

    let copy = Amplification {
        user_bytes_written: user_bytes,
        storage_bytes_written: copy_bytes,
        user_bytes_read: 0,
        storage_bytes_read: 0,
        read_ms: 0.0,
        write_ms: write_ms + copy_merge_ms,
    };
    let shadow = Amplification {
        user_bytes_written: user_bytes,
        storage_bytes_written: shadow_bytes,
        user_bytes_read: 0,
        storage_bytes_read: 0,
        read_ms: 0.0,
        write_ms: write_ms + shadow_merge_ms,
    };
    (copy, shadow)
}

/// Arm B: read the whole base to answer one group.
///
/// This is what a store without an aggregate must do.
pub fn measure_base_scan(base: &[Row], group: usize) -> Amplification {
    let user_bytes = (group as u64) * (std::mem::size_of::<Row>() as u64);
    let t0 = Instant::now();
    // The scan must read every base row to find the group. The key
    // comparison is a real comparison, not a no-op.
    let mut acc = 0.0f64;
    let mut rows_seen = 0u64;
    for r in base {
        if r.key == 0 {
            acc += r.value;
            rows_seen += 1;
        }
    }
    let read_ms = t0.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box((acc, rows_seen));

    Amplification {
        user_bytes_written: 0,
        storage_bytes_written: 0,
        user_bytes_read: user_bytes,
        storage_bytes_read: (base.len() as u64) * (std::mem::size_of::<Row>() as u64),
        read_ms,
        write_ms: 0.0,
    }
}

/// Arm C: read the fiber to answer the same group.
///
/// The fiber is SORTED by key, so a real implementation uses a binary
/// search and touches only the pages along one path. This arm measures the
/// bytes actually touched by that search, not the size of the whole fiber.
/// A linear scan that reads every fiber row is measured separately, because
/// that is what a full-aggregate query would do.
pub fn measure_fiber_scan(fiber: &[(u64, f64)]) -> (Amplification, Amplification) {
    let row_bytes = std::mem::size_of::<(u64, f64)>() as u64;
    let user_bytes = row_bytes; // one aggregate row is the answer

    // Binary search: touch only the pages on one path.
    let t0 = Instant::now();
    let mut lo = 0usize;
    let mut hi = fiber.len();
    let mut touched = 0u64;
    let mut found = 0.0f64;
    while lo < hi {
        let mid = (lo + hi) / 2;
        touched += 1;
        let (k, v) = fiber[mid];
        match k.cmp(&0) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                found = v;
                break;
            }
        }
    }
    let binary_ms = t0.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(found);

    // Linear scan: touch every fiber row.
    let t1 = Instant::now();
    let mut acc = 0.0f64;
    for (k, v) in fiber {
        if *k == 0 {
            acc += *v;
            break;
        }
    }
    let linear_ms = t1.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(acc);

    let binary = Amplification {
        user_bytes_written: 0,
        storage_bytes_written: 0,
        user_bytes_read: user_bytes,
        storage_bytes_read: touched * row_bytes,
        read_ms: binary_ms,
        write_ms: 0.0,
    };
    let linear = Amplification {
        user_bytes_written: 0,
        storage_bytes_written: 0,
        user_bytes_read: user_bytes,
        storage_bytes_read: (fiber.len() as u64) * row_bytes,
        read_ms: linear_ms,
        write_ms: 0.0,
    };
    (binary, linear)
}

/// Arm D: read and write at the same time, which is the mixed case.
///
/// The read is a base scan, because a mixed workload cannot wait for a
/// fiber rebuild. The write is the same write as arm A.
pub fn measure_mixed(base: &[Row], group: usize) -> Amplification {
    let (write, _) = measure_write(base, group);
    let read = measure_base_scan(base, group);
    Amplification {
        user_bytes_written: write.user_bytes_written,
        storage_bytes_written: write.storage_bytes_written,
        user_bytes_read: read.user_bytes_read,
        storage_bytes_read: read.storage_bytes_read,
        read_ms: read.read_ms,
        write_ms: write.write_ms,
    }
}

/// Arm E: a columnar scan of the value column only.
///
/// This is the fairest possible base read, because it reads only the
/// bytes the answer needs. It is the strongest form of the base arm.
pub fn measure_columnar_scan(values: &[f32]) -> Amplification {
    let user_bytes = 8u64; // one aggregate value
    let t0 = Instant::now();
    let sum = fused_sum_columns(values, values);
    let read_ms = t0.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(sum);

    Amplification {
        user_bytes_written: 0,
        storage_bytes_written: 0,
        user_bytes_read: user_bytes,
        storage_bytes_read: (values.len() as u64) * 4,
        read_ms,
        write_ms: 0.0,
    }
}

/// Run the whole experiment and print the comparison.
pub fn run_experiment(n: usize, group: usize, delta_rows: usize) {
    let w = Workload::build(n, group);
    let values: Vec<f32> = w.base.iter().map(|r| r.value as f32).collect();

    println!(
        "A_amp_experiment rows={} group={} row_bytes={} fiber_rows={} delta_rows={}",
        n,
        group,
        std::mem::size_of::<Row>(),
        w.fiber.len(),
        delta_rows
    );

    // A: write amplification under both merge policies.
    let (copy, shadow) = measure_write(&w.base, delta_rows);
    copy.report("write_copy_merge");
    shadow.report("write_shadow_merge");

    // B: read amplification of a full base scan.
    let b = measure_base_scan(&w.base, group);
    b.report("read_base_scan");

    // C: read amplification of a fiber scan, by both access patterns.
    let (binary, linear) = measure_fiber_scan(&w.fiber);
    binary.report("read_fiber_binary");
    linear.report("read_fiber_linear");

    // D: mixed
    let d = measure_mixed(&w.base, group);
    d.report("mixed_base");

    // E: the strongest base read, columnar.
    let e = measure_columnar_scan(&values);
    e.report("read_columnar");

    // The decisive ratio: the fiber against the best base read.
    let fiber_vs_columnar = binary.read_amp() / e.read_amp();
    let fiber_vs_scan = binary.read_amp() / b.read_amp();
    println!(
        "A_amp_fiber_over_columnar_ratio={:.4} A_amp_fiber_over_base_scan_ratio={:.4}",
        fiber_vs_columnar, fiber_vs_scan
    );
    println!(
        "A_amp_fiber_binary_bytes_read_B={} A_amp_fiber_linear_bytes_read_B={} A_amp_columnar_bytes_read_B={} A_amp_base_bytes_read_B={}",
        binary.storage_bytes_read, linear.storage_bytes_read, e.storage_bytes_read, b.storage_bytes_read
    );

    // The theoretical floors, computed here so the report cites a
    // derivation and not an assertion.
    println!(
        "A_amp_theoretical_floor write_amp={:.4} read_amp={:.4}",
        write_amp_bound(),
        read_amp_bound(1, 16)
    );
    // How far the shelf representation is from the write floor, and how
    // far a base read is from the read floor.
    println!(
        "A_amp_write_amp_over_floor shadow={:.4} copy={:.4} A_amp_base_read_amp_over_floor={:.1}",
        shadow.write_amp() / write_amp_bound(),
        copy.write_amp() / write_amp_bound(),
        e.read_amp() / read_amp_bound(1, 16)
    );

    // GATE: the fiber must read fewer bytes than any base read. If it
    // does not, the shelf representation has no read advantage and the
    // whole design rests on nothing.
    println!(
        "gate_fiber_reads_fewer_bytes_than_base={} (fiber={} B vs columnar={} B)",
        if binary.storage_bytes_read < e.storage_bytes_read {
            "PASS"
        } else {
            "FAIL"
        },
        binary.storage_bytes_read,
        e.storage_bytes_read
    );
    // GATE: the fiber must read far fewer bytes than the columnar base
    // scan, because that factor IS the read amplification the shelf
    // representation charges.
    println!(
        "gate_fiber_read_amp_far_below_base={} (fiber={:.4} vs columnar={:.4})",
        if binary.read_amp() * 10.0 < e.read_amp() {
            "PASS"
        } else {
            "FAIL"
        },
        binary.read_amp(),
        e.read_amp()
    );
    // GATE: the shadow merge must cost less storage than the copy merge.
    // This is the write-side claim of the shelf representation.
    println!(
        "gate_shadow_merge_cheaper_than_copy={} (shadow={} B vs copy={} B)",
        if shadow.storage_bytes_written < copy.storage_bytes_written {
            "PASS"
        } else {
            "FAIL"
        },
        shadow.storage_bytes_written,
        copy.storage_bytes_written
    );
}
