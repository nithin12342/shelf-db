// crates/sheaf-core/src/morphism/matrix.rs — store restriction weight matrices
// FILE-010. Must never filter strings.
#[derive(Clone, Debug)]
pub struct WeightMatrix {
    pub rows: usize,
    pub cols: usize,
    /// Row-major flat weights.
    pub data: [f32; 8],
}

impl WeightMatrix {
    /// Fixed 2x4 projection used by the Phase-1 benchmark.
    pub fn project_4_to_2() -> Self {
        Self { rows: 2, cols: 4, data: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] }
    }
    pub fn apply(&self, x: &[f32; 4]) -> [f32; 2] {
        let mut y = [0.0f32; 2];
        for (r, out) in y.iter_mut().enumerate() {
            let mut s = 0.0;
            for (c, val) in x.iter().enumerate() {
                s += self.data[r * 4 + c] * val;
            }
            *out = s;
        }
        y
    }
}
