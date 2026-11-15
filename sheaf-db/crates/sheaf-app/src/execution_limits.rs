use crate::model::{Result, Row, Value, MAX_QUERY_ROWS, MEMORY_LIMIT};

// Conservative charges include vector growth, hash table slack, and sort scratch.
pub const HASH_ENTRY: usize = 160;
pub const INDEX_ENTRY: usize = 16;

#[derive(Default)]
pub struct Budget {
    used: usize,
}

impl Budget {
    pub fn reserve(&mut self, bytes: usize) -> Result<()> {
        let used = self.used.checked_add(bytes).ok_or("Query memory size overflow")?;
        if used > MEMORY_LIMIT {
            return Err(format!("Query exceeds memory limit of {MEMORY_LIMIT} bytes"));
        }
        self.used = used;
        Ok(())
    }
    pub fn release(&mut self, bytes: usize) {
        self.used = self.used.saturating_sub(bytes);
    }
}

pub fn row_limit(count: usize) -> Result<()> {
    if count >= MAX_QUERY_ROWS {
        return Err(format!("Query exceeds row/candidate limit of {MAX_QUERY_ROWS}"));
    }
    Ok(())
}

pub fn value_bytes(value: &Value) -> usize {
    std::mem::size_of::<Value>()
        + match value {
            Value::Text(text) => text.len(),
            _ => 0,
        }
}

pub fn row_bytes(row: &Row) -> Result<usize> {
    row.iter().try_fold(std::mem::size_of::<Row>(), |total, value| {
        total.checked_add(value_bytes(value)).ok_or_else(|| "Row memory size overflow".into())
    })
}

pub fn copy_row(row: &Row, budget: &mut Budget) -> Result<Row> {
    budget.reserve(row_bytes(row)?.checked_mul(2).ok_or("Row memory size overflow")?)?;
    Ok(row.clone())
}

// DataSource owns materialization. Its returned allocation is inspected before reuse.
pub fn source_bytes(rows: &Vec<Row>) -> Result<usize> {
    if rows.len() > MAX_QUERY_ROWS {
        return Err("Source exceeds query row limit".into());
    }
    let mut total = rows.capacity().checked_mul(std::mem::size_of::<Row>()).ok_or("Source size overflow")?;
    for row in rows {
        total = total
            .checked_add(row.capacity().checked_mul(std::mem::size_of::<Value>()).ok_or("Source size overflow")?)
            .ok_or("Source size overflow")?;
        for value in row {
            if let Value::Text(text) = value {
                total = total.checked_add(text.capacity()).ok_or("Source size overflow")?;
            }
        }
    }
    Ok(total)
}
