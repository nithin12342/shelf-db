// page.rs - integration test for the versioned page format (MF-07).
//
// The test drives the production path: encode_page writes a version 2 frame and
// decode_page reads it back through the CRC32C gate, the codec, and the row
// fingerprint gate. Every case states its expected result.

use sheaf_app::model::{Column, DataType, Row, Value};
use sheaf_app::{decode_page, encode_page, PageError, PageVersion};
use sheaf_storage::dict::Interner;

/// The columns of the test table. Every data type appears once.
fn columns() -> Vec<Column> {
    vec![
        Column { name: "id".into(), data_type: DataType::Int, nullable: false },
        Column { name: "price".into(), data_type: DataType::Float, nullable: true },
        Column { name: "name".into(), data_type: DataType::Text, nullable: false },
        Column { name: "note".into(), data_type: DataType::Text, nullable: true },
        Column { name: "live".into(), data_type: DataType::Bool, nullable: false },
    ]
}

/// Sixteen rows: an int, a nullable float, two text columns, and a bool.
fn rows() -> Vec<Row> {
    let names = ["alpha", "beta", "gamma", "delta"];
    (0..16usize)
        .map(|index| {
            vec![
                Value::Int(index as i64 * 3),
                if index % 3 == 0 { Value::Null } else { Value::Float(index as f64 / 4.0) },
                Value::Text(names[index % names.len()].into()),
                if index % 2 == 0 { Value::Null } else { Value::Text("shared note".into()) },
                Value::Bool(index % 2 == 0),
            ]
        })
        .collect()
}

#[test]
fn a_sixteen_row_page_with_every_type_round_trips() {
    let mut dict = Interner::new();
    let rows = rows();
    let page = encode_page(&columns(), &rows, 1, &mut dict).expect("encode");

    assert_eq!(&page[..4], b"SHPG");
    assert_eq!(page[4], 2, "a new page must be version 2");

    let decoded = decode_page(&page, &dict).expect("decode");
    assert_eq!(decoded.version, PageVersion::V2);
    assert_eq!(decoded.migrated, None);
    assert_eq!(decoded.rows, rows, "every row must be equal");
    assert_eq!(decoded.dictionary_id, 1);
    assert_ne!(decoded.crc32c, 0, "the reader must report the checked CRC");

    // Four distinct names and one shared note means five dictionary entries.
    assert_eq!(dict.entries(), 5, "the two text columns share one dictionary");
}

#[test]
fn an_unknown_version_byte_is_rejected() {
    let mut dict = Interner::new();
    let mut page = encode_page(&columns(), &rows(), 1, &mut dict).expect("encode");
    page[4] = 7;
    assert_eq!(
        decode_page(&page, &dict).err(),
        Some(PageError::UnsupportedPageVersion(7))
    );
}

#[test]
fn a_flipped_payload_bit_returns_a_checksum_mismatch() {
    let mut dict = Interner::new();
    let mut page = encode_page(&columns(), &rows(), 1, &mut dict).expect("encode");
    let last = page.len() - 1;
    page[last] ^= 0x80;
    match decode_page(&page, &dict) {
        Err(PageError::ChecksumMismatch { expected, actual }) => {
            assert_ne!(expected, actual, "the two checksums must differ");
        }
        other => panic!("expected a checksum mismatch, got {other:?}"),
    }
}

#[test]
fn a_version_one_fixture_reads_with_migrated_from_v1() {
    // The version 1 page shape is a JSON array of rows.
    let fixture = br#"[
        [{"Int":1},{"Float":1.5},{"Text":"alpha"},{"Bool":true}],
        [{"Int":2},"Null",{"Text":"beta"},{"Bool":false}]
    ]"#;
    let dict = Interner::new();
    let decoded = decode_page(fixture, &dict).expect("decode v1");
    assert_eq!(decoded.version, PageVersion::V1);
    let migrated = decoded.migrated.expect("a v1 page must report the migration");
    assert_eq!(migrated.source_version, 1);
    assert_eq!(migrated.rows, 2);
    assert_eq!(decoded.rows.len(), 2);
    assert_eq!(decoded.rows[0][2], Value::Text("alpha".into()));
}

#[test]
fn a_page_with_one_column_round_trips() {
    let mut dict = Interner::new();
    let columns = vec![Column { name: "n".into(), data_type: DataType::Int, nullable: false }];
    let rows: Vec<Row> = (0..16).map(|i| vec![Value::Int(i)]).collect();
    let page = encode_page(&columns, &rows, 0, &mut dict).expect("encode");
    let decoded = decode_page(&page, &dict).expect("decode");
    assert_eq!(decoded.rows, rows);
}

#[test]
fn an_empty_page_round_trips() {
    let mut dict = Interner::new();
    let page = encode_page(&columns(), &[], 0, &mut dict).expect("encode");
    let decoded = decode_page(&page, &dict).expect("decode");
    assert!(decoded.rows.is_empty());
}
