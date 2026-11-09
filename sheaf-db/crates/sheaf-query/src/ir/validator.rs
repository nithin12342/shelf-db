// crates/sheaf-query/src/ir/validator.rs — reject dim cycles mismatches
// FILE-034. Depends on FILE-033 (lowered shapes) + FILE-004 (poset order).
// Eulerian mapping (Gap-1 fix): parity signs are well-defined exactly when
// every boundary step is codimension-1. The two-level storage scope admits
// only dim-0 boundary members, so codim-1 holds by construction for direct
// faces and by sorted-subset virtual faces otherwise — anything else is
// rejected here, which IS the Eulerian δ²=0 gate at query level.
use std::collections::{HashMap, HashSet};
use super::super::syntax::ast::{Expr, Program, Stmt};
use sheaf_core::poset::ordering::can_be_face;

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

fn expr_fields(e: &Expr, bound: &HashMap<String, Vec<String>>, errs: &mut Vec<ValidationError>) {
    match e {
        Expr::And(a, b) | Expr::Or(a, b) => {
            expr_fields(a, bound, errs);
            expr_fields(b, bound, errs);
        }
        Expr::Not(x) => expr_fields(x, bound, errs),
        Expr::Cmp { path, .. } => {
            if path.len() < 2 {
                errs.push(ValidationError { line: 1, col: 1, msg: format!("path {:?} needs var.field form", path) });
                return;
            }
            match bound.get(&path[0]) {
                Some(fields) if fields.contains(&path[1]) => {}
                Some(_) => errs.push(ValidationError {
                    line: 1,
                    col: 1,
                    msg: format!("unknown field '{}' on '{}'", path[1], path[0]),
                }),
                None => errs.push(ValidationError {
                    line: 1,
                    col: 1,
                    msg: format!("unbound variable '{}'", path[0]),
                }),
            }
        }
    }
}

