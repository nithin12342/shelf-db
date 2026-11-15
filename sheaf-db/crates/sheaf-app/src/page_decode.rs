// page_decode.rs - reads a version 1 or a version 2 page (MF-07).
//
// Two gates run before a row set is returned:
//
//   1. The CRC32C in the header must match the body. This runs before any
//      decode, so a damaged page never reaches the codecs.
//   2. The fingerprint of every decoded row must match the fingerprint in the
//      body. This is the stalk hash check.
//
// A version 1 page is a JSON array of rows, with or without the SHPG frame. It
// carries no checksum, so the reader reports the migration and no CRC.
//
// An unknown version returns UnsupportedPageVersion. The reader never guesses a
// layout.

use sheaf_codec::{decode, DecodedColumn};
use sheaf_storage::dict::Interner;

use crate::model::{Row, Value};
use crate::page_format::{
    bitmap_len, crc32c, is_null, row_fingerprint, DecodedPage, MigratedFromV1, PageError,
    PageHeader, PageType, PageVersion, FINGERPRINT, HEADER_FIXED, MAGIC, VERSION_V1, VERSION_V2,
};

/// Decode a page of any supported version.
pub fn decode_page(bytes: &[u8], dict: &Interner) -> Result<DecodedPage, PageError> {
    if bytes.len() >= 4 && bytes[..4] == MAGIC {
        let version = *bytes.get(4).ok_or(PageError::Truncated("version"))?;
        return match version {
            VERSION_V2 => {
                let (header, header_len) = PageHeader::parse(bytes)?;
                decode_v2(bytes, header, header_len, dict)
            }
            VERSION_V1 => decode_v1_frame(bytes),
            other => Err(PageError::UnsupportedPageVersion(other)),
        };
    }
    decode_v1_json(bytes, 0, false)
}

/// Decode a version 2 page.
fn decode_v2(
    bytes: &[u8],
    header: PageHeader,
    header_len: usize,
    dict: &Interner,
) -> Result<DecodedPage, PageError> {
    let body = bytes.get(header_len..).ok_or(PageError::Truncated("body"))?;
    if body.len() != header.payload_len as usize {
        return Err(PageError::Truncated("payload length"));
    }
    let actual = crc32c(body);
    if actual != header.crc32c {
        return Err(PageError::ChecksumMismatch { expected: header.crc32c, actual });
    }

    let rows = header.row_count as usize;
    let fingerprints_len = rows * FINGERPRINT;
    if body.len() < fingerprints_len {
        return Err(PageError::Truncated("row fingerprints"));
    }
    let fingerprints: Vec<u64> = body[..fingerprints_len]
        .chunks_exact(FINGERPRINT)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    let mut at = fingerprints_len;

    let mut columns: Vec<Vec<Option<Value>>> = Vec::with_capacity(header.columns.len());
    for descriptor in &header.columns {
        let bitmap_end = at + bitmap_len(rows);
        let bitmap = body.get(at..bitmap_end).ok_or(PageError::Truncated("null bitmap"))?;
        at = bitmap_end;
        let length = u32::from_le_bytes(
            body.get(at..at + 4)
                .ok_or(PageError::Truncated("block length"))?
                .try_into()
                .unwrap(),
        ) as usize;
        at += 4;
        let block = body.get(at..at + length).ok_or(PageError::Truncated("column block"))?;
        at += length;
        let decoded = decode(descriptor.codec_id, block)?;
        columns.push(expand_column(descriptor.page_type, decoded, bitmap, rows, dict)?);
    }
    if at != body.len() {
        return Err(PageError::Shape("the body holds trailing bytes"));
    }

    let mut out: Vec<Row> = Vec::with_capacity(rows);
    for row_index in 0..rows {
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            row.push(column[row_index].clone().unwrap_or(Value::Null));
        }
        out.push(row);
    }

    for (row_index, row) in out.iter().enumerate() {
        if row_fingerprint(row) != fingerprints[row_index] {
            return Err(PageError::RowFingerprintMismatch(row_index));
        }
    }

    Ok(DecodedPage {
        rows: out,
        version: PageVersion::V2,
        migrated: None,
        crc32c: actual,
        dictionary_id: header.dictionary_id,
    })
}

