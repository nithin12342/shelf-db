// crates/sheaf-query/src/syntax/ast.rs — represent match restrict emit
// FILE-032. Depends on nothing. Plain data: parser builds it, TIR lowers it,
// validator checks it. Discrete literal maps are ordered pairs (insertion
// order preserved for canonical class hashing downstream).
#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Cmp { path: Vec<String>, op: CmpOp, lit: Lit },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellDef {
    pub name: String,
    pub dim: u8,
    pub discrete_fields: Vec<String>,
    pub cont_dim: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MorphDef {
    pub name: String,
    pub from_type: String,
    pub to_type: String,
    pub proj_fields: Vec<(String, String)>,
    pub matrix: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InsertCell {
    pub type_name: String,
    pub id: u64,
    pub discrete: Vec<(String, Lit)>,
    pub stalk: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellRef {
    pub type_name: String,
    pub id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InsertHyper {
    pub type_name: String,
    pub id: u64,
    pub boundary: Vec<CellRef>,
    pub discrete: Vec<(String, Lit)>,
    pub stalk: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolveParams {
    pub regularization: f64,
    pub max_iterations: usize,
    pub energy_threshold: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmitItem {
    Field(Vec<String>),
    SectionEnergy,
    CohomologyState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub hyper_var: String,
    pub hyper_type: String,
    pub overs: Vec<(String, String)>,
    pub where_clause: Option<Expr>,
    pub restrict: Option<Vec<String>>,
    pub solve: SolveParams,
    pub emit: Vec<EmitItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    DefineCell(CellDef),
    DefineMorphism(MorphDef),
    InsertCell(InsertCell),
    InsertHyper(InsertHyper),
    Query(Query),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}