/// Validate a whole program. Returns all errors (empty = valid).
pub fn validate_program(p: &Program) -> Vec<ValidationError> {
    let mut errs: Vec<ValidationError> = Vec::new();
    let mut cell_dims: HashMap<String, u8> = HashMap::new();
    let mut cell_cont: HashMap<String, usize> = HashMap::new();
    let mut cell_fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut morphs: HashMap<String, (String, String, usize)> = HashMap::new();
    let mut seen_types: HashSet<String> = HashSet::new();
    let mut seen_morphs: HashSet<String> = HashSet::new();

    for stmt in &p.stmts {
        match stmt {
            Stmt::DefineCell(d) => {
                if !seen_types.insert(d.name.clone()) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("duplicate cell type '{}'", d.name) });
                }
                cell_dims.insert(d.name.clone(), d.dim);
                cell_cont.insert(d.name.clone(), d.cont_dim);
                cell_fields.insert(d.name.clone(), d.discrete_fields.clone());
            }
            Stmt::DefineMorphism(m) => {
                if !seen_morphs.insert(m.name.clone()) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("duplicate morphism '{}'", m.name) });
                }
                if !seen_types.contains(&m.from_type) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("unknown source type '{}'", m.from_type) });
                }
                if !seen_types.contains(&m.to_type) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("unknown target type '{}'", m.to_type) });
                }
                let w = m.matrix.first().map(|r| r.len()).unwrap_or(0);
                if m.matrix.is_empty() || m.matrix.iter().any(|r| r.len() != w) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("morphism '{}' matrix must be non-empty rectangular", m.name) });
                } else {
                    morphs.insert(m.name.clone(), (m.from_type.clone(), m.to_type.clone(), w));
                    // Square maps sized to the source stalk dim (kernel model).
                    if let (Some(&sd), Some(&td)) = (cell_cont.get(&m.from_type), cell_cont.get(&m.to_type)) {
                            if w != sd {
                                errs.push(ValidationError {
                                    line: 1,
                                    col: 1,
                                    msg: format!("morphism '{}' width {} != source '{}' cont dim {}", m.name, w, m.from_type, sd),
                                });
                            }
                            if td != sd {
                                errs.push(ValidationError {
                                    line: 1,
                                    col: 1,
                                    msg: format!("target '{}' cont dim {} != source dim {} (one d per query)", m.to_type, td, sd),
                                });
                            }
                    }
                }
            }
            Stmt::InsertCell(c) => {
                match cell_cont.get(&c.type_name) {
                    Some(&d) if c.stalk.len() != d => errs.push(ValidationError {
                        line: 1,
                        col: 1,
                        msg: format!("stalk len {} != '{}' cont dim {}", c.stalk.len(), c.type_name, d),
                    }),
                    None => errs.push(ValidationError { line: 1, col: 1, msg: format!("unknown cell type '{}'", c.type_name) }),
                    _ => {}
                }
                match cell_dims.get(&c.type_name) {
                    Some(&0) => {}
                    Some(&d) => errs.push(ValidationError {
                        line: 1,
                        col: 1,
                        msg: format!("INSERT CELL '{}' has dim {} (must be 0)", c.type_name, d),
                    }),
                    None => {}
                }
            }
            Stmt::InsertHyper(h) => {
                match cell_dims.get(&h.type_name) {
                    Some(&d) if d >= 1 => {}
                    Some(&d) => errs.push(ValidationError {
                        line: 1,
                        col: 1,
                        msg: format!("INSERT HYPERCELL '{}' has dim {} (must be >=1)", h.type_name, d),
                    }),
                    None => errs.push(ValidationError { line: 1, col: 1, msg: format!("unknown hypercell type '{}'", h.type_name) }),
                }
                for r in &h.boundary {
                    match cell_dims.get(&r.type_name) {
                        // Two-level scope: hypercell boundaries are dim-0
                        // cells, so every step is codim-1 and parity signs
                        // (hence δ²=0) are well-defined. Anything else is
                        // rejected here — the Eulerian gate at query level.
                        Some(&0) => {}
                        Some(&d) => errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("boundary '{}' has dim {} (must be 0 for codim-1 parity)", r.type_name, d),
                        }),
                        None => errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("unknown boundary type '{}'", r.type_name),
                        }),
                    }
                }
                match cell_cont.get(&h.type_name) {
                    Some(&d) if h.stalk.len() != d => errs.push(ValidationError {
                        line: 1,
                        col: 1,
                        msg: format!("hypercell stalk len {} != cont dim {}", h.stalk.len(), d),
                    }),
                    _ => {}
                }
            }
            Stmt::Query(q) => {
                if !seen_types.contains(&q.hyper_type) {
                    errs.push(ValidationError { line: 1, col: 1, msg: format!("unknown hypercell type '{}'", q.hyper_type) });
                }
                if let Some(&hd) = cell_dims.get(&q.hyper_type) {
                    if hd < 1 {
                        errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("MATCH target '{}' has dim {} (must be >=1)", q.hyper_type, hd),
                        });
                    }
                }
                let mut bound: HashMap<String, Vec<String>> = HashMap::new();
                let mut seen_vars: HashSet<String> = HashSet::new();
                // The hypercell var itself is bound to its type's fields.
                bound.insert(q.hyper_var.clone(), cell_fields.get(&q.hyper_type).cloned().unwrap_or_default());
                for (v, t) in &q.overs {
                    if !seen_vars.insert(v.clone()) {
                        errs.push(ValidationError { line: 1, col: 1, msg: format!("duplicate bound var '{}'", v) });
                    }
                    match cell_dims.get(t) {
                        Some(&0) => {}
                        Some(&d) => errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("OVER '{}' has dim {} (must be 0)", t, d),
                        }),
                        None => errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("unknown OVER type '{}'", t),
                        }),
                    }
                    bound.insert(v.clone(), cell_fields.get(t).cloned().unwrap_or_default());
                    // Every (face-type → hypercell-type) pair needs a morphism.
                    let has_morph = morphs.values().any(|(f, t2, _)| f == t && t2 == &q.hyper_type);
                    if seen_types.contains(t) && seen_types.contains(&q.hyper_type) && !has_morph {
                        errs.push(ValidationError {
                            line: 1,
                            col: 1,
                            msg: format!("no morphism '{}' -> '{}'", t, q.hyper_type),
                        });
                    }
                }
                // Poset order sanity on every declared step (uses FILE-004).
                for (v, t) in &q.overs {
                    if let (Some(&fd), Some(&hd)) = (cell_dims.get(t), cell_dims.get(&q.hyper_type)) {
                        if !can_be_face(fd, hd) {
                            errs.push(ValidationError {
                                line: 1,
                                col: 1,
                                msg: format!("'{}' (dim {}) cannot bound '{}' (dim {})", v, fd, q.hyper_type, hd),
                            });
                        }
                    }
                }
                if let Some(e) = &q.where_clause {
                    expr_fields(e, &bound, &mut errs);
                }
                if let Some(vars) = &q.restrict {
                    for v in vars {
                        if !bound.contains_key(v) {
                            errs.push(ValidationError {
                                line: 1,
                                col: 1,
                                msg: format!("RESTRICT of unbound var '{}'", v),
                            });
                        }
                    }
                }
                if q.solve.max_iterations < 1 {
                    errs.push(ValidationError { line: 1, col: 1, msg: "MAX_ITERATIONS must be >= 1".to_string() });
                }
                if q.solve.energy_threshold < 0.0 || q.solve.regularization < 0.0 {
                    errs.push(ValidationError {
                        line: 1,
                        col: 1,
                        msg: "SOLVE params must be non-negative".to_string(),
                    });
                }
                if q.emit.is_empty() {
                    errs.push(ValidationError { line: 1, col: 1, msg: "EMIT list must not be empty".to_string() });
                }
                for item in &q.emit {
                    if let super::super::syntax::ast::EmitItem::Field(path) = item {
                        if path.len() < 2 {
                            errs.push(ValidationError { line: 1, col: 1, msg: "emit path needs var.field form".to_string() });
                            continue;
                        }
                        match bound.get(&path[0]) {
                            Some(fields) if fields.contains(&path[1]) => {}
                            Some(_) => errs.push(ValidationError {
                                line: 1,
                                col: 1,
                                msg: format!("unknown emit field '{}' on '{}'", path[1], path[0]),
                            }),
                            None => {
                                // Hypercell-own fields resolve against its type.
                                let hf = cell_fields.get(&q.hyper_type).cloned().unwrap_or_default();
                                if path[0] != q.hyper_var || !hf.contains(&path[1]) {
                                    errs.push(ValidationError {
                                        line: 1,
                                        col: 1,
                                        msg: format!("unbound emit path '{:?}'", path),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    errs
}

/// Eulerian δ²=0 gate (METHOD-009): verify that the coboundary operator
/// satisfies δ²=0 on a given complex. For a single hypercell with boundary
/// faces, this means the parity signs alternate correctly and the boundary
/// of the boundary is empty. For dim mismatches (a dim-k cell appearing in
/// a dim-(k+2) boundary), the gate rejects.
///
/// The check is: for each hypercell, the sum of signed boundary incidences
/// must be zero (each face appears exactly twice with opposite signs, or the
/// boundary is a closed cycle). This is the discrete analog of δ²=0.
pub fn check_eulerian_delta2(
    _hyper_dim: u8,
    boundary_dims: &[(String, u8)],
    parity_signs: &[f32],
) -> Result<(), String> {
    // Two-level scope: hypercell boundaries are dim-0 cells. Any other dim
    // breaks the Eulerian parity (δ²=0 requires codim-1 steps).
    for (name, dim) in boundary_dims {
        if *dim != 0 {
            return Err(format!(
                "dim mismatch: boundary '{}' has dim {} (must be 0 for two-level scope)",
                name, dim
            ));
        }
    }
    // δ²=0: the signed sum of boundary incidences must be zero.
    // For a closed boundary (cycle), each face appears twice with opposite
    // signs, summing to zero. For an open boundary, the sum is nonzero.
    let signed_sum: f32 = parity_signs.iter().sum();
    if signed_sum.abs() > 1e-6 {
        return Err(format!(
            "delta2 != 0: signed boundary sum = {} (expected 0 for closed boundary)",
            signed_sum
        ));
    }
    Ok(())
}

/// Check that a coboundary matrix D satisfies D^T D = I (orthonormality),
/// which is required for the harmonic solve to be well-posed.
pub fn check_coboundary_ortho(d: &[Vec<f32>]) -> Result<(), String> {
    if d.is_empty() {
        return Err("empty coboundary matrix".to_string());
    }
    let n = d[0].len();
    if d.iter().any(|row| row.len() != n) {
        return Err("coboundary matrix not rectangular".to_string());
    }
    // D^T D should be identity (orthonormal columns).
    for i in 0..n {
        for j in 0..n {
            let dot: f32 = d.iter().map(|row| row[i] * row[j]).sum();
            let expected = if i == j { 1.0 } else { 0.0 };
            if (dot - expected).abs() > 1e-4 {
                return Err(format!(
                    "coboundary not orthonormal: col {} dot col {} = {} (expected {})",
                    i, j, dot, expected
                ));
            }
        }
    }
    Ok(())
}