/// Expand one decoded codec block into one optional value per row.
fn expand_column(
    page_type: PageType,
    decoded: DecodedColumn,
    bitmap: &[u8],
    rows: usize,
    dict: &Interner,
) -> Result<Vec<Option<Value>>, PageError> {
    let values: Vec<Option<Value>> = match (page_type, decoded) {
        (PageType::Int, DecodedColumn::I64(numbers)) => {
            numbers.into_iter().map(|n| Some(Value::Int(n))).collect()
        }
        (PageType::Float, DecodedColumn::U64(words)) => words
            .into_iter()
            .map(|word| Some(Value::Float(f64::from_bits(word))))
            .collect(),
        (PageType::Bool, DecodedColumn::U64(words)) => words
            .into_iter()
            .map(|word| -> Result<Option<Value>, PageError> {
                match word {
                    0 => Ok(Some(Value::Bool(false))),
                    1 => Ok(Some(Value::Bool(true))),
                    _ => Err(PageError::Shape("a bool column holds a value above one")),
                }
            })
            .collect::<Result<_, _>>()?,
        (PageType::Text, DecodedColumn::U64(offsets)) => offsets
            .into_iter()
            .map(|offset| -> Result<Option<Value>, PageError> {
                let offset = u32::try_from(offset)
                    .map_err(|_| PageError::Shape("a text offset does not fit in 32 bits"))?;
                let bytes = dict.resolve(offset)?;
                let text = String::from_utf8(bytes.to_vec())
                    .map_err(|_| PageError::Shape("a dictionary value is not UTF-8"))?;
                Ok(Some(Value::Text(text)))
            })
            .collect::<Result<_, _>>()?,
        (kind, _) => {
            return Err(PageError::Shape(match kind {
                PageType::Int => "an int column block is not an i64 block",
                PageType::Float => "a float column block is not a u64 block",
                PageType::Text => "a text column block is not a u64 block",
                PageType::Bool => "a bool column block is not a u64 block",
            }));
        }
    };

    let present = (0..rows).filter(|row| !is_null(bitmap, *row)).count();
    if present != values.len() {
        return Err(PageError::Shape("the null bitmap does not match the block length"));
    }
    let mut iter = values.into_iter();
    Ok((0..rows)
        .map(|row| if is_null(bitmap, row) { None } else { iter.next().flatten() })
        .collect())
}

