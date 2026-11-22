use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, String>;
pub type Row = Vec<Value>;
pub const MAX_REQUEST_BYTES: usize = 65_536;
pub const MAX_ROWS: usize = 100_000;
pub const MAX_QUERY_ROWS: usize = 100_000;
pub const MEMORY_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl Value {
    pub fn text(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Bool(v) => v.to_string(),
            Self::Int(v) => v.to_string(),
            Self::Float(v) => v.to_string(),
            Self::Text(v) => v.clone(),
        }
    }

    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Text(v) => v.len(),
                _ => 0,
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Int,
    Float,
    Text,
    Bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aggregate {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Column {
        relation: Option<String>,
        name: String,
    },
    Literal(Value),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Not(Box<Expr>),
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },
    Aggregate {
        function: Aggregate,
        expr: Option<Box<Expr>>,
        distinct: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Projection {
    pub expr: Expr,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub table: String,
    pub alias: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Join {
    pub relation: Relation,
    pub on: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub expr: Expr,
    pub ascending: bool,
    pub nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub from: Relation,
    pub joins: Vec<Join>,
    pub filter: Option<Expr>,
    pub projection: Vec<Projection>,
    pub group_by: Vec<Expr>,
    pub order_by: Vec<Order>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub distinct: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Create {
        table: String,
        columns: Vec<Column>,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        rows: Vec<Row>,
    },
    Update {
        table: String,
        assignments: Vec<(String, Expr)>,
        filter: Option<Expr>,
    },
    Delete {
        table: String,
        filter: Option<Expr>,
    },
    Query(Query),
    Explain {
        query: Query,
        analyze: bool,
    },
}

pub trait DataSource {
    fn columns(&self, table: &str) -> Result<Vec<Column>>;
    fn rows(&self, table: &str) -> Result<Vec<Row>>;
    /// Read the table with page-level pruning (NF-12). The default source has
    /// no page summaries, so it reports zero pages and decodes every row.
    fn scan(&self, table: &str, columns: &[String], predicates: &[Expr]) -> Result<(Vec<Row>, ScanStats)> {
        let _ = (columns, predicates);
        Ok((self.rows(table)?, ScanStats::default()))
    }
}

/// What one table scan did with the page summaries (NF-12).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanStats {
    /// The pages in the table.
    pub pages_total: usize,
    /// The pages skipped before decode because the summary cannot match.
    pub pages_skipped: usize,
    /// The rows the decoder returned.
    pub rows_decoded: usize,
    /// The number of summary clauses the scan tested. Zero means no pruning.
    pub page_clauses: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub scanned_rows: usize,
    pub candidate_rows: usize,
    pub output_rows: usize,
    pub elapsed_us: u128,
    /// The pages considered by every scan (NF-12).
    pub pages_total: usize,
    /// The pages skipped before decode (NF-12).
    pub pages_skipped: usize,
    /// The summary clauses the scans tested (NF-12).
    pub page_clauses: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub affected: usize,
    pub tql: Option<String>,
    pub plan: Vec<String>,
    pub stats: ExecutionStats,
}
