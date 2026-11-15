use super::{expr, query_modifiers, relation};
use crate::model::{Column, DataType, Result, Statement};
use sqlparser::ast::{self, helpers::stmt_create_table::CreateTableBuilder};
use std::collections::HashSet;

pub(super) fn create(c: &ast::CreateTable) -> Result<Statement> {
    // Compare every field with the supported shape, including future parser additions.
    let baseline = CreateTableBuilder::new(c.name.clone()).columns(c.columns.clone())
        .hive_formats(Some(ast::HiveFormat::default())).build();
    if baseline != ast::Statement::CreateTable(c.clone()) {
        return Err("unsupported CREATE TABLE modifier or constraint".into());
    }
    if c.columns.is_empty() { return Err("CREATE TABLE requires columns".into()); }
    let mut names = HashSet::new();
    let mut columns = Vec::new();
    for column in &c.columns {
        let name = expr::ident(&column.name);
        if !names.insert(name.clone()) { return Err("duplicate column name".into()); }
        let data_type = match &column.data_type {
            ast::DataType::Int(None) | ast::DataType::Integer(None) | ast::DataType::BigInt(None) => DataType::Int,
            ast::DataType::Double(ast::ExactNumberInfo::None)
            | ast::DataType::Float(ast::ExactNumberInfo::None) | ast::DataType::DoublePrecision => DataType::Float,
            ast::DataType::Text | ast::DataType::Varchar(None) => DataType::Text,
            ast::DataType::Boolean | ast::DataType::Bool => DataType::Bool,
            _ => return Err("unsupported column type or type modifier".into()),
        };
        let mut nullable = true;
        if column.options.len() > 1 { return Err("only one nullability clause is supported".into()); }
        for option in &column.options {
            if option.name.is_some() { return Err("named column constraints are not supported".into()); }
            nullable = match option.option {
                ast::ColumnOption::Null => true,
                ast::ColumnOption::NotNull => false,
                _ => return Err("only NULL and NOT NULL column clauses are supported".into()),
            };
        }
        columns.push(Column { name, data_type, nullable });
    }
    Ok(Statement::Create { table: expr::object(&c.name)?, columns })
}

pub(super) fn insert(i: &ast::Insert) -> Result<Statement> {
    if i.or.is_some() || i.ignore || i.table_alias.is_some() || i.overwrite
        || !i.assignments.is_empty() || i.partitioned.is_some() || !i.after_columns.is_empty()
        || i.has_table_keyword || i.on.is_some() || i.returning.is_some() || i.replace_into
        || i.priority.is_some() || i.insert_alias.is_some() || i.settings.is_some() || i.format_clause.is_some()
    { return Err("unsupported INSERT modifier".into()); }
    let ast::TableObject::TableName(table) = &i.table else {
        return Err("INSERT requires a named table".into());
    };
    let source = i.source.as_ref().ok_or("INSERT requires VALUES")?;
    query_modifiers(source)?;
    if source.order_by.is_some() || source.limit_clause.is_some() {
        return Err("VALUES cannot have ORDER BY, LIMIT, or OFFSET".into());
    }
    let ast::SetExpr::Values(values) = source.body.as_ref() else {
        return Err("INSERT supports only VALUES".into());
    };
    if values.explicit_row { return Err("VALUES ROW is not supported".into()); }
    let columns: Vec<_> = i.columns.iter().map(expr::ident).collect();
    if columns.iter().collect::<HashSet<_>>().len() != columns.len() {
        return Err("duplicate INSERT column".into());
    }
    let width = values.rows.first().map_or(0, Vec::len);
    if width == 0 || (!columns.is_empty() && columns.len() != width)
        || values.rows.iter().any(|r| r.len() != width)
    { return Err("VALUES rows and column list must have matching widths".into()); }
    let rows = values.rows.iter().map(|row| row.iter().map(expr::literal).collect::<Result<_>>())
        .collect::<Result<_>>()?;
    Ok(Statement::Insert { table: expr::object(table)?, columns, rows })
}

fn write_table(table: &ast::TableWithJoins) -> Result<String> {
    if !table.joins.is_empty() { return Err("write joins are not supported".into()); }
    if matches!(&table.relation, ast::TableFactor::Table { alias: Some(_), .. }) {
        return Err("write table aliases are not supported".into());
    }
    relation(&table.relation).map(|r| r.table)
}

pub(super) fn update(table: &ast::TableWithJoins, assignments: &[ast::Assignment], filter: Option<&ast::Expr>) -> Result<Statement> {
    let table = write_table(table)?;
    let mut names = HashSet::new();
    let assignments = assignments.iter().map(|assignment| {
        let ast::AssignmentTarget::ColumnName(name) = &assignment.target else {
            return Err("tuple assignments are not supported".into());
        };
        let name = expr::object(name)?;
        if !names.insert(name.clone()) { return Err("duplicate UPDATE assignment".into()); }
        Ok((name, expr::convert(&assignment.value, false)?))
    }).collect::<Result<Vec<_>>>()?;
    if assignments.is_empty() { return Err("UPDATE requires assignments".into()); }
    Ok(Statement::Update { table, assignments,
        filter: filter.map(|f| expr::convert(f, false)).transpose()? })
}

pub(super) fn delete(d: &ast::Delete) -> Result<Statement> {
    if !d.tables.is_empty() || d.using.is_some() || d.returning.is_some()
        || !d.order_by.is_empty() || d.limit.is_some()
    { return Err("unsupported DELETE clause".into()); }
    let ast::FromTable::WithFromKeyword(tables) = &d.from else {
        return Err("DELETE requires FROM".into());
    };
    let [table] = tables.as_slice() else { return Err("DELETE requires one table".into()); };
    Ok(Statement::Delete { table: write_table(table)?,
        filter: d.selection.as_ref().map(|f| expr::convert(f, false)).transpose()? })
}
