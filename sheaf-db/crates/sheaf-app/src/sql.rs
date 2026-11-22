use crate::model::{Join, Query, Relation, Result, Statement};
use sqlparser::{ast, dialect::GenericDialect, parser::Parser, tokenizer::Token};

pub(crate) use crate::sql_expr as expr;
#[path = "sql_guard.rs"]
mod guard;
#[path = "sql_write.rs"]
mod write;

pub(crate) use guard::{keyword, tokens};

pub fn parse(text: &str) -> Result<Vec<Statement>> {
    let tokens = tokens(text)?;
    // A morphism declaration is not part of the sqlparser grammar. Handle the
    // one-statement form here with its own strict parser, then report an
    // explicit error for a mixed request.
    if tokens.iter().any(|t| keyword(t, "DEFINE")) {
        if !crate::ddl_morphism::is_declaration(&tokens)
            || tokens.iter().filter(|t| matches!(t, Token::SemiColon)).count() != 1
        {
            return Err("DEFINE MORPHISM must be the only statement in a request".into());
        }
        return Ok(vec![Statement::DefineMorphism(crate::ddl_morphism::parse(text)?)]);
    }
    on_parser_stack(move || {
        let statements = Parser::new(&GenericDialect).with_recursion_limit(256)
            .with_tokens(tokens).parse_statements().map_err(|e| e.to_string())?;
        if statements.is_empty() { return Err("request contains no statements".into()); }
        statements.iter().map(statement).collect()
    })
}

pub(crate) fn on_parser_stack<T: Send + 'static>(parse: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    // sqlparser's recursive frames exceed the Windows test stack at 64 parentheses.
    std::thread::Builder::new().name("row-query-parser".into()).stack_size(8 * 1024 * 1024)
        .spawn(parse).map_err(|e| format!("cannot start parser: {e}"))?
        .join().map_err(|_| "parser failed".to_string())?
}

fn statement(statement: &ast::Statement) -> Result<Statement> {
    match statement {
        ast::Statement::Query(q) => query(q).map(Statement::Query),
        ast::Statement::CreateTable(c) => write::create(c),
        ast::Statement::Insert(i) => write::insert(i),
        ast::Statement::Update { table, assignments, from, selection, returning, or, limit } => {
            if from.is_some() || returning.is_some() || or.is_some() || limit.is_some() {
                return Err("unsupported UPDATE clause".into());
            }
            write::update(table, assignments, selection.as_ref())
        }
        ast::Statement::Delete(d) => write::delete(d),
        ast::Statement::Explain { describe_alias, analyze, verbose, query_plan, estimate,
            statement, format, options } => {
            if *describe_alias != ast::DescribeAlias::Explain || *verbose || *query_plan
                || *estimate || format.is_some() || options.is_some()
            { return Err("only EXPLAIN [ANALYZE] SELECT is supported".into()); }
            let ast::Statement::Query(q) = statement.as_ref() else {
                return Err("EXPLAIN requires a SELECT query".into());
            };
            Ok(Statement::Explain { query: query(q)?, analyze: *analyze })
        }
        _ => Err("unsupported statement".into()),
    }
}

pub(super) fn query_modifiers(q: &ast::Query) -> Result<()> {
    if q.with.is_some() || q.fetch.is_some() || !q.locks.is_empty()
        || q.for_clause.is_some() || q.settings.is_some() || q.format_clause.is_some()
        || !q.pipe_operators.is_empty()
    { return Err("unsupported query clause".into()); }
    Ok(())
}

