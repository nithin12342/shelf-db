// page_format.rs - the versioned page format (MF-07, spec Pillar 5).
//
// A page is one self-describing frame:
//
//   magic "SHPG" | version u8 | column count u16 | row count u32
//   dictionary id u32 | payload length u32 | CRC32C u32
//   per column: page type u8 | codec id u8
//   body
//
// The body holds a fingerprint per row and then one block per column:
//
//   row fingerprints: row count * 8 bytes
//   per column: null bitmap | block length u32 | encoded block
//
// Two rules keep the format honest. The CRC32C is checked before any decode,
// and the row fingerprint is checked after the decode. A page that fails either
// check returns an error, never a partial row set.
//
// Version 2 is the columnar layout above. Version 1 is a JSON array of rows,
// which is the shape that the earlier writer produced. A version 1 page is read
// and reported as migrated, so the caller can record the migration.

use std::fmt;

use sheaf_codec::CodecError;
use sheaf_storage::dict::DictError;

use crate::model::{Row, Value};

/// The four magic bytes at the start of a version 2 page.
pub const MAGIC: [u8; 4] = *b"SHPG";
/// The version of the JSON page shape.
pub const VERSION_V1: u8 = 1;
/// The version of the columnar page shape.
pub const VERSION_V2: u8 = 2;
/// The fixed header bytes before the column descriptions.
pub const HEADER_FIXED: usize = 23;
/// The bytes of one column description.
pub const COLUMN_DESC: usize = 2;
/// The bytes of one row fingerprint.
pub const FINGERPRINT: usize = 8;

/// Why a page operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageError {
    /// The frame does not start with the magic bytes, but the caller asked for
    /// a version 2 page.
    BadMagic,
    /// The version byte is not a version that this reader knows.
    UnsupportedPageVersion(u8),
    /// The CRC32C in the header does not match the body.
    ChecksumMismatch { expected: u32, actual: u32 },
    /// A field runs past the end of the buffer.
    Truncated(&'static str),
    /// A row value does not match its declared column type.
    Shape(&'static str),
    /// The row fingerprint does not match the decoded row.
    RowFingerprintMismatch(usize),
    /// A column codec failed.
    Codec(CodecError),
    /// A dictionary read failed.
    Dictionary(DictError),
    /// A version 1 payload is not a JSON array of rows.
    BadV1Json(String),
}

impl fmt::Display for PageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageError::BadMagic => write!(f, "the page does not start with the SHPG magic"),
            PageError::UnsupportedPageVersion(version) => {
                write!(f, "page version {version} is not supported")
            }
            PageError::ChecksumMismatch { expected, actual } => {
                write!(f, "page checksum is {actual:#010x}, header says {expected:#010x}")
            }
            PageError::Truncated(what) => write!(f, "page is truncated at {what}"),
            PageError::Shape(what) => write!(f, "page shape error: {what}"),
            PageError::RowFingerprintMismatch(row) => {
                write!(f, "row {row} does not match its fingerprint")
            }
            PageError::Codec(err) => write!(f, "page codec error: {err}"),
            PageError::Dictionary(err) => write!(f, "page dictionary error: {err}"),
            PageError::BadV1Json(detail) => write!(f, "version 1 page is not a row array: {detail}"),
        }
    }
}

impl std::error::Error for PageError {}

impl From<CodecError> for PageError {
    fn from(err: CodecError) -> Self {
        PageError::Codec(err)
    }
}

impl From<DictError> for PageError {
    fn from(err: DictError) -> Self {
        PageError::Dictionary(err)
    }
}

/// The page type of one column. It names the meaning of the codec block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageType {
    /// An i64 column.
    Int,
    /// An f64 column, stored as its 64-bit pattern.
    Float,
    /// A UTF-8 text column, stored as dictionary offsets.
    Text,
    /// A boolean column, stored as 0 or 1.
    Bool,
}

impl PageType {
    /// The one-byte tag in the header.
    pub fn tag(self) -> u8 {
        match self {
            PageType::Int => 0,
            PageType::Float => 1,
            PageType::Text => 2,
            PageType::Bool => 3,
        }
    }

