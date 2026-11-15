use crate::model::*;
use crate::persistence::Journal;
use crate::storage::{Catalog, Snapshot, Table};
use sheaf_native::storage::StorageBackend;
use sheaf_runtime::EpochRegistry;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

pub struct Database {
    catalog: Catalog,
    backend: Box<dyn StorageBackend>,
    journal: Option<Journal>,
    epochs: EpochRegistry,
}

impl Database {
    pub fn memory() -> Self {
        Self {
            catalog: Catalog::new(),
            backend: sheaf_native::backends::storage(),
            journal: None,
            epochs: EpochRegistry::new(64),
        }
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut database = Self::memory();
        let journal = Journal::open(path)?;
        journal.replay(|statement| {
            let (catalog, _) = database.prepare(&statement)?;
            database.catalog = catalog;
            database.epochs.advance();
            Ok(())
        })?;
        database.journal = Some(journal);
        Ok(database)
    }

    pub fn backends(&self) -> Vec<String> {
        vec![
            format!("storage={}", self.backend.name()),
            format!(
                "wal={}",
                self.journal.as_ref().map_or("memory-only", Journal::name)
            ),
            format!(
                "kernel={}",
                if sheaf_native::native_kernel_linked() {
                    "cpp23"
                } else {
                    "rust-fallback"
                }
            ),
        ]
    }

    pub fn epoch(&self) -> u64 {
        self.journal
            .as_ref()
            .map_or(self.epochs.current(), Journal::epoch)
    }

    pub fn run_sql(&mut self, source: &str, optimize: bool) -> Result<Vec<QueryOutput>> {
        let statements = crate::sql::parse(source)?;
        let mut results = Vec::new();
        let mut bytes = 0usize;
        for statement in &statements {
            let result = self.execute(statement, optimize)?;
            bytes += result.rows.iter().map(|row| {
                std::mem::size_of::<Row>() + row.iter().map(Value::bytes).sum::<usize>()
            }).sum::<usize>();
            bytes += result.columns.iter().chain(&result.plan).map(String::len).sum::<usize>()
                + result.tql.as_ref().map_or(0, String::len);
            if bytes > MEMORY_LIMIT { return Err("script result memory limit exceeded".into()); }
            results.push(result);
        }
        Ok(results)
    }

    pub fn run_tql(&mut self, source: &str, optimize: bool) -> Result<Vec<QueryOutput>> {
        if source.len() > MAX_REQUEST_BYTES {
            return Err("request exceeds 65536 bytes".into());
        }
        let tokens = crate::sql::tokens(source)?;
        if tokens.first().is_some_and(|t| crate::sql::keyword(t, "MATCH"))
            && tokens.get(1).is_some_and(|t| crate::sql::keyword(t, "ROWS"))
        {
            let query = crate::tql::parse(source)?;
            Ok(vec![self.query(&query, optimize)?])
        } else {
            Ok(vec![crate::topological::execute(source)?])
        }
    }

    pub fn query(&self, query: &Query, optimize: bool) -> Result<QueryOutput> {
        let _lease = self.epochs.pin().ok_or("reader slots exhausted")?;
        let source = Snapshot {
            catalog: &self.catalog,
            backend: self.backend.as_ref(),
        };
        // Execution consumes the parsed translation, not the original SQL tree.
        let text = crate::tql::render(query);
        let translated = crate::tql::parse(&text)?;
        if &translated != query {
            return Err("SQL/TQL translation did not preserve the query".into());
        }
        let mut output = crate::executor::execute(&source, &translated, optimize)?;
        output.tql = Some(text);
        Ok(output)
    }

    pub fn execute(&mut self, statement: &Statement, optimize: bool) -> Result<QueryOutput> {
        match statement {
            Statement::Query(query) => self.query(query, optimize),
            Statement::Explain { query, analyze } => {
                if *analyze {
                    return self.query(query, optimize);
                }
                let source = Snapshot {
                    catalog: &self.catalog,
                    backend: self.backend.as_ref(),
                };
                Ok(QueryOutput {
                    tql: Some(crate::tql::render(query)),
                    plan: crate::executor::explain(&source, query, optimize)?,
                    ..Default::default()
                })
            }
            _ => {
                let (prepared, affected) = self.prepare(statement)?;
                if let Some(journal) = &mut self.journal {
                    journal.commit(statement)?;
                }
                self.catalog = prepared;
                self.epochs.advance();
                Ok(QueryOutput {
                    affected,
                    ..Default::default()
                })
            }
        }
    }