/// Decode a version 1 page inside the SHPG frame.
fn decode_v1_frame(bytes: &[u8]) -> Result<DecodedPage, PageError> {
    if bytes.len() < HEADER_FIXED {
        return Err(PageError::Truncated("header"));
    }
    let columns = u16::from_le_bytes(bytes[5..7].try_into().unwrap());
    if columns != 0 {
        return Err(PageError::Shape("a version 1 frame carries no column descriptions"));
    }
    let payload_len = u32::from_le_bytes(bytes[15..19].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(bytes[19..23].try_into().unwrap());
    let dictionary_id = u32::from_le_bytes(bytes[11..15].try_into().unwrap());
    let body = bytes.get(HEADER_FIXED..).ok_or(PageError::Truncated("body"))?;
    if body.len() != payload_len {
        return Err(PageError::Truncated("payload length"));
    }
    let actual = crc32c(body);
    if actual != crc {
        return Err(PageError::ChecksumMismatch { expected: crc, actual });
    }
    decode_v1_json(body, dictionary_id, true)
}

/// Decode a version 1 page: a JSON array of rows.
fn decode_v1_json(bytes: &[u8], dictionary_id: u32, framed: bool) -> Result<DecodedPage, PageError> {
    let rows: Vec<Row> =
        serde_json::from_slice(bytes).map_err(|e| PageError::BadV1Json(e.to_string()))?;
    Ok(DecodedPage {
        version: PageVersion::V1,
        migrated: Some(MigratedFromV1 {
            source_version: VERSION_V1,
            rows: rows.len(),
            framed,
        }),
        rows,
        crc32c: 0,
        dictionary_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Column, DataType};
    use crate::page_encode::encode_page;

    fn columns() -> Vec<Column> {
        vec![
            Column { name: "id".into(), data_type: DataType::Int, nullable: false },
            Column { name: "score".into(), data_type: DataType::Float, nullable: true },
            Column { name: "name".into(), data_type: DataType::Text, nullable: false },
            Column { name: "live".into(), data_type: DataType::Bool, nullable: false },
        ]
    }

    fn rows() -> Vec<Row> {
        vec![
            vec![Value::Int(1), Value::Float(1.5), Value::Text("alpha".into()), Value::Bool(true)],
            vec![Value::Int(2), Value::Null, Value::Text("beta".into()), Value::Bool(false)],
        ]
    }

    #[test]
    fn a_version_two_page_round_trips_every_type() {
        let mut dict = Interner::new();
        let page = encode_page(&columns(), &rows(), 9, &mut dict).expect("encode");
        let decoded = decode_page(&page, &dict).expect("decode");
        assert_eq!(decoded.version, PageVersion::V2);
        assert_eq!(decoded.migrated, None);
        assert_eq!(decoded.rows, rows());
        assert_eq!(decoded.dictionary_id, 9);
        assert_ne!(decoded.crc32c, 0);
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let mut dict = Interner::new();
        let mut page = encode_page(&columns(), &rows(), 9, &mut dict).expect("encode");
        page[4] = 9;
        assert_eq!(
            decode_page(&page, &dict).err(),
            Some(PageError::UnsupportedPageVersion(9))
        );
    }

    #[test]
    fn a_flipped_payload_bit_returns_a_checksum_mismatch() {
        let mut dict = Interner::new();
        let mut page = encode_page(&columns(), &rows(), 9, &mut dict).expect("encode");
        let last = page.len() - 1;
        page[last] ^= 0x01;
        match decode_page(&page, &dict) {
            Err(PageError::ChecksumMismatch { expected, actual }) => assert_ne!(expected, actual),
            other => panic!("expected a checksum mismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_version_one_fixture_is_read_and_reported_as_migrated() {
        let fixture = br#"[[{"Int":1},{"Text":"alpha"}],[{"Int":2},{"Text":"beta"}]]"#;
        let dict = Interner::new();
        let decoded = decode_page(fixture, &dict).expect("decode v1");
        assert_eq!(decoded.version, PageVersion::V1);
        assert_eq!(decoded.rows.len(), 2);
        assert_eq!(
            decoded.migrated,
            Some(MigratedFromV1 { source_version: 1, rows: 2, framed: false })
        );
    }

    #[test]
    fn a_bad_version_one_payload_is_rejected() {
        let dict = Interner::new();
        assert!(matches!(
            decode_page(b"not json", &dict),
            Err(PageError::BadV1Json(_))
        ));
    }

    #[test]
    fn a_row_fingerprint_mismatch_is_reported() {
        let mut dict = Interner::new();
        let mut page = encode_page(&columns(), &rows(), 9, &mut dict).expect("encode");
        let header_len = HEADER_FIXED + 4 * 2;
        // Change the first fingerprint, then repair the CRC so the CRC gate
        // passes and the fingerprint gate is the one that fires.
        page[header_len] ^= 0xff;
        let payload_len = u32::from_le_bytes(page[15..19].try_into().unwrap()) as usize;
        let crc = crc32c(&page[header_len..header_len + payload_len]);
        page[19..23].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(
            decode_page(&page, &dict).err(),
            Some(PageError::RowFingerprintMismatch(0))
        );
    }
}
