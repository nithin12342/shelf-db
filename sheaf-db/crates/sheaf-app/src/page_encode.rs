// page_encode.rs - writes a version 2 page (MF-07).
//
// The writer turns a row set into the columnar frame from page_format.rs. Every
// column is encoded through sheaf-codec, so the selector picks the smallest
// codec per column and records its id in the header.
//
// A text column is stored as dictionary offsets, so one copy of a repeated
// string serves every row and every column of the table.
//
// The writer never guesses. A value that does not match its declared column
// type returns Shape, and a codec or dictionary failure is passed through.

use sheaf_codec::{select, ColumnData};
use sheaf_storage::dict::Interner;

use crate::model::{Column, DataType, Row, Value};
use crate::page_format::{
    bitmap_len, crc32c, row_fingerprint, set_null, ColumnDesc, PageError, PageHeader, PageType,
    COLUMN_DESC, FINGERPRINT, HEADER_FIXED, VERSION_V2,
};

/// Write one page. `columns` declares the type of every column, `rows` holds
/// the values in row order, and `dict` supplies the text offsets.
pub fn encode_page(
    columns: &[Column],
    rows: &[Row],
    dictionary_id: u32,
    dict: &mut Interner,
) -> Result<Vec<u8>, PageError> {
    if columns.is_empty() {
        return Err(PageError::Shape("a page needs at least one column"));
    }
    if columns.len() > u16::MAX as usize {
        return Err(PageError::Shape("a page holds at most 65535 columns"));
    }
    if rows.len() > u32::MAX as usize {
        return Err(PageError::Shape("a page holds at most 2^32-1 rows"));
    }
    for row in rows {
        if row.len() != columns.len() {
            return Err(PageError::Shape("a row width differs from the column count"));
        }
    }

    let mut body = Vec::with_capacity(rows.len() * FINGERPRINT + columns.len() * 32);
    for row in rows {
        body.extend_from_slice(&row_fingerprint(row).to_le_bytes());
    }

    let mut descriptors = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        descriptors.push(encode_column(column, index, rows, dict, &mut body)?);
    }

    let header = PageHeader {
        version: VERSION_V2,
        row_count: rows.len() as u32,
        dictionary_id,
        payload_len: body.len() as u32,
        crc32c: crc32c(&body),
        columns: descriptors,
    };
    let mut page = Vec::with_capacity(HEADER_FIXED + header.columns.len() * COLUMN_DESC + body.len());
    page.extend_from_slice(&header.to_bytes());
    page.extend_from_slice(&body);
    Ok(page)
}

/// Encode one column and append its bytes to `body`. Returns the descriptor that
/// names the page type and the codec id.
fn encode_column(
    column: &Column,
    index: usize,
    rows: &[Row],
    dict: &mut Interner,
    body: &mut Vec<u8>,
) -> Result<ColumnDesc, PageError> {
    let page_type = match column.data_type {
        DataType::Int => PageType::Int,
        DataType::Float => PageType::Float,
        DataType::Text => PageType::Text,
        DataType::Bool => PageType::Bool,
    };

    let mut bitmap = vec![0u8; bitmap_len(rows.len())];
    let mut ints: Vec<i64> = Vec::with_capacity(rows.len());
    let mut words: Vec<u64> = Vec::with_capacity(rows.len());

    for (row_index, row) in rows.iter().enumerate() {
        let value = &row[index];
        if let Value::Null = value {
            if !column.nullable {
                return Err(PageError::Shape("a null value is in a non-nullable column"));
            }
            set_null(&mut bitmap, row_index);
            continue;
        }
        match (page_type, value) {
            (PageType::Int, Value::Int(number)) => ints.push(*number),
            (PageType::Float, Value::Float(number)) => words.push(number.to_bits()),
            (PageType::Text, Value::Text(text)) => {
                words.push(dict.intern(text.as_bytes())? as u64);
            }
            (PageType::Bool, Value::Bool(flag)) => words.push(u64::from(*flag)),
            (kind, _) => {
                return Err(PageError::Shape(match kind {
                    PageType::Int => "an int column holds a non-int value",
                    PageType::Float => "a float column holds a non-float value",
                    PageType::Text => "a text column holds a non-text value",
                    PageType::Bool => "a bool column holds a non-bool value",
                }))
            }
        }
    }

    let choice = match page_type {
        PageType::Int => select(&ColumnData::I64(&ints))?,
        _ => select(&ColumnData::U64(&words))?,
    };

    body.extend_from_slice(&bitmap);
    body.extend_from_slice(&(choice.encoded.len() as u32).to_le_bytes());
    body.extend_from_slice(&choice.encoded);
    Ok(ColumnDesc { page_type, codec_id: choice.codec_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataType;

    fn columns() -> Vec<Column> {
        vec![
            Column { name: "id".into(), data_type: DataType::Int, nullable: false },
            Column { name: "price".into(), data_type: DataType::Float, nullable: true },
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
    fn a_page_starts_with_the_magic_and_the_version() {
        let mut dict = Interner::new();
        let page = encode_page(&columns(), &rows(), 1, &mut dict).expect("encode");
        assert_eq!(&page[..4], b"SHPG");
        assert_eq!(page[4], VERSION_V2);
        let (header, len) = PageHeader::parse(&page).expect("parse");
        assert_eq!(header.row_count, 2);
        assert_eq!(header.columns.len(), 4);
        assert_eq!(len + header.payload_len as usize, page.len());
        assert_eq!(header.crc32c, crc32c(&page[len..]));
    }

    #[test]
    fn a_wrong_value_type_is_rejected() {
        let mut dict = Interner::new();
        let bad = vec![vec![
            Value::Text("one".into()),
            Value::Float(1.0),
            Value::Text("alpha".into()),
            Value::Bool(true),
        ]];
        assert_eq!(
            encode_page(&columns(), &bad, 1, &mut dict).err(),
            Some(PageError::Shape("an int column holds a non-int value"))
        );
    }

    #[test]
    fn a_null_in_a_non_nullable_column_is_rejected() {
        let mut dict = Interner::new();
        let bad = vec![vec![
            Value::Null,
            Value::Float(1.0),
            Value::Text("alpha".into()),
            Value::Bool(true),
        ]];
        assert_eq!(
            encode_page(&columns(), &bad, 1, &mut dict).err(),
            Some(PageError::Shape("a null value is in a non-nullable column"))
        );
    }

    #[test]
    fn a_row_of_the_wrong_width_is_rejected() {
        let mut dict = Interner::new();
        let bad = vec![vec![Value::Int(1)]];
        assert_eq!(
            encode_page(&columns(), &bad, 1, &mut dict).err(),
            Some(PageError::Shape("a row width differs from the column count"))
        );
    }
}