    fn prepare(&self, statement: &Statement) -> Result<(Catalog, usize)> {
        let mut catalog = self.catalog.clone();
        let affected = match statement {
            Statement::Create { table, columns } => {
                if table.is_empty() || catalog.contains_key(table) {
                    return Err(format!("table already exists or is invalid: {table}"));
                }
                if catalog.len() >= 1024 {
                    return Err("catalog table limit exceeded".into());
                }
                let value = Table::build(columns.clone(), &[], self.backend.as_ref())?;
                catalog.insert(table.clone(), Arc::new(value));
                0
            }
            Statement::Insert {
                table,
                columns,
                rows,
            } => {
                let previous = catalog
                    .get(table)
                    .ok_or_else(|| format!("unknown table {table}"))?;
                let supplied: Vec<String> = if columns.is_empty() {
                    previous.columns.iter().map(|c| c.name.clone()).collect()
                } else {
                    columns.clone()
                };
                let mut seen = HashSet::new();
                let indices: Vec<usize> = supplied
                    .iter()
                    .map(|name| {
                        if !seen.insert(name) {
                            return Err(format!("duplicate insert column {name}"));
                        }
                        previous
                            .columns
                            .iter()
                            .position(|c| &c.name == name)
                            .ok_or_else(|| format!("unknown column {name}"))
                    })
                    .collect::<Result<_>>()?;
                if previous.count + rows.len() > MAX_ROWS {
                    return Err("table row limit exceeded".into());
                }
                let mut combined = previous.rows(self.backend.as_ref())?;
                for row in rows {
                    if row.len() != supplied.len() {
                        return Err("insert column count mismatch".into());
                    }
                    let mut full = vec![Value::Null; previous.columns.len()];
                    for (&index, value) in indices.iter().zip(row) {
                        full[index] = value.clone();
                    }
                    combined.push(full);
                }
                let value =
                    Table::build(previous.columns.clone(), &combined, self.backend.as_ref())?;
                catalog.insert(table.clone(), Arc::new(value));
                rows.len()
            }
            Statement::Update {
                table,
                assignments,
                filter,
            } => {
                let previous = catalog
                    .get(table)
                    .ok_or_else(|| format!("unknown table {table}"))?;
                let names: Vec<String> = previous
                    .columns
                    .iter()
                    .map(|c| format!("{table}.{}", c.name))
                    .collect();
                if let Some(expr) = filter {
                    crate::eval::validate(expr, &names)?;
                }
                let mut seen = HashSet::new();
                let mut updates = Vec::new();
                for (name, expr) in assignments {
                    if !seen.insert(name) {
                        return Err(format!("duplicate assignment {name}"));
                    }
                    let index = previous
                        .columns
                        .iter()
                        .position(|c| &c.name == name)
                        .ok_or_else(|| format!("unknown column {name}"))?;
                    crate::eval::validate(expr, &names)?;
                    updates.push((index, expr));
                }
                let mut rows = previous.rows(self.backend.as_ref())?;
                let mut affected = 0;
                for row in &mut rows {
                    if matches_filter(filter, &names, row)? {
                        let changes: Vec<_> = updates
                            .iter()
                            .map(|(index, expr)| {
                                crate::eval::eval(expr, &names, row).map(|value| (*index, value))
                            })
                            .collect::<Result<_>>()?;
                        for (index, value) in changes {
                            row[index] = value;
                        }
                        affected += 1;
                    }
                }
                let value = Table::build(previous.columns.clone(), &rows, self.backend.as_ref())?;
                catalog.insert(table.clone(), Arc::new(value));
                affected
            }
            Statement::Delete { table, filter } => {
                let previous = catalog
                    .get(table)
                    .ok_or_else(|| format!("unknown table {table}"))?;
                let names: Vec<String> = previous
                    .columns
                    .iter()
                    .map(|c| format!("{table}.{}", c.name))
                    .collect();
                if let Some(expr) = filter {
                    crate::eval::validate(expr, &names)?;
                }
                let rows = previous.rows(self.backend.as_ref())?;
                let mut kept = Vec::new();
                for row in rows {
                    if !matches_filter(filter, &names, &row)? {
                        kept.push(row);
                    }
                }
                let affected = previous.count - kept.len();
                let value = Table::build(previous.columns.clone(), &kept, self.backend.as_ref())?;
                catalog.insert(table.clone(), Arc::new(value));
                affected
            }
            _ => return Err("a query cannot appear in the mutation journal".into()),
        };
        if catalog.values().map(|t| t.bytes).sum::<usize>() > MEMORY_LIMIT
            || catalog.values().map(|t| t.count).sum::<usize>() > MAX_ROWS
        {
            return Err("database memory or row limit exceeded".into());
        }
        Ok((catalog, affected))
    }
}

fn matches_filter(filter: &Option<Expr>, columns: &[String], row: &Row) -> Result<bool> {
    match filter {
        Some(expr) => crate::eval::truth(crate::eval::eval(expr, columns, row)?),
        None => Ok(true),
    }
}
