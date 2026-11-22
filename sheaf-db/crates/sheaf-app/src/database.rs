use crate::model::*;
use crate::persistence::Journal;
use crate::storage::{Catalog, Snapshot, Table};
use sheaf_native::storage::StorageBackend;
use sheaf_runtime::EpochRegistry;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub struct Database {
    catalog: Catalog,
    backend: Box<dyn StorageBackend>,
    journal: Option<Journal>,
    epochs: EpochRegistry,
    /// Morphism declarations from the schema-guide DDL (NF-15).
    morphisms: BTreeMap<String, MorphismDecl>,
}

impl Database {
    pub fn memory() -> Self {
        Self {
            catalog: Catalog::new(),
            backend: sheaf_native::backends::storage(),
            journal: None,
            epochs: EpochRegistry::new(64),
            morphisms: BTreeMap::new(),
        }
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut database = Self::memory();
        let journal = Journal::open(path)?;
        journal.replay(|statement| {
            database.apply_replay(&statement)?;
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
            Statement::DefineMorphism(declaration) => {
                self.install_morphism(declaration.clone())?;
                if let Some(journal) = &mut self.journal {
                    journal.commit(statement)?;
                }
                self.epochs.advance();
                let width = declaration.matrix.first().map_or(0, Vec::len);
                Ok(QueryOutput {
                    columns: vec!["name".into(), "from".into(), "to".into()],
                    rows: vec![vec![
                        Value::Text(declaration.name.clone()),
                        Value::Text(declaration.from.clone()),
                        Value::Text(declaration.to.clone()),
                    ]],
                    affected: 1,
                    plan: vec![format!(
                        "Define morphism {} from {} to {}: {} projections, {}x{} matrix",
                        declaration.name,
                        declaration.from,
                        declaration.to,
                        declaration.projection.len(),
                        declaration.matrix.len(),
                        width
                    )],
                    ..Default::default()
                })
            }
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

    /// Apply a journaled statement during recovery. A morphism declaration is
    /// installed in the morphism map; every other statement goes through
    /// `prepare`, which rebuilds the catalog.
    fn apply_replay(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::DefineMorphism(declaration) => self.install_morphism(declaration.clone())?,
            other => {
                let (catalog, _) = self.prepare(other)?;
                self.catalog = catalog;
            }
        }
        self.epochs.advance();
        Ok(())
    }

    /// Validate and install a morphism. Both cells, every projected column,
    /// and the matrix shape are checked before the declaration is stored.
    fn install_morphism(&mut self, declaration: MorphismDecl) -> Result<()> {
        // SQL folds unquoted identifiers to lowercase. The DDL follows that
        // rule, so `FROM Account` finds a table created as `Account`.
        let mut declaration = declaration;
        declaration.from = declaration.from.to_lowercase();
        declaration.to = declaration.to.to_lowercase();
        for (source, target) in &mut declaration.projection {
            *source = source.to_lowercase();
            *target = target.to_lowercase();
        }
        if declaration.name.is_empty() {
            return Err("morphism name is empty".into());
        }
        if self.morphisms.contains_key(&declaration.name) {
            return Err(format!("morphism already exists: {}", declaration.name));
        }
        let source = self
            .catalog
            .get(&declaration.from)
            .ok_or_else(|| format!("unknown source cell {}", declaration.from))?;
        let target = self
            .catalog
            .get(&declaration.to)
            .ok_or_else(|| format!("unknown target cell {}", declaration.to))?;
        let mut seen_sources = HashSet::new();
        let mut seen_targets = HashSet::new();
        for (from_column, to_column) in &declaration.projection {
            let left = source
                .columns
                .iter()
                .find(|column| &column.name == from_column)
                .ok_or_else(|| format!("unknown source column {from_column}"))?;
            let right = target
                .columns
                .iter()
                .find(|column| &column.name == to_column)
                .ok_or_else(|| format!("unknown target column {to_column}"))?;
            if left.data_type != right.data_type {
                return Err(format!("morphism column types differ: {from_column} -> {to_column}"));
            }
            if !seen_sources.insert(from_column) || !seen_targets.insert(to_column) {
                return Err("morphism projection uses a column more than once".into());
            }
        }
        let width = declaration.matrix.first().map_or(0, Vec::len);
        if width == 0
            || declaration.matrix.iter().any(|row| {
                row.len() != width || row.iter().any(|value| !value.is_finite())
            })
        {
            return Err("morphism matrix requires a finite rectangular shape".into());
        }
        self.morphisms.insert(declaration.name.clone(), declaration);
        Ok(())
    }

    /// A stored morphism declaration, by name.
    pub fn morphism(&self, name: &str) -> Option<&MorphismDecl> {
        self.morphisms.get(name)
    }

    /// How many morphism declarations are stored.
    pub fn morphism_count(&self) -> usize {
        self.morphisms.len()
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
