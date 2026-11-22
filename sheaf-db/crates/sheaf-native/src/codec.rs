// codec.rs - Rust port for the C++23 run-length codec (C-03).
//
// The selector in sheaf-codec still chooses the codec. The byte loops live in
// C++23. The `source` field reports which adapter produced the bytes.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    InvalidArgument,
    BufferTooSmall,
    Corrupt,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => f.write_str("invalid codec argument"),
            Self::BufferTooSmall => f.write_str("output buffer too small"),
            Self::Corrupt => f.write_str("corrupt run-length block"),
        }
    }
}

impl std::error::Error for CodecError {}

/// Encode one u64 block in the stable run-length format.
pub fn rle_encode_u64(values: &[u64]) -> Result<(Vec<u8>, &'static str), CodecError> {
    if values.is_empty() || values.len() > u32::MAX as usize {
        return Err(CodecError::InvalidArgument);
    }
    #[cfg(sheaf_native_kernel)]
    {
        let capacity = 9usize + values.len() * 12;
        let mut out = vec![0u8; capacity];
        let mut written = 0u32;
        let code = unsafe {
            crate::abi::sheaf_rle_encode_u64(
                values.as_ptr(),
                values.len() as u32,
                out.as_mut_ptr(),
                capacity as u32,
                &mut written,
            )
        };
        match code {
            0 => {
                out.truncate(written as usize);
                Ok((out, "cpp23"))
            }
            -2 => Err(CodecError::BufferTooSmall),
            _ => Err(CodecError::InvalidArgument),
        }
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        let mut runs: Vec<(u32, u64)> = Vec::new();
        for &value in values {
            match runs.last_mut() {
                Some((length, last)) if *last == value => *length += 1,
                _ => runs.push((1, value)),
            }
        }
        let mut out = Vec::with_capacity(9 + runs.len() * 12);
        out.push(0);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (length, value) in runs {
            out.extend_from_slice(&length.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        Ok((out, "rust-fallback"))
    }
}

/// Decode one u64 run-length block.
pub fn rle_decode_u64(bytes: &[u8], count: usize) -> Result<(Vec<u64>, &'static str), CodecError> {
    #[cfg(sheaf_native_kernel)]
    {
        let mut out = vec![0u64; count];
        let mut written = 0u32;
        let code = unsafe {
            crate::abi::sheaf_rle_decode_u64(
                bytes.as_ptr(),
                bytes.len() as u32,
                out.as_mut_ptr(),
                count as u32,
                &mut written,
            )
        };
        if code != 0 || written as usize != count {
            return Err(CodecError::Corrupt);
        }
        Ok((out, "cpp23"))
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        if bytes.len() < 9 || bytes[0] != 0 {
            return Err(CodecError::Corrupt);
        }
        let runs = u32::from_le_bytes(bytes[5..9].try_into().unwrap()) as usize;
        let mut at = 9;
        let mut out = Vec::with_capacity(count);
        for _ in 0..runs {
            let length = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            let value = u64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap());
            at += 12;
            out.extend(std::iter::repeat(value).take(length));
        }
        if out.len() != count {
            return Err(CodecError::Corrupt);
        }
        Ok((out, "rust-fallback"))
    }
}
