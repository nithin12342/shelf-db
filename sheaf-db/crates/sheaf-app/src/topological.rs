//! Execute complete Topological Query Language (TQL) programs in local typed maps.
//! Boundaries retain the legacy two-level scope and insertion-order parity signs.
use crate::model::{BinaryOp, ExecutionStats, Expr, QueryOutput, Result, Row, Value};
use sheaf_native::hypercell::{self, CsrCoboundary};
use sheaf_query::ir::validator::validate_program;
use sheaf_query::syntax::{ast as ast, lexer::{lex, Spanned, Tok}, parser::Parser};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

type FieldTypes = HashMap<String, HashMap<String, String>>;
const MAX_CELLS: usize = 512;
const MAX_DIM: usize = 1024;
const MAX_WORK: usize = 20_000_000;

// The legacy tree discards discrete type names and narrows some numeric tokens.
// Retain those names and reject lossy numeric coercions before invoking its parser.
fn frontend(source: &str) -> Result<(ast::Program, FieldTypes)> {
    if source.len() > 65_536 { return Err("TQL source exceeds 65536 bytes".into()); }
    if !source.is_ascii() { return Err("the existing TQL lexer requires ASCII source".into()); }
    let tokens = lex(source).map_err(|e| format!("{}:{}: {}", e.line, e.col, e.msg))?;
    if tokens.len() > 16_384 { return Err("TQL token limit exceeded".into()); }
    let mut fields = FieldTypes::new();
    for stmt in tokens.split(|s| matches!(s.tok, Tok::Semi)) {
        let mut depth = 0usize;
        let mut operators = 0;
        let mut solve_keys = HashSet::new();
        let solve_start = stmt.iter().position(|s| matches!(&s.tok, Tok::Kw(k) if k == "SOLVE")).unwrap_or(usize::MAX);
        let definition = matches!(stmt.first().map(|s| &s.tok), Some(Tok::Kw(k)) if k == "DEFINE")
            && matches!(stmt.get(2).map(|s| &s.tok), Some(Tok::Kw(k)) if k == "TYPE");
        for (i, token) in stmt.iter().enumerate() {
            match &token.tok {
                Tok::LParen | Tok::LBracket | Tok::LBrace => {
                    depth += 1;
                    if depth > 64 { return Err("TQL nesting limit exceeded".into()); }
                }
                Tok::RParen | Tok::RBracket | Tok::RBrace => depth = depth.saturating_sub(1),
                Tok::Kw(s) if ["AND", "OR", "NOT"].contains(&s.as_str()) => {
                    operators += 1;
                    if operators > 64 { return Err("TQL expression limit exceeded".into()); }
                }
                Tok::Float(v) if !v.is_finite() => return Err("nonfinite literal".into()),
                Tok::Int(v) if *v < 0 && i > 0 && matches!(stmt[i - 1].tok, Tok::LParen) =>
                    return Err("cell identifiers must be nonnegative".into()),
                Tok::Ident(s) if s == "dim" && definition && i == 5 => {
                    if !matches!(stmt.get(i + 2).map(|t| &t.tok), Some(Tok::Int(0..=255))) {
                        return Err("topological dimension must be an integer in 0..255".into());
                    }
                }
                Tok::Ident(s) if s == "Float32" && matches!(stmt.get(i + 1).map(|t| &t.tok), Some(Tok::LBracket)) => {
                    if !matches!(stmt.get(i + 2).map(|t| &t.tok), Some(Tok::Int(1..=1024))) {
                        return Err("continuous dimension must be in 1..1024".into());
                    }
                }
                Tok::Ident(s) if i > solve_start && ["REGULARIZATION", "MAX_ITERATIONS", "ENERGY_THRESHOLD"].contains(&s.as_str())
                    && matches!(stmt.get(i + 1).map(|t| &t.tok), Some(Tok::Colon)) => {
                    if !solve_keys.insert(s) { return Err(format!("duplicate solve parameter {s}")); }
                    if s == "MAX_ITERATIONS" && !matches!(stmt.get(i + 2).map(|t| &t.tok), Some(Tok::Int(1..=512))) {
                        return Err("MAX_ITERATIONS must be an integer in 1..512".into());
                    }
                }
                _ => {}
            }
        }
        if matches!(stmt.first().map(|s| &s.tok), Some(Tok::Kw(s)) if s == "DEFINE")
            && matches!(stmt.get(2).map(|s| &s.tok), Some(Tok::Kw(s)) if s == "TYPE")
        {
            if let Some(Spanned { tok: Tok::Ident(name), .. }) = stmt.get(3) {
                let mut typed = HashMap::new();
                let body: Vec<_> = stmt.iter().skip_while(|s| !matches!(s.tok, Tok::LBrace))
                    .skip(1).take_while(|s| !matches!(s.tok, Tok::RBrace)).collect();
                for part in body.windows(3) {
                    if let (Tok::Ident(field), Tok::Colon, Tok::Ident(ty)) = (&part[0].tok, &part[1].tok, &part[2].tok) {
                        if !["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64", "Float32", "Float64", "string", "bool"].contains(&ty.as_str()) {
                            return Err(format!("unsupported discrete type {ty}"));
                        }
                        if typed.insert(field.clone(), ty.clone()).is_some() { return Err(format!("duplicate field {name}.{field}")); }
                    }
                }
                fields.insert(name.clone(), typed);
            }
        }
    }
    let program = Parser::new(tokens).parse_program().map_err(|e| e.to_string())?;
    if program.stmts.len() > 2048 { return Err("TQL statement limit exceeded".into()); }
    Ok((program, fields))
}

