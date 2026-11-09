// crates/sheaf-core/src/stalk/bundle.rs — unify discrete plus continuous handle
// FILE-008. Must never persist or compact.
use super::continuous::ContinuousStalk;
use super::discrete::DiscreteStalk;

#[derive(Clone, Debug)]
pub struct StalkBundle {
    pub discrete: DiscreteStalk,
    pub continuous: ContinuousStalk,
}