    /// Read a tag. An unknown tag is a shape error, never a guess.
    pub fn from_tag(tag: u8) -> Result<Self, PageError> {
        match tag {
            0 => Ok(PageType::Int),
            1 => Ok(PageType::Float),
            2 => Ok(PageType::Text),
            3 => Ok(PageType::Bool),
            _other => Err(PageError::Shape("unknown page type tag")),
        }
    }
}

/// One column description in the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnDesc {
    /// The meaning of the block.
    pub page_type: PageType,
    /// The id of the codec that wrote the block.
    pub codec_id: u8,
}

/// The page header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageHeader {
    /// The frame version.
    pub version: u8,
    /// The number of rows.
    pub row_count: u32,
    /// The dictionary that the text offsets address. Zero means no dictionary.
    pub dictionary_id: u32,
    /// The body length in bytes.
    pub payload_len: u32,
    /// The CRC32C over the body.
    pub crc32c: u32,
    /// One description per column.
    pub columns: Vec<ColumnDesc>,
}

impl PageHeader {
    /// The header length in bytes for this column count.
    pub fn len(&self) -> usize {
        HEADER_FIXED + self.columns.len() * COLUMN_DESC
    }

    /// A page header always has a fixed part, so it is never empty.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Write the header bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len());
        out.extend_from_slice(&MAGIC);
        out.push(self.version);
        out.extend_from_slice(&(self.columns.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.row_count.to_le_bytes());
        out.extend_from_slice(&self.dictionary_id.to_le_bytes());
        out.extend_from_slice(&self.payload_len.to_le_bytes());
        out.extend_from_slice(&self.crc32c.to_le_bytes());
        for column in &self.columns {
            out.push(column.page_type.tag());
            out.push(column.codec_id);
        }
        out
    }

    /// Read a header. Returns the header and the header length.
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), PageError> {
        if bytes.len() < HEADER_FIXED {
            return Err(PageError::Truncated("header"));
        }
        if bytes[..4] != MAGIC {
            return Err(PageError::BadMagic);
        }
        let version = bytes[4];
        let column_count = u16::from_le_bytes(bytes[5..7].try_into().unwrap()) as usize;
        let row_count = u32::from_le_bytes(bytes[7..11].try_into().unwrap());
        let dictionary_id = u32::from_le_bytes(bytes[11..15].try_into().unwrap());
        let payload_len = u32::from_le_bytes(bytes[15..19].try_into().unwrap());
        let crc32c = u32::from_le_bytes(bytes[19..23].try_into().unwrap());
        let header_len = HEADER_FIXED + column_count * COLUMN_DESC;
        if bytes.len() < header_len {
            return Err(PageError::Truncated("column descriptions"));
        }
        let mut columns = Vec::with_capacity(column_count);
        for index in 0..column_count {
            let at = HEADER_FIXED + index * COLUMN_DESC;
            columns.push(ColumnDesc {
                page_type: PageType::from_tag(bytes[at])?,
                codec_id: bytes[at + 1],
            });
        }
        Ok((
            Self { version, row_count, dictionary_id, payload_len, crc32c, columns },
            header_len,
        ))
    }
}

/// The CRC32C (Castagnoli) table. The polynomial is 0x1EDC6F41, reflected to
/// 0x82F63B78.
const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0x82f6_3b78 } else { crc >> 1 };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

/// The CRC32C table, built at compile time.
static CRC_TABLE: [u32; 256] = crc_table();

/// The CRC32C of a byte range. It is the hardware CRC of x86 SSE4.2 and of the
/// iSCSI and NVMe storage paths.
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        let index = ((crc ^ byte as u32) & 0xff) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    !crc
}

/// The number of bytes in a null bitmap for `rows` rows.
pub fn bitmap_len(rows: usize) -> usize {
    rows.div_ceil(8)
}

/// True when the bitmap marks `row` as null.
pub fn is_null(bitmap: &[u8], row: usize) -> bool {
    bitmap.get(row / 8).map(|byte| byte & (1 << (row % 8)) != 0).unwrap_or(true)
}

/// Set the null bit of `row`.
pub fn set_null(bitmap: &mut [u8], row: usize) {
    bitmap[row / 8] |= 1 << (row % 8);
}