fn literal(l: &ast::Lit) -> Value {
    match l {
        ast::Lit::Int(v) => Value::Int(*v), ast::Lit::Float(v) => Value::Float(*v),
        ast::Lit::Str(v) => Value::Text(v.clone()), ast::Lit::Bool(v) => Value::Bool(*v),
    }
}

fn typed_literal(l: &ast::Lit, ty: &str) -> bool {
    match (ty, l) {
        ("string", ast::Lit::Str(_)) | ("bool", ast::Lit::Bool(_)) => true,
        ("f32" | "Float32", ast::Lit::Float(v)) => v.is_finite() && (*v as f32).is_finite(),
        ("f64" | "Float64", ast::Lit::Float(v)) => v.is_finite(),
        ("f32" | "Float32" | "f64" | "Float64", ast::Lit::Int(_)) => true,
        (t, ast::Lit::Int(v)) if t.starts_with('u') => {
            let bits = t[1..].parse::<u32>().unwrap_or(0);
            *v >= 0 && (bits == 64 || (bits > 0 && (*v as u64) < (1u64 << bits)))
        }
        (t, ast::Lit::Int(v)) if t.starts_with('i') => {
            let bits = t[1..].parse::<u32>().unwrap_or(0);
            bits == 64 || (bits > 0 && *v >= -(1i64 << (bits - 1)) && *v < (1i64 << (bits - 1)))
        }
        _ => false,
    }
}

struct Cell<'a> {
    typ: &'a str,
    id: u64,
    discrete: &'a [(String, ast::Lit)],
    stalk: &'a [f32],
    boundary: &'a [ast::CellRef],
}

struct Local<'a> {
    types: HashMap<&'a str, &'a ast::CellDef>,
    morphs: HashMap<(&'a str, &'a str), &'a ast::MorphDef>,
    cells: HashMap<u64, Cell<'a>>,
    order: Vec<u64>,
}

