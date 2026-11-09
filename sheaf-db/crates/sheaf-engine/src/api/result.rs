// crates/sheaf-engine/src/api/result.rs — serialize section plus energy
// FILE-047. Depends on nothing (plain data + rendering; the kernel owns the
// math, TQL owns the syntax). Canonical .section text format per SPEC-020:
// fixed field order so golden outputs compare byte-for-byte. Cohomology is
// HARMONIC_BOUNDED iff energy < threshold, else UNBOUNDED (documented,
// minimal two-state certificate for Phase 4).
#[derive(Clone, Debug, PartialEq)]
pub enum CohomologyState {
    HarmonicBounded,
    Unbounded,
}

impl CohomologyState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CohomologyState::HarmonicBounded => "HARMONIC_BOUNDED",
            CohomologyState::Unbounded => "UNBOUNDED",
        }
    }

    pub fn certify(energy: f32, threshold: f64) -> Self {
        if (energy as f64) < threshold {
            CohomologyState::HarmonicBounded
        } else {
            CohomologyState::Unbounded
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SectionResult {
    pub status: String,
    pub execution_time_us: u64,
    pub dimension: u8,
    pub cells: Vec<(String, u8)>,
    pub discrete_payload: Vec<(String, String)>,
    pub dirichlet_energy: f32,
    pub iterations_to_converge: usize,
    pub cohomology_state: CohomologyState,
    pub stalk_projections: Vec<(String, Vec<f32>)>,
}

impl SectionResult {
    fn fmt_f32(v: &f32) -> String {
        // Shortest round-trip float rendering (stable across runs).
        let mut s = format!("{:.6}", v);
        while s.contains('.') && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.push('0');
        }
        s
    }

    /// Canonical rendering. Field order is part of the contract.
    pub fn render(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!("status: {}\n", self.status));
        o.push_str(&format!("execution_time_us: {}\n", self.execution_time_us));
        o.push_str("subcomplex:\n");
        o.push_str(&format!("  dimension: {}\n", self.dimension));
        o.push_str(&format!("  cell_count: {}\n", self.cells.len()));
        o.push_str("  cells:\n");
        for (id, dim) in &self.cells {
            o.push_str(&format!("    - {{ id: \"{}\", dim: {} }}\n", id, dim));
        }
        o.push_str("discrete_payload:\n");
        for (k, v) in &self.discrete_payload {
            o.push_str(&format!("  {}: \"{}\"\n", k, v));
        }
        o.push_str("spectral_metrics:\n");
        o.push_str(&format!("  dirichlet_energy: {}\n", Self::fmt_f32(&self.dirichlet_energy)));
        o.push_str(&format!("  iterations_to_converge: {}\n", self.iterations_to_converge));
        o.push_str(&format!("  cohomology_state: {}\n", self.cohomology_state.as_str()));
        o.push_str("harmonic_section:\n");
        o.push_str("  stalk_projections:\n");
        for (cell, vec) in &self.stalk_projections {
            let vs: Vec<String> = vec.iter().map(Self::fmt_f32).collect();
            o.push_str(&format!("    - {{ cell: \"{}\", vector: [{}] }}\n", cell, vs.join(", ")));
        }
        o
    }
}