fn query(q: &ast::Query) -> Result<Query> {
    query_modifiers(q)?;
    let ast::SetExpr::Select(select) = q.body.as_ref() else {
        return Err("only a single SELECT is supported".into());
    };
    if select.top.is_some() || select.exclude.is_some() || select.into.is_some()
        || !select.lateral_views.is_empty() || select.prewhere.is_some()
        || !select.cluster_by.is_empty() || !select.distribute_by.is_empty()
        || !select.sort_by.is_empty() || select.having.is_some() || !select.named_window.is_empty()
        || select.qualify.is_some() || select.value_table_mode.is_some()
        || select.connect_by.is_some() || select.flavor != ast::SelectFlavor::Standard
    { return Err("unsupported SELECT clause".into()); }
    let distinct = match select.distinct {
        None => false,
        Some(ast::Distinct::Distinct) => true,
        _ => return Err("DISTINCT ON is not supported".into()),
    };
    let [table] = select.from.as_slice() else {
        return Err("SELECT requires exactly one FROM source with optional INNER JOINs".into());
    };
    let from = relation(&table.relation)?;
    let mut joins = Vec::new();
    for join in &table.joins {
        if join.global { return Err("GLOBAL JOIN is not supported".into()); }
        let on = match &join.join_operator {
            ast::JoinOperator::Inner(ast::JoinConstraint::On(on))
            | ast::JoinOperator::Join(ast::JoinConstraint::On(on)) => expr::convert(on, false)?,
            _ => return Err("only INNER JOIN with ON is supported".into()),
        };
        expr::equijoin(&on)?;
        joins.push(Join { relation: relation(&join.relation)?, on });
    }
    unique_relations(&from, &joins)?;
    let group_by = match &select.group_by {
        ast::GroupByExpr::Expressions(expressions, modifiers) if modifiers.is_empty() => {
            expressions.iter().map(expr::group).collect::<Result<_>>()?
        }
        _ => return Err("unsupported GROUP BY clause".into()),
    };
    let order_by = match &q.order_by {
        None => Vec::new(),
        Some(ast::OrderBy { kind: ast::OrderByKind::Expressions(items), interpolate: None }) => {
            items.iter().map(expr::order).collect::<Result<_>>()?
        }
        _ => return Err("unsupported ORDER BY clause".into()),
    };
    let (limit, offset) = match &q.limit_clause {
        None => (None, 0),
        Some(ast::LimitClause::LimitOffset { limit, offset, limit_by }) if limit_by.is_empty() => {
            (limit.as_ref().map(unsigned).transpose()?,
                offset.as_ref().map(|o| unsigned(&o.value)).transpose()?.unwrap_or(0))
        }
        _ => return Err("only LIMIT n OFFSET n is supported".into()),
    };
    Ok(Query { from, joins, distinct, group_by, order_by, limit, offset,
        filter: select.selection.as_ref().map(|e| expr::convert(e, false)).transpose()?,
        projection: select.projection.iter().map(expr::projection).collect::<Result<_>>()? })
}

pub(crate) fn unique_relations(from: &Relation, joins: &[Join]) -> Result<()> {
    let mut names = std::collections::HashSet::new();
    names.insert(&from.alias);
    for join in joins {
        if !names.insert(&join.relation.alias) { return Err("duplicate relation alias".into()); }
    }
    Ok(())
}

pub(super) fn relation(table: &ast::TableFactor) -> Result<Relation> {
    let ast::TableFactor::Table { name, alias, args, with_hints, version, with_ordinality,
        partitions, json_path, sample, index_hints } = table else {
        return Err("only named tables are supported".into());
    };
    if args.is_some() || !with_hints.is_empty() || version.is_some() || *with_ordinality
        || !partitions.is_empty() || json_path.is_some() || sample.is_some() || !index_hints.is_empty()
    { return Err("unsupported table modifier".into()); }
    let table = expr::object(name)?;
    let alias = match alias {
        None => table.clone(),
        Some(alias) if alias.columns.is_empty() => expr::ident(&alias.name),
        _ => return Err("table column aliases are not supported".into()),
    };
    Ok(Relation { table, alias })
}

pub(crate) fn unsigned(expr: &ast::Expr) -> Result<usize> {
    if let ast::Expr::Value(value) = expr {
        if let ast::Value::Number(n, false) = &value.value {
            if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
                return n.parse().map_err(|_| "row count exceeds platform range".into());
            }
        }
    }
    Err("LIMIT and OFFSET require nonnegative integer literals".into())
}

pub(crate) fn fragment<T>(tokens: Vec<Token>, parse: impl FnOnce(&mut Parser<'_>) -> std::result::Result<T, sqlparser::parser::ParserError>) -> Result<T> {
    let mut parser = Parser::new(&GenericDialect).with_recursion_limit(256).with_tokens(tokens);
    let result = parse(&mut parser).map_err(|e| e.to_string())?;
    if parser.peek_token().token != Token::EOF {
        return Err(format!("unexpected token: {}", parser.peek_token().token));
    }
    Ok(result)
}