impl<'a> Local<'a> {
    fn definition(&self, name: &str) -> Result<&'a ast::CellDef> {
        self.types.get(name).copied().ok_or_else(|| format!("unknown type {name}"))
    }

    fn morph(&self, from: &str, to: &str) -> Result<&'a ast::MorphDef> {
        self.morphs.get(&(from, to)).copied().ok_or_else(|| format!("missing morphism {from} -> {to}"))
    }

    fn load(program: &'a ast::Program, fields: &FieldTypes) -> Result<Self> {
        let mut local = Self { types: HashMap::new(), morphs: HashMap::new(), cells: HashMap::new(), order: Vec::new() };
        for stmt in &program.stmts {
            match stmt {
                ast::Stmt::DefineCell(d) => {
                    if d.cont_dim == 0 || d.cont_dim > MAX_DIM { return Err("continuous dimension limit exceeded".into()); }
                    if local.types.insert(&d.name, d).is_some() { return Err(format!("duplicate type {}", d.name)); }
                }
                ast::Stmt::DefineMorphism(m) => {
                    let from = local.definition(&m.from_type)?;
                    let to = local.definition(&m.to_type)?;
                    if from.dim != 0 || to.dim == 0 { return Err("morphisms require a dim-0 source and a hypercell target".into()); }
                    if m.matrix.len() != to.cont_dim || m.matrix.iter().any(|r| r.len() != from.cont_dim
                        || r.iter().any(|v| !v.is_finite() || !(*v as f32).is_finite())) {
                        return Err(format!("morphism {} requires a finite {} by {} matrix", m.name, to.cont_dim, from.cont_dim));
                    }
                    let (mut sources, mut targets) = (HashSet::new(), HashSet::new());
                    for (a, b) in &m.proj_fields {
                        if !from.discrete_fields.contains(a) || !to.discrete_fields.contains(b)
                            || !sources.insert(a) || !targets.insert(b) {
                            return Err(format!("invalid or ambiguous projection in {}", m.name));
                        }
                    }
                    if local.morphs.insert((&m.from_type, &m.to_type), m).is_some() {
                        return Err(format!("ambiguous morphisms {} -> {}", m.from_type, m.to_type));
                    }
                }
                ast::Stmt::InsertCell(c) => local.insert(Cell { typ: &c.type_name, id: c.id,
                    discrete: &c.discrete, stalk: &c.stalk, boundary: &[] }, fields, false)?,
                ast::Stmt::InsertHyper(c) => local.insert(Cell { typ: &c.type_name, id: c.id,
                    discrete: &c.discrete, stalk: &c.stalk, boundary: &c.boundary }, fields, true)?,
                ast::Stmt::Query(_) => {}
            }
        }
        for c in local.cells.values() {
            let mut seen = HashSet::new();
            for face in c.boundary {
                let other = local.cells.get(&face.id).ok_or_else(|| format!("missing boundary reference {}({})", face.type_name, face.id))?;
                if other.typ != face.type_name || local.definition(other.typ)?.dim != 0 || !seen.insert(face.id) {
                    return Err(format!("invalid, repeated, or mistyped boundary reference {}({})", face.type_name, face.id));
                }
                local.morph(other.typ, c.typ)?;
            }
        }
        Ok(local)
    }

    fn insert(&mut self, c: Cell<'a>, fields: &FieldTypes, hyper: bool) -> Result<()> {
        let d = self.definition(c.typ)?;
        if (d.dim > 0) != hyper || c.stalk.len() != d.cont_dim || c.stalk.iter().any(|v| !v.is_finite()) {
            return Err(format!("invalid dimension or stalk for {}({})", c.typ, c.id));
        }
        if hyper && (c.boundary.is_empty() || c.boundary.len() > 64) { return Err("hypercell boundary size must be in 1..64".into()); }
        let declared = fields.get(c.typ).ok_or("missing discrete field declarations")?;
        let mut seen = HashSet::new();
        for (name, value) in c.discrete {
            if !seen.insert(name) || !declared.get(name).is_some_and(|ty| typed_literal(value, ty)) {
                return Err(format!("invalid discrete field {}.{name}", c.typ));
            }
        }
        if seen.len() != declared.len() { return Err(format!("missing discrete fields on {}", c.typ)); }
        if self.cells.len() >= MAX_CELLS { return Err("cell limit of 512 exceeded".into()); }
        if self.cells.contains_key(&c.id) { return Err(format!("duplicate cell identifier {}", c.id)); }
        self.order.push(c.id);
        self.cells.insert(c.id, c);
        Ok(())
    }

    fn compact(&self, h: &Cell<'_>) -> Result<(CsrCoboundary, Vec<f32>, Vec<usize>)> {
        let mut initial = Vec::new();
        let mut starts = vec![0];
        let mut morphs = Vec::new();
        for face in h.boundary {
            let c = self.cells.get(&face.id).ok_or("missing boundary cell")?;
            initial.extend_from_slice(c.stalk);
            starts.push(initial.len());
            morphs.push(self.morph(c.typ, h.typ)?);
        }
        let rows = self.definition(h.typ)?.cont_dim;
        if rows.checked_mul(initial.len()).is_none_or(|n| n > 1_048_576) { return Err("restriction matrix limit exceeded".into()); }
        let mut matrix = CsrCoboundary { cols: initial.len(), row_offsets: vec![0], columns: Vec::new(), weights: Vec::new() };
        for row in 0..rows {
            for (face, morph) in morphs.iter().enumerate() {
                let sign = if face % 2 == 0 { 1.0 } else { -1.0 };
                for (column, weight) in morph.matrix[row].iter().enumerate() {
                    let weight = sign * *weight as f32;
                    if weight != 0.0 { matrix.columns.push(starts[face] + column); matrix.weights.push(weight); }
                }
            }
            matrix.row_offsets.push(matrix.columns.len());
        }
        Ok((matrix, initial, starts))
    }
}

