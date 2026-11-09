// crates/sheaf-core/src/stalk/continuous.rs — store aligned continuous embedding vectors
// FILE-006. Must never do string matching.
#[derive(Clone, Debug)]
pub struct ContinuousStalk {
    pub dim: u8,
    pub values: [f32; 4],
}

impl ContinuousStalk {
    pub fn new(values: [f32; 4]) -> Self {
        Self { dim: 4, values }
    }
}
