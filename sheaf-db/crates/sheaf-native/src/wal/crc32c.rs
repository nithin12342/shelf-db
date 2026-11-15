//! crc32c - Castagnoli CRC32C, identical to the C WAL framing function.
//!
//! Reflected polynomial 0x82F63B78, init 0xFFFFFFFF, final XOR 0xFFFFFFFF.
//! The bitwise recurrence needs no tables and matches frame.c byte for
//! byte, so the Rust fallback and the C tier agree on every checksum.

const POLY: u32 = 0x82F6_3B78;

/// Compute CRC32C over `data`.
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            // Branchless step: mask is all-ones when the low bit is set.
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (POLY & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::crc32c;

    #[test]
    fn known_check_value() {
        // The standard CRC32C check value for "123456789".
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn empty_input() {
        assert_eq!(crc32c(b""), 0x0000_0000);
    }
}