fn column(path: &[String]) -> Result<Expr> {
    if path.len() != 2 { return Err("field paths must have exactly two parts".into()); }
    Ok(Expr::Column { relation: Some(path[0].clone()), name: path[1].clone() })
}

fn expression(e: &ast::Expr) -> Result<Expr> {
    let binary = |left, op, right| Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
    Ok(match e {
        ast::Expr::And(a, b) => binary(expression(a)?, BinaryOp::And, expression(b)?),
        ast::Expr::Or(a, b) => binary(expression(a)?, BinaryOp::Or, expression(b)?),
        ast::Expr::Not(e) => Expr::Not(Box::new(expression(e)?)),
        ast::Expr::Cmp { path, op, lit } => {
            let op = match op { ast::CmpOp::Eq => BinaryOp::Eq, ast::CmpOp::Ne => BinaryOp::Ne,
                ast::CmpOp::Gt => BinaryOp::Gt, ast::CmpOp::Lt => BinaryOp::Lt,
                ast::CmpOp::Ge => BinaryOp::Ge, ast::CmpOp::Le => BinaryOp::Le };
            binary(column(path)?, op, Expr::Literal(literal(lit)))
        }
    })
}

fn keep(q: &ast::Query, var: &str) -> bool {
    var == q.hyper_var || q.restrict.as_ref().is_none_or(|vars| vars.iter().any(|v| v == var))
}

