// crates/sheaf-core/src/morphism/compound.rs — combine pi plus W morphism
// FILE-011. Must never own storage layout.
use super::matrix::WeightMatrix;

#[derive(Clone, Debug)]
pub struct CompoundMorphism {
    pub mask: [u8; 16],
    pub matrix: WeightMatrix,
}

impl CompoundMorphism {
    pub fn identity_like() -> Self {
        Self { mask: [0xFF; 16], matrix: WeightMatrix::project_4_to_2() }
    }
}
