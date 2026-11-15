// error.rs - the explicit codec errors (MF-05).
//
// No codec guesses. An unknown id, a block that a codec cannot represent, and
// a corrupt buffer each return their own variant, so the selector and the
// decoder can report the exact reason.

use std::fmt;

/// Why a codec operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    /// No codec is registered with this id.
    UnknownCodec(u8),
    /// The codec declines this block. The reason is recorded, not hidden.
    NotApplicable(&'static str),
    /// The codec does not accept this column type.
    UnsupportedType(&'static str),
    /// The buffer is truncated or carries bad values.
    Corrupt(&'static str),
    /// A symbol table would exceed 255 symbols.
    TooManySymbols(usize),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::UnknownCodec(id) => write!(f, "codec id {id} is not registered"),
            CodecError::NotApplicable(reason) => write!(f, "codec declined the block: {reason}"),
            CodecError::UnsupportedType(name) => write!(f, "the codec does not accept {name}"),
            CodecError::Corrupt(reason) => write!(f, "corrupt codec buffer: {reason}"),
            CodecError::TooManySymbols(count) => {
                write!(f, "the symbol table holds {count} symbols, above the limit of 255")
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// Read a u32 length or count from a buffer, or report a truncation.
pub(crate) fn take_u32(bytes: &[u8], at: usize) -> Result<u32, CodecError> {
    let end = at + 4;
    if end > bytes.len() {
        return Err(CodecError::Corrupt("truncated u32"));
    }
    Ok(u32::from_le_bytes(bytes[at..end].try_into().unwrap()))
}

/// Read one byte, or report a truncation.
pub(crate) fn take_u8(bytes: &[u8], at: usize) -> Result<u8, CodecError> {
    bytes.get(at).copied().ok_or(CodecError::Corrupt("truncated byte"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_has_a_readable_detail() {
        assert_eq!(CodecError::UnknownCodec(9).to_string(), "codec id 9 is not registered");
        assert_eq!(
            CodecError::NotApplicable("round trip differs").to_string(),
            "codec declined the block: round trip differs"
        );
        assert_eq!(
            CodecError::Corrupt("truncated u32").to_string(),
            "corrupt codec buffer: truncated u32"
        );
        assert_eq!(
            CodecError::TooManySymbols(300).to_string(),
            "the symbol table holds 300 symbols, above the limit of 255"
        );
    }

    #[test]
    fn take_reports_a_truncated_buffer() {
        assert_eq!(take_u32(&[1, 0], 0), Err(CodecError::Corrupt("truncated u32")));
        assert_eq!(take_u8(&[], 0), Err(CodecError::Corrupt("truncated byte")));
        assert_eq!(take_u32(&[4, 0, 0, 0], 0), Ok(4));
    }
}
