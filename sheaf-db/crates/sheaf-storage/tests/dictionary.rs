// dictionary.rs - integration test for the global dictionary arena (MF-06).
//
// The test drives the production path: DictColumn::push calls Interner::intern,
// which appends to the ByteArena, and DictColumn::get calls Interner::resolve,
// which reads the arena. Every case states its expected result.

use sheaf_storage::dict::{DictColumn, DictError, Interner, ALIGNMENT};

/// The number of intern calls in the large test.
const INSERTS: usize = 1_000_000;
/// The number of distinct values behind those calls.
const UNIQUE: usize = 10_000;

/// A value with a fixed length, so the arena layout is predictable.
fn value(index: usize) -> Vec<u8> {
    format!("value-{index:05}-{}", "x".repeat(14)).into_bytes()
}

#[test]
fn one_million_inserts_over_ten_thousand_values_keep_ten_thousand_entries() {
    let mut dict = Interner::new();
    let mut column = DictColumn::new();
    for row in 0..INSERTS {
        column.push(&mut dict, &value(row % UNIQUE)).expect("intern");
    }

    assert_eq!(column.len(), INSERTS, "every row keeps one offset");
    assert_eq!(dict.entries(), UNIQUE, "the arena keeps one entry per value");
    assert_eq!(dict.interned(), UNIQUE as u64, "only a new value is appended");
    assert_eq!(dict.reused(), (INSERTS - UNIQUE) as u64, "every repeat is a reuse");
    assert_eq!(column.distinct(), UNIQUE, "the column holds one offset per value");
    assert_eq!(dict.arena_bytes() % ALIGNMENT, 0, "the arena stays aligned");
    // Each entry is 4 prefix bytes plus 26 value bytes, padded to 64.
    assert_eq!(dict.arena_bytes(), UNIQUE * ALIGNMENT);

    // Decode the first and the last unique value through the column.
    for index in [0usize, 1, UNIQUE / 2, UNIQUE - 1] {
        assert_eq!(
            column.get(index, &dict).expect("decode"),
            value(index).as_slice(),
            "value {index} must decode"
        );
        assert_eq!(
            column.get(index + UNIQUE, &dict).expect("decode"),
            value(index).as_slice(),
            "the repeat of value {index} must decode"
        );
    }
}

#[test]
fn two_columns_with_the_same_value_share_one_offset() {
    let mut dict = Interner::new();
    let mut country = DictColumn::new();
    let mut capital = DictColumn::new();

    let a = country.push(&mut dict, b"france").expect("country");
    let b = capital.push(&mut dict, b"france").expect("capital");
    assert_eq!(a, b, "one value must have one offset");
    assert_eq!(dict.entries(), 1);
    assert_eq!(dict.arena_bytes(), ALIGNMENT);
}

#[test]
fn decode_returns_the_original_bytes_for_every_unique_value() {
    let mut dict = Interner::new();
    let mut column = DictColumn::new();
    let mut expected: Vec<Vec<u8>> = Vec::new();
    for index in 0..1000 {
        let text = value(index);
        column.push(&mut dict, &text).expect("push");
        expected.push(text);
    }
    assert_eq!(column.values(&dict).expect("decode all"), expected);
    assert_eq!(dict.values().expect("arena walk"), expected);
}

#[test]
fn a_corrupt_offset_returns_invalid_offset_and_never_a_truncated_value() {
    let mut dict = Interner::new();
    let mut column = DictColumn::new();
    column.push(&mut dict, &value(0)).expect("push");

    // A misaligned offset and an offset past the arena both fail. The arena
    // unit test covers the third case, a length prefix that runs past the end.
    column.push_offset(3);
    assert_eq!(column.get(1, &dict), Err(DictError::InvalidOffset(3)));

    column.push_offset(64);
    assert_eq!(column.get(2, &dict), Err(DictError::InvalidOffset(64)));
}

#[test]
fn an_empty_value_and_a_four_kib_value_round_trip() {
    let mut dict = Interner::new();
    let mut column = DictColumn::new();
    let empty: &[u8] = b"";
    let big = vec![0xab; 4096];
    column.push(&mut dict, empty).expect("empty");
    column.push(&mut dict, &big).expect("big");
    assert_eq!(column.get(0, &dict).expect("empty"), empty);
    assert_eq!(column.get(1, &dict).expect("big"), big.as_slice());
    assert_eq!(dict.arena_bytes(), 64 + 4096 + 4 + 60);
}
