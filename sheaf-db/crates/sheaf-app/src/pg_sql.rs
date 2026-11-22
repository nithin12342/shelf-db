//! One-command autocommit policy. Parsing completes before the application executes SQL.
use super::codec::{self, WireError};
use crate::model::{QueryOutput, Statement, Value};
use crate::service::{Application, Request};

pub(super) fn execute(application: &Application, text: &str, optimize: bool) -> codec::Result<Option<Vec<u8>>> {
    let statements = match crate::sql::parse(text) {
        Ok(statements) => statements,
        Err(error) if error == "request contains no statements" => return Ok(None),
        Err(error) => return Err(sql_error(error, true)),
    };
    let [statement] = statements.as_slice() else {
        return Err(WireError::new("0A000",
            "send one SQL command per Query message. Multi-command requests are not supported"));
    };
    // The application remains the only execution entry point for wire SQL.
    let outputs = application.execute(&Request {
        language: "sql".into(), query: text.into(), optimize,
    }).map_err(|error| sql_error(error, false))?;
    let mut outputs = outputs.into_iter();
    let mut output = outputs.next().ok_or_else(|| WireError::new("XX000", "missing query output"))?;
    if outputs.next().is_some() { return Err(WireError::new("XX000", "unexpected extra query output")); }
    let tag = match statement {
        Statement::Create { .. } => "CREATE TABLE".into(),
        Statement::Insert { .. } => format!("INSERT 0 {}", output.affected),
        Statement::Update { .. } => format!("UPDATE {}", output.affected),
        Statement::Delete { .. } => format!("DELETE {}", output.affected),
        Statement::Query(_) => format!("SELECT {}", output.rows.len()),
        Statement::Explain { analyze, .. } => {
            output = explain(output, *analyze);
            "EXPLAIN".into()
        }
        Statement::DefineMorphism(_) => "DEFINE MORPHISM".into(),
    };
    codec::result(&output, &tag).map(Some)
}

fn explain(output: QueryOutput, analyze: bool) -> QueryOutput {
    let mut lines = Vec::new();
    if let Some(tql) = output.tql { lines.push(format!("TQL: {tql}")); }
    lines.extend(output.plan);
    if analyze {
        lines.push(format!("Measured elapsed_us={} scanned_rows={} candidate_rows={} output_rows={}",
            output.stats.elapsed_us, output.stats.scanned_rows,
            output.stats.candidate_rows, output.stats.output_rows));
    }
    QueryOutput { columns: vec!["QUERY PLAN".into()],
        rows: lines.into_iter().map(|line| vec![Value::Text(line)]).collect(),
        ..Default::default() }
}

fn sql_error(mut message: String, parsing: bool) -> WireError {
    let lower = message.to_ascii_lowercase();
    let code = if lower == "unsupported statement" {
        message.push_str(". Use one supported SQL command in autocommit. Explicit transactions are not supported");
        "0A000"
    } else if lower.contains("unsupported") || lower.contains("not supported") || lower.contains("only ") {
        "0A000"
    } else if lower.contains("unknown table") { "42P01"
    } else if lower.contains("unknown column") { "42703"
    } else if lower.contains("ambiguous") { "42702"
    } else if lower.contains("table already exists") { "42P07"
    } else if lower.contains("invalid value for column") { "42804"
    } else if lower.contains("division by zero") { "22012"
    } else if lower.contains("overflow") { "22003"
    } else if lower.contains("limit") || lower.contains("exceeds") || lower.contains("exhausted") { "54000"
    } else if parsing { "42601"
    } else { "XX000" };
    WireError::new(code, message)
}
