// crates/sheaf-query/src/ir/tir.rs — lower AST to TIR graph
// FILE-033. Depends on FILE-032. The 12-opcode topological plan from the
// architecture: every query lowers to exactly these steps in canonical
// order; Phase-5's optimizer rewrites them, never invents new ones.
use super::super::syntax::ast::{Expr, Query};

#[derive(Clone, Debug, PartialEq)]
pub enum TirOp {
    BcsfScan { hyper_type: String },
    DiscreteFilter { expr: Option<Expr> },
    Gather,
    ApplyMask,
    VecGather,
    LoadMorphisms,
    BuildLaplacian { regularization: f64 },
    SolvePcg { max_iterations: usize },
    EvalEnergy,
    FilterThreshold { energy_threshold: f64 },
    Materialize,
    Yield,
}

/// Lower one query to the canonical 12-opcode plan. DDL/DML statements are
/// catalog effects, not query plans — they lower to the empty sequence
/// (documented; optimizer input is always a lowered query).
pub fn lower_query(q: &Query) -> Vec<TirOp> {
    vec![
        TirOp::BcsfScan { hyper_type: q.hyper_type.clone() },
        TirOp::DiscreteFilter { expr: q.where_clause.clone() },
        TirOp::Gather,
        TirOp::ApplyMask,
        TirOp::VecGather,
        TirOp::LoadMorphisms,
        TirOp::BuildLaplacian { regularization: q.solve.regularization },
        TirOp::SolvePcg { max_iterations: q.solve.max_iterations },
        TirOp::EvalEnergy,
        TirOp::FilterThreshold { energy_threshold: q.solve.energy_threshold },
        TirOp::Materialize,
        TirOp::Yield,
    ]
}