/// Execute one complete program with exactly one final MATCH statement.
/// Stalk projections appear in per-cell plan diagnostics, separate from EMIT columns.
pub fn execute(source: &str) -> Result<QueryOutput> {
    let start = Instant::now();
    let (program, fields) = frontend(source)?;
    let query = match program.stmts.last() {
        Some(ast::Stmt::Query(q)) if program.stmts.iter().filter(|s| matches!(s, ast::Stmt::Query(_))).count() == 1 => q,
        _ => return Err("a program requires exactly one final MATCH query".into()),
    };
    // The legacy validator imposes equal source and target stalk sizes. Our
    // typed rectangular shape gate replaces only that obsolete restriction.
    let local = Local::load(&program, &fields)?;
    for error in validate_program(&program) {
        if !error.msg.ends_with("(one d per query)") {
            return Err(format!("{}:{}: {}", error.line, error.col, error.msg));
        }
    }
    if query.overs.is_empty() || query.overs.len() > 64 { return Err("OVER requires 1..64 boundary variables".into()); }
    if query.overs.iter().any(|(v, _)| v == &query.hyper_var) { return Err("hypercell and boundary variables must differ".into()); }
    if let Some(vars) = &query.restrict {
        let mut seen = HashSet::new();
        if vars.iter().any(|v| !seen.insert(v) || !query.overs.iter().any(|(a, _)| a == v)) {
            return Err("RESTRICT requires unique boundary variables".into());
        }
    }
    let lambda = query.solve.regularization as f32;
    if !lambda.is_finite() || lambda < 0.0 || !query.solve.energy_threshold.is_finite() || query.solve.energy_threshold < 0.0 {
        return Err("SOLVE requires finite nonnegative parameters".into());
    }
    let filter = query.where_clause.as_ref().map(expression).transpose()?;
    let mut emitted = Vec::new();
    let mut out = QueryOutput { tql: Some(source.to_owned()), ..QueryOutput::default() };
    for item in &query.emit {
        let name = match item {
            ast::EmitItem::Field(path) => {
                column(path)?;
                if !keep(query, &path[0]) { continue; }
                path.join(".")
            }
            ast::EmitItem::SectionEnergy => "SECTION_ENERGY".into(),
            ast::EmitItem::CohomologyState => "COHOMOLOGY_STATE".into(),
        };
        out.columns.push(name);
        emitted.push(item);
    }
    let backend = if sheaf_native::native_kernel_linked() { "native C++" } else { "Rust fallback" };
    out.plan.push(format!("{backend}: typed local scan, ordered boundary match, WHERE, compact CSR, shifted conjugate gradient, EMIT"));
    out.plan.push("Parity signs alternate +,- in boundary order. Stalk projections are solved coordinates. Discrete projection maps are shape-checked only. State labels classify energy, not a cohomology proof.".into());
    let mut work = 0usize;
    let mut output_bytes = 0usize;
    for id in &local.order {
        let h = local.cells.get(id).ok_or("missing local cell")?;
        if h.typ != query.hyper_type { continue; }
        out.stats.scanned_rows += 1;
        if h.boundary.len() != query.overs.len() || h.boundary.iter().zip(&query.overs).any(|(f, (_, t))| &f.type_name != t) { continue; }
        let (mut columns, mut row) = (Vec::new(), Row::new());
        for (var, c) in std::iter::once((&query.hyper_var, h)).chain(query.overs.iter().zip(h.boundary)
            .filter_map(|((v, _), f)| local.cells.get(&f.id).map(|c| (v, c)))) {
            for (name, value) in c.discrete {
                columns.push(format!("{var}.{name}"));
                row.push(literal(value));
            }
        }
        if let Some(e) = &filter {
            if !matches!(crate::eval::eval(e, &columns, &row)?, Value::Bool(true)) { continue; }
        }
        out.stats.candidate_rows += 1;
        let (matrix, initial, starts) = local.compact(h)?;
        let cost = matrix.cols.checked_add(matrix.columns.len()).and_then(|n| n.checked_add(matrix.row_offsets.len()))
            .and_then(|n| n.checked_mul(query.solve.max_iterations)).ok_or("solve work overflow")?;
        work = work.checked_add(cost).ok_or("solve work overflow")?;
        if work > MAX_WORK { return Err("program solve work limit exceeded".into()); }
        let result = hypercell::solve(&matrix, &initial, lambda, query.solve.max_iterations, 0.000001)?;
        let state = if !result.converged { "NOT_CONVERGED" } else if (result.energy as f64) < query.solve.energy_threshold {
            "HARMONIC_BOUNDED"
        } else { "UNBOUNDED" };
        let mut values = Row::new();
        for item in &emitted {
            values.push(match item {
                ast::EmitItem::Field(path) => crate::eval::eval(&column(path)?, &columns, &row)?,
                ast::EmitItem::SectionEnergy => Value::Float(result.energy as f64),
                ast::EmitItem::CohomologyState => Value::Text(state.into()),
            });
        }
        let mut projections = Vec::new();
        for (i, (var, _)) in query.overs.iter().enumerate() {
            if keep(query, var) { projections.push(format!("{var}={:?}", &result.values[starts[i]..starts[i + 1]])); }
        }
        let diagnostic = format!("{}({}): cols={}, rows={}, nnz={}, iterations={}, converged={}, energy={}, projections: {}",
            h.typ, h.id, matrix.cols, matrix.row_offsets.len() - 1, matrix.columns.len(), result.iterations,
            result.converged, result.energy, projections.join(", "));
        output_bytes += diagnostic.len() + values.iter().map(Value::bytes).sum::<usize>();
        if output_bytes > 8 * 1024 * 1024 { return Err("topological output limit exceeded".into()); }
        out.plan.push(diagnostic);
        out.rows.push(values);
    }
    out.stats = ExecutionStats { output_rows: out.rows.len(), elapsed_us: start.elapsed().as_micros(), ..out.stats };
    Ok(out)
}
