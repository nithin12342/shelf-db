use crate::model::{Aggregate, BinaryOp, Expr, Order, Projection, Result, Value};
use sqlparser::ast as ast;

pub(crate) fn ident(id: &ast::Ident) -> String {
    if id.quote_style.is_some() { id.value.clone() } else { id.value.to_lowercase() }
}

pub(crate) fn object(name: &ast::ObjectName) -> Result<String> {
    match name.0.as_slice() {
        [ast::ObjectNamePart::Identifier(id)] => Ok(ident(id)),
        _ => Err("only single-part object names are supported".into()),
    }
}

pub(crate) fn number(text: &str) -> Result<Value> {
    if text.contains(['.', 'e', 'E']) {
        let value: f64 = text.parse().map_err(|_| "invalid floating-point literal")?;
        if !value.is_finite() {
            return Err("nonfinite floating-point literal".into());
        }
        Ok(Value::Float(value))
    } else {
        text.parse().map(Value::Int).map_err(|_| "integer literal exceeds signed 64-bit range".into())
    }
}

pub(crate) fn literal(expr: &ast::Expr) -> Result<Value> {
    match expr {
        ast::Expr::Nested(inner) => literal(inner),
        ast::Expr::Value(value) => match &value.value {
            ast::Value::Null => Ok(Value::Null),
            ast::Value::Boolean(v) => Ok(Value::Bool(*v)),
            ast::Value::SingleQuotedString(v) => Ok(Value::Text(v.clone())),
            ast::Value::Number(v, false) => number(v),
            _ => Err("unsupported literal".into()),
        },
        ast::Expr::UnaryOp { op, expr } if matches!(op, ast::UnaryOperator::Plus | ast::UnaryOperator::Minus) => {
            // Parse the sign with the digits so that i64::MIN remains representable.
            let mut inner = expr.as_ref();
            while let ast::Expr::Nested(nested) = inner { inner = nested; }
            if let ast::Expr::Value(value) = inner {
                if let ast::Value::Number(v, false) = &value.value {
                    return number(&format!("{}{v}", if *op == ast::UnaryOperator::Minus { "-" } else { "" }));
                }
            }
            match (op, literal(inner)?) {
                (ast::UnaryOperator::Plus, v @ (Value::Int(_) | Value::Float(_))) => Ok(v),
                (ast::UnaryOperator::Minus, Value::Int(v)) => v.checked_neg().map(Value::Int)
                    .ok_or_else(|| "integer negation overflow".into()),
                (ast::UnaryOperator::Minus, Value::Float(v)) => Ok(Value::Float(-v)),
                _ => Err("numeric sign requires a numeric literal".into()),
            }
        }
        _ => Err("VALUES requires typed literals".into()),
    }
}

pub(crate) fn convert(expr: &ast::Expr, aggregates: bool) -> Result<Expr> {
    convert_at(expr, aggregates, 0)
}

