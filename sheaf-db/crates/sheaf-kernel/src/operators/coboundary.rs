// crates/sheaf-kernel/src/operators/coboundary.rs — compute delta residuals per cell
// FILE-024. Depends on FILE-022 (ordering not needed here) + FILE-023.
// One hypercell, constant restriction map M (d×d row-major, cloned from the
// cell's stored morph slice at build time): δ = Σᵢ sᵢ·M·x[vᵢ], signs sᵢ are
// canonical parity (−1)^position over the sorted boundary (Gap-1 rule).
// Discrete agreement (ker π) is checked separately via discrete_agrees:
// a mismatched class means "no consistent section", not a number.
use crate::simd::matvec::matvec;

pub const MAX_D: usize = 8;

#[derive(Clone, Debug)]
pub struct CoboundaryOp {
    pub faces: Vec<u32>,
    pub signs: Vec<f32>,
    pub morph: Vec<f32>,
    pub d: usize,
}

impl CoboundaryOp {
    pub fn new(faces: Vec<u32>, signs: Vec<f32>, morph: Vec<f32>, d: usize) -> Self {
        assert_eq!(faces.len(), signs.len(), "faces/signs length mismatch");
        assert_eq!(morph.len(), d * d, "morph must be d×d");
        assert!(matches!(d, 1..=MAX_D), "stalk dim out of range");
        assert!(signs.iter().all(|s| *s == 1.0 || *s == -1.0), "signs must be ±1");
        Self { faces, signs, morph, d }
    }

    /// Adds δ into out (length d). Uses stack buffers, zero heap per call.
    pub fn residual_into(&self, stalks: &[f32], out: &mut [f32]) {
        debug_assert_eq!(out.len(), self.d);
        let mut proj = [0.0f32; MAX_D];
        let mut acc = [0.0f32; MAX_D];
        for (face, sign) in self.faces.iter().zip(self.signs.iter()) {
            let base = (*face as usize) * self.d;
            matvec(&self.morph, &stalks[base..base + self.d], self.d, &mut proj[..self.d]);
            for (slot, val) in acc[..self.d].iter_mut().zip(proj[..self.d].iter()) {
                *slot += sign * val;
            }
        }
        for c in 0..self.d {
            out[c] += acc[c];
        }
    }

    /// ||δ||² for one cell.
    pub fn cell_energy(&self, stalks: &[f32]) -> f32 {
        let mut delta = [0.0f32; MAX_D];
        self.residual_into(stalks, &mut delta[..self.d]);
        delta[..self.d].iter().map(|v| v * v).sum()
    }
}

/// Discrete stalk agreement across one hypercell's faces (ker π probe).
/// Classes are the u64 ids carried in each face's discrete payload.
pub fn discrete_agrees(classes: &[u64]) -> bool {
    classes.windows(2).all(|w| w[0] == w[1])
}
