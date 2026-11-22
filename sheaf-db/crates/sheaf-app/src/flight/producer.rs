// Timestamp: 2026-11-22_11-53-08 +05:30
// flight/producer.rs - the row store behind the Flight service (NF-09).

use std::sync::{Arc, Mutex};

/// Rows shared between do_put and do_get.
#[derive(Clone, Default)]
pub struct RowStore {
    rows: Arc<Mutex<Vec<String>>>,
}

impl RowStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all rows. The ticket then returns this exact result set.
    pub fn set(&self, rows: Vec<String>) {
        *self.rows.lock().expect("row store lock") = rows;
    }

    /// Append rows from a do_put batch.
    pub fn extend(&self, rows: impl IntoIterator<Item = String>) {
        self.rows.lock().expect("row store lock").extend(rows);
    }

    /// A snapshot of the rows in insertion order.
    pub fn snapshot(&self) -> Vec<String> {
        self.rows.lock().expect("row store lock").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_store_is_shared_between_handles() {
        let store = RowStore::new();
        let clone = store.clone();
        store.set(vec!["a".into(), "b".into()]);
        clone.extend(["c".into()]);
        assert_eq!(store.snapshot(), vec!["a", "b", "c"]);
    }
}