fn convert_at(expr: &ast::Expr, aggregates: bool, depth: usize) -> Result<Expr> {
    if depth >= 64 { return Err("expression nesting exceeds 64".into()); }
    let child = |e: &ast::Expr| convert_at(e, aggregates, depth + 1);
    match expr {
        ast::Expr::Identifier(id) => Ok(Expr::Column { relation: None, name: ident(id) }),
        ast::Expr::CompoundIdentifier(ids) if ids.len() == 2 => Ok(Expr::Column {
            relation: Some(ident(&ids[0])), name: ident(&ids[1]),
        }),
        ast::Expr::Value(_) => literal(expr).map(Expr::Literal),
        ast::Expr::Nested(inner) => convert_at(inner, aggregates, depth),
        ast::Expr::BinaryOp { left, op, right } => {
            use ast::BinaryOperator as B;
            let op = match op {
                B::Eq => BinaryOp::Eq, B::NotEq => BinaryOp::Ne,
                B::Lt => BinaryOp::Lt, B::LtEq => BinaryOp::Le,
                B::Gt => BinaryOp::Gt, B::GtEq => BinaryOp::Ge,
                B::And => BinaryOp::And, B::Or => BinaryOp::Or,
                B::Plus => BinaryOp::Add, B::Minus => BinaryOp::Sub,
                B::Multiply => BinaryOp::Mul, B::Divide => BinaryOp::Div,
                _ => return Err("unsupported binary operator".into()),
            };
            Ok(Expr::Binary { left: Box::new(child(left)?), op, right: Box::new(child(right)?) })
        }
        ast::Expr::UnaryOp { op: ast::UnaryOperator::Not, expr } => Ok(Expr::Not(Box::new(child(expr)?))),
        ast::Expr::UnaryOp { op, expr: inner } if matches!(op, ast::UnaryOperator::Plus | ast::UnaryOperator::Minus) => {
            if let Ok(value) = literal(expr) { return Ok(Expr::Literal(value)); }
            if numeric_literal(inner) { return Err("invalid signed numeric literal".into()); }
            let value = child(inner)?;
            if *op == ast::UnaryOperator::Plus { return Ok(value); }
            Ok(Expr::Binary {
                left: Box::new(Expr::Literal(Value::Int(0))), op: BinaryOp::Sub, right: Box::new(value),
            })
        }
        ast::Expr::IsNull(inner) | ast::Expr::IsNotNull(inner) => Ok(Expr::IsNull {
            expr: Box::new(child(inner)?), negated: matches!(expr, ast::Expr::IsNotNull(_)),
        }),
        ast::Expr::Function(function) if aggregates => aggregate(function, depth),
        _ => Err("unsupported expression".into()),
    }
}

fn numeric_literal(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Value(v) => matches!(v.value, ast::Value::Number(_, _)),
        ast::Expr::Nested(e) | ast::Expr::UnaryOp { expr: e, .. } => numeric_literal(e),
        _ => false,
    }
}

fn aggregate(f: &ast::Function, depth: usize) -> Result<Expr> {
    if f.uses_odbc_syntax || f.parameters != ast::FunctionArguments::None
        || f.filter.is_some() || f.null_treatment.is_some() || f.over.is_some()
        || !f.within_group.is_empty()
    { return Err("unsupported aggregate modifier".into()); }
    let function = match object(&f.name)?.as_str() {
        "count" => Aggregate::Count, "sum" => Aggregate::Sum,
        "avg" => Aggregate::Avg, "min" => Aggregate::Min, "max" => Aggregate::Max,
        _ => return Err("only COUNT, SUM, AVG, MIN, and MAX are supported".into()),
    };
    let ast::FunctionArguments::List(args) = &f.args else {
        return Err("aggregate requires an argument list".into());
    };
    if !args.clauses.is_empty() { return Err("unsupported aggregate argument clause".into()); }
    let distinct = args.duplicate_treatment == Some(ast::DuplicateTreatment::Distinct);
    let expr = match args.args.as_slice() {
        [ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Wildcard)] if function == Aggregate::Count && !distinct => None,
        [ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(expr))] => {
            Some(Box::new(convert_at(expr, false, depth + 1)?))
        }
        _ => return Err("aggregate requires one expression, or COUNT(*)".into()),
    };
    Ok(Expr::Aggregate { function, expr, distinct })
}

pub(crate) fn projection(item: &ast::SelectItem) -> Result<Projection> {
    let (expr, alias) = match item {
        ast::SelectItem::UnnamedExpr(expr) => (convert(expr, true)?, None),
        ast::SelectItem::ExprWithAlias { expr, alias } => (convert(expr, true)?, Some(ident(alias))),
        ast::SelectItem::Wildcard(options) => (wildcard(None, options)?, None),
        ast::SelectItem::QualifiedWildcard(ast::SelectItemQualifiedWildcardKind::ObjectName(name), options) => {
            (wildcard(Some(object(name)?), options)?, None)
        }
        _ => return Err("unsupported projection".into()),
    };
    Ok(Projection { expr, alias })
}

