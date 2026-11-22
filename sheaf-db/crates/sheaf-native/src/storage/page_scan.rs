// Timestamp: 2026-11-22_11-53-08 +05:30
// page_scan.rs - physical page row ranges (C-04).
//
// The Zig storage tier owns this loop. The Rust fallback walks the same spine
// through the storage port and reports its source. A truncated frame is an
// explicit error in both paths.

use super::{StorageError, SPINE_BYTES};

/// The row ranges of one page payload, in spine slot order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowRanges {
    pub offsets: Vec<u64>,
    pub lengths: Vec<u32>,
    pub source: &'static str,
}

/// Read the row ranges from one packed spine and its payload.
pub fn page_ranges(
    spine: &[u8; SPINE_BYTES],
    payload: &[u8],
    offsets: &mut [u64],
    lengths: &mut [u32],
) -> Result<usize, StorageError> {
    if offsets.len() != lengths.len() {
        return Err(StorageError::InvalidArgument);
    }
    #[cfg(sheaf_native_storage)]
    {
        let local = super::native::spine_local(spine);
        let count = unsafe {
            super::ffi::sheaf_page_row_ranges(
                &local,
                payload.as_ptr(),
                payload.len(),
                offsets.as_mut_ptr(),
                lengths.as_mut_ptr(),
                offsets.len() as u32,
            )
        };
        if count < 0 {
            return Err(StorageError::Corrupt);
        }
        Ok(count as usize)
    }
    #[cfg(not(sheaf_native_storage))]
    {
        let backend = crate::backends::storage();
        let count = backend.spine_count(spine);
        if count > offsets.len() {
            return Err(StorageError::InvalidArgument);
        }
        for slot in 0..count {
            let entry = backend.spine_get(spine, slot).ok_or(StorageError::Corrupt)?;
            let start = usize::try_from(entry.byte_offset).map_err(|_| StorageError::Corrupt)?;
            if start > payload.len() || payload.len() - start < 4 {
                return Err(StorageError::Corrupt);
            }
            let length = u32::from_le_bytes(payload[start..start + 4].try_into().unwrap()) as usize;
            if length > payload.len() - start - 4 {
                return Err(StorageError::Corrupt);
            }
            offsets[slot] = entry.byte_offset;
            lengths[slot] = length as u32;
        }
        Ok(count)
    }
}

/// A reported source string for diagnostics.
pub fn source() -> &'static str {
    if cfg!(sheaf_native_storage) {
        "zig"
    } else {
        "rust-fallback"
    }
}