/// The canonical bytes of one value. The row fingerprint is the BLAKE3 hash of
/// the concatenated value bytes, so two equal rows always hash the same.
fn value_bytes(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(0),
        Value::Bool(flag) => {
            out.push(1);
            out.push(u8::from(*flag));
        }
        Value::Int(number) => {
            out.push(2);
            out.extend_from_slice(&number.to_le_bytes());
        }
        Value::Float(number) => {
            out.push(3);
            out.extend_from_slice(&number.to_bits().to_le_bytes());
        }
        Value::Text(text) => {
            out.push(4);
            out.extend_from_slice(&(text.len() as u32).to_le_bytes());
            out.extend_from_slice(text.as_bytes());
        }
    }
}

/// The fingerprint of one row.
pub fn row_fingerprint(row: &Row) -> u64 {
    let mut bytes = Vec::with_capacity(row.len() * 9);
    for value in row {
        value_bytes(value, &mut bytes);
    }
    let hash = blake3::hash(&bytes);
    u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap())
}

/// The version of a page that the reader saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageVersion {
    /// The JSON row array.
    V1,
    /// The columnar frame.
    V2,
}

/// The record of a version 1 page that the reader migrated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigratedFromV1 {
    /// The version that was read.
    pub source_version: u8,
    /// The number of rows in the page.
    pub rows: usize,
    /// True when the page carried the SHPG frame.
    pub framed: bool,
}

/// A decoded page.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedPage {
    /// The rows in page order.
    pub rows: Vec<Row>,
    /// The version that was read.
    pub version: PageVersion,
    /// The migration record, or None when the page was already version 2.
    pub migrated: Option<MigratedFromV1>,
    /// The CRC32C that the reader checked. Zero for a version 1 page, which
    /// carries no checksum.
    pub crc32c: u32,
    /// The dictionary that the page names.
    pub dictionary_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32c_matches_the_known_check_value() {
        // The standard CRC32C check value for "123456789" is 0xE3069283.
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
    }

    #[test]
    fn a_header_round_trips() {
        let header = PageHeader {
            version: VERSION_V2,
            row_count: 16,
            dictionary_id: 7,
            payload_len: 128,
            crc32c: 0xdead_beef,
            columns: vec![
                ColumnDesc { page_type: PageType::Int, codec_id: 2 },
                ColumnDesc { page_type: PageType::Text, codec_id: 4 },
            ],
        };
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), HEADER_FIXED + 4);
        let (parsed, len) = PageHeader::parse(&bytes).expect("parse");
        assert_eq!(len, bytes.len());
        assert_eq!(parsed, header);
    }

    #[test]
    fn a_wrong_magic_is_rejected() {
        let header = PageHeader {
            version: VERSION_V2,
            row_count: 0,
            dictionary_id: 0,
            payload_len: 0,
            crc32c: 0,
            columns: Vec::new(),
        };
        let mut bytes = header.to_bytes();
        bytes[0] = b'X';
        assert_eq!(PageHeader::parse(&bytes).err(), Some(PageError::BadMagic));
    }

    #[test]
    fn an_unknown_type_tag_is_rejected() {
        assert_eq!(
            PageType::from_tag(9).err(),
            Some(PageError::Shape("unknown page type tag"))
        );
    }

    #[test]
    fn a_null_bitmap_marks_the_stated_rows() {
        let mut bitmap = vec![0u8; bitmap_len(20)];
        set_null(&mut bitmap, 0);
        set_null(&mut bitmap, 9);
        set_null(&mut bitmap, 19);
        assert!(is_null(&bitmap, 0));
        assert!(!is_null(&bitmap, 1));
        assert!(is_null(&bitmap, 9));
        assert!(is_null(&bitmap, 19));
        assert_eq!(bitmap_len(20), 3);
    }

    #[test]
    fn equal_rows_share_a_fingerprint_and_a_changed_row_does_not() {
        let a = vec![Value::Int(1), Value::Text("x".into())];
        let b = vec![Value::Int(1), Value::Text("x".into())];
        let c = vec![Value::Int(1), Value::Text("y".into())];
        assert_eq!(row_fingerprint(&a), row_fingerprint(&b));
        assert_ne!(row_fingerprint(&a), row_fingerprint(&c));
    }

    #[test]
    fn the_float_fingerprint_uses_the_bit_pattern() {
        let a = vec![Value::Float(0.0)];
        let b = vec![Value::Float(-0.0)];
        assert_ne!(row_fingerprint(&a), row_fingerprint(&b));
    }
}