fn wildcard(relation: Option<String>, o: &ast::WildcardAdditionalOptions) -> Result<Expr> {
    if o.opt_ilike.is_some() || o.opt_exclude.is_some() || o.opt_except.is_some()
        || o.opt_replace.is_some() || o.opt_rename.is_some()
    { return Err("wildcard modifiers are not supported".into()); }
    Ok(Expr::Column { relation, name: "*".into() })
}

pub(crate) fn group(expr: &ast::Expr) -> Result<Expr> {
    reject_ordinal(expr)?;
    convert(expr, false)
}

pub(crate) fn order(item: &ast::OrderByExpr) -> Result<Order> {
    if matches!(&item.expr, ast::Expr::Identifier(id) if id.quote_style.is_none() && id.value.eq_ignore_ascii_case("ALL")) {
        return Err("ORDER BY ALL is not supported".into());
    }
    if item.with_fill.is_some() { return Err("ORDER BY WITH FILL is not supported".into()); }
    reject_ordinal(&item.expr)?;
    Ok(Order { expr: convert(&item.expr, true)?, ascending: item.options.asc.unwrap_or(true),
        nulls_first: item.options.nulls_first })
}

fn reject_ordinal(expr: &ast::Expr) -> Result<()> {
    if matches!(literal(expr), Ok(Value::Int(_))) {
        Err("positional GROUP BY and ORDER BY are not supported".into())
    } else { Ok(()) }
}

pub(crate) fn equijoin(expr: &Expr) -> Result<()> {
    match expr {
        Expr::Binary { left, op: BinaryOp::Eq, right }
            if matches!(left.as_ref(), Expr::Column { name, .. } if name != "*")
                && matches!(right.as_ref(), Expr::Column { name, .. } if name != "*") => Ok(()),
        Expr::Binary { left, op: BinaryOp::And, right } => { equijoin(left)?; equijoin(right) }
        _ => Err("INNER JOIN requires column equality, optionally joined with AND".into()),
    }
}

pub(crate) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(crate) fn render(expr: &Expr) -> String {
    match expr {
        Expr::Column { relation, name } => {
            let name = if name == "*" { "*".into() } else { quote(name) };
            relation.as_ref().map_or(name.clone(), |r| format!("{}.{name}", quote(r)))
        }
        Expr::Literal(value) => match value {
            Value::Null => "NULL".into(),
            Value::Bool(v) => if *v { "TRUE" } else { "FALSE" }.into(),
            Value::Int(v) => v.to_string(),
            // Debug formatting retains the decimal point for integral floating-point values.
            Value::Float(v) => format!("{v:?}"),
            Value::Text(v) => format!("'{}'", v.replace('\'', "''")),
        },
        Expr::Binary { left, op, right } => {
            let op = match op {
                BinaryOp::Eq => "=", BinaryOp::Ne => "<>", BinaryOp::Lt => "<", BinaryOp::Le => "<=",
                BinaryOp::Gt => ">", BinaryOp::Ge => ">=", BinaryOp::And => "AND", BinaryOp::Or => "OR",
                BinaryOp::Add => "+", BinaryOp::Sub => "-", BinaryOp::Mul => "*", BinaryOp::Div => "/",
            };
            format!("({} {op} {})", render(left), render(right))
        }
        Expr::Not(expr) => format!("(NOT {})", render(expr)),
        Expr::IsNull { expr, negated } => format!("({} IS {}NULL)", render(expr), if *negated { "NOT " } else { "" }),
        Expr::Aggregate { function, expr, distinct } => {
            let name = match function { Aggregate::Count => "COUNT", Aggregate::Sum => "SUM",
                Aggregate::Avg => "AVG", Aggregate::Min => "MIN", Aggregate::Max => "MAX" };
            format!("{name}({}{})", if *distinct { "DISTINCT " } else { "" },
                expr.as_deref().map_or_else(|| "*".into(), render))
        }
    }
}
