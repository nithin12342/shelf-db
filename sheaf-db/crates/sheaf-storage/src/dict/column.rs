// dict/column.rs - a dictionary-encoded text column (MF-06).
//
// The column holds 32-bit offsets and nothing else. The bytes live in the
// shared interner, so two columns with the same value hold the same offset and
// the table holds one copy of the value.
//
// A read names the column and the dictionary. A position outside the column
// returns IndexOutOfRange. An offset whose arena entry is bad returns
// InvalidOffset through the dictionary, which is the only place that can
// detect it.

use super::{DictError, Interner};

/// A column of dictionary offsets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DictColumn {
    offsets: Vec<u32>,
}

impl DictColumn {
    /// An empty column.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty column with room for `capacity` values.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { offsets: Vec::with_capacity(capacity) }
    }

    /// Store one value through the dictionary and record its offset.
    pub fn push(&mut self, dict: &mut Interner, value: &[u8]) -> Result<u32, DictError> {
        let offset = dict.intern(value)?;
        self.offsets.push(offset);
        Ok(offset)
    }

    /// Append an offset that the caller already interned.
    pub fn push_offset(&mut self, offset: u32) {
        self.offsets.push(offset);
    }

    /// The number of values in the column.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// True when the column holds no value.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// The offsets in row order.
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    /// The value at `index`, read through the dictionary.
    pub fn get<'a>(&self, index: usize, dict: &'a Interner) -> Result<&'a [u8], DictError> {
        let offset = *self.offsets.get(index).ok_or(DictError::IndexOutOfRange(index))?;
        dict.resolve(offset)
    }

    /// Every value in row order.
    pub fn values(&self, dict: &Interner) -> Result<Vec<Vec<u8>>, DictError> {
        let mut out = Vec::with_capacity(self.offsets.len());
        for &offset in &self.offsets {
            out.push(dict.resolve(offset)?.to_vec());
        }
        Ok(out)
    }

    /// The number of distinct offsets. A column with one distinct offset per
    /// row stores every row once, which is the worst case.
    pub fn distinct(&self) -> usize {
        let mut seen: Vec<u32> = self.offsets.clone();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    /// True when the underlying bytes equal `other` through the same
    /// dictionary. This compares values, not offsets, so a column that was
    /// built in another order still matches.
    pub fn equals_values(&self, other: &DictColumn, dict: &Interner) -> Result<bool, DictError> {
        if self.offsets.len() != other.offsets.len() {
            return Ok(false);
        }
        for (a, b) in self.offsets.iter().zip(other.offsets.iter()) {
            if dict.resolve(*a)? != dict.resolve(*b)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_columns_share_one_offset_for_one_value() {
        let mut dict = Interner::new();
        let mut left = DictColumn::new();
        let mut right = DictColumn::new();
        let a = left.push(&mut dict, b"canada").expect("left");
        let b = right.push(&mut dict, b"canada").expect("right");
        assert_eq!(a, b);
        assert_eq!(dict.entries(), 1);
        assert_eq!(left.get(0, &dict).expect("read"), b"canada");
        assert_eq!(right.get(0, &dict).expect("read"), b"canada");
    }

    #[test]
    fn a_repeated_value_keeps_one_offset_per_row() {
        let mut dict = Interner::new();
        let mut column = DictColumn::new();
        for _ in 0..5 {
            column.push(&mut dict, b"chile").expect("push");
        }
        assert_eq!(column.len(), 5);
        assert_eq!(column.distinct(), 1);
        assert_eq!(dict.entries(), 1);
        assert_eq!(column.values(&dict).expect("values"), vec![b"chile".to_vec(); 5]);
    }

    #[test]
    fn a_position_past_the_column_is_reported() {
        let mut dict = Interner::new();
        let mut column = DictColumn::new();
        column.push(&mut dict, b"peru").expect("push");
        assert_eq!(column.get(1, &dict), Err(DictError::IndexOutOfRange(1)));
    }

    #[test]
    fn a_bad_offset_in_the_column_is_reported() {
        let dict = Interner::new();
        let mut column = DictColumn::new();
        column.push_offset(128);
        assert_eq!(column.get(0, &dict), Err(DictError::InvalidOffset(128)));
    }

    #[test]
    fn value_equality_ignores_the_build_order() {
        let mut dict = Interner::new();
        let mut left = DictColumn::new();
        let mut right = DictColumn::new();
        left.push(&mut dict, b"alpha").expect("alpha");
        left.push(&mut dict, b"beta").expect("beta");
        right.push(&mut dict, b"beta").expect("beta");
        right.push(&mut dict, b"alpha").expect("alpha");
        assert!(!left.equals_values(&right, &dict).expect("compare"));
        assert!(left.equals_values(&left.clone(), &dict).expect("compare"));
    }
}
