// topology/windows.rs - Windows L3 cache discovery.
//
// GetLogicalProcessorInformationEx with RelationCache returns one record per
// logical processor per cache level. Each level 3 unified record names the CPU
// mask that shares the cache, so records with the same mask describe one
// domain.
//
// The record layout is read by fixed offsets. The union holds variable-length
// group masks, so a cast to a Rust struct would misread any record with more
// than one group.
//
// Offsets inside one record:
//   0  relationship   u32   (2 = RelationCache)
//   4  size           u32
//   8  level          u8
//   9  associativity  u8
//   10 line size      u16
//   12 cache size     u32
//   16 cache type     u32   (0 = unified, 1 = instruction, 2 = data)
//   20 reserved       18 B
//   38 group count    u16
//   40 group masks    16 B each: mask u64, group u16, reserved 6 B
//
// Only processor group 0 is reported. The thread affinity adapter in
// sheaf-runtime works in the same group, so the two views always agree.

use super::{build_domains, L3Domain, TopologyError};

type Bool = i32;

/// SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX relationship value for a cache.
const RELATION_CACHE: u32 = 2;
/// PROCESSOR_CACHE_TYPE value for a unified cache.
const CACHE_UNIFIED: u32 = 0;
/// The cache level this module reports.
const L3_LEVEL: u8 = 3;
/// Bytes before the first group mask.
const GROUP_MASK_OFFSET: usize = 40;
/// Bytes per GROUP_AFFINITY value on a 64-bit target.
const GROUP_AFFINITY_LEN: usize = 16;

#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalProcessorInformationEx(
        relationship: u32,
        buffer: *mut u8,
        length: *mut u32,
    ) -> Bool;
    fn GetLastError() -> u32;
}

fn last_error() -> i32 {
    unsafe { GetLastError() as i32 }
}

/// Read the response buffer for one relationship class.
fn query(relationship: u32) -> Result<Vec<u8>, TopologyError> {
    let mut length: u32 = 0;
    // The first call reports the needed length. It fails by design.
    unsafe {
        GetLogicalProcessorInformationEx(relationship, std::ptr::null_mut(), &mut length);
    }
    if length == 0 {
        return Err(TopologyError::OsError {
            what: "GetLogicalProcessorInformationEx",
            code: last_error(),
        });
    }
    let mut buffer = vec![0u8; length as usize];
    let ok = unsafe {
        GetLogicalProcessorInformationEx(relationship, buffer.as_mut_ptr(), &mut length)
    };
    if ok == 0 {
        return Err(TopologyError::OsError {
            what: "GetLogicalProcessorInformationEx",
            code: last_error(),
        });
    }
    buffer.truncate(length as usize);
    Ok(buffer)
}

/// Walk the response buffer record by record.
fn walk(buffer: &[u8], mut visit: impl FnMut(&[u8])) {
    let mut at = 0usize;
    while at + 8 <= buffer.len() {
        let size = u32::from_le_bytes(buffer[at + 4..at + 8].try_into().unwrap()) as usize;
        if size < 8 || at + size > buffer.len() {
            break;
        }
        visit(&buffer[at..at + size]);
        at += size;
    }
}

/// Parse one cache record into its level, type, byte size, and group 0 CPUs.
fn cache_record(record: &[u8]) -> Option<(u8, u32, u64, Vec<usize>)> {
    if record.len() < GROUP_MASK_OFFSET {
        return None;
    }
    let level = record[8];
    let size = u32::from_le_bytes(record[12..16].try_into().unwrap()) as u64;
    let kind = u32::from_le_bytes(record[16..20].try_into().unwrap());
    let groups = u16::from_le_bytes(record[38..40].try_into().unwrap()) as usize;

    let mut cpus = Vec::new();
    for index in 0..groups {
        let base = GROUP_MASK_OFFSET + index * GROUP_AFFINITY_LEN;
        if base + GROUP_AFFINITY_LEN > record.len() {
            break;
        }
        let mask = usize::from_le_bytes(record[base..base + 8].try_into().unwrap());
        let group = u16::from_le_bytes(record[base + 8..base + 10].try_into().unwrap());
        if group != 0 {
            continue;
        }
        for bit in 0..usize::BITS as usize {
            if mask & (1usize << bit) != 0 {
                cpus.push(bit);
            }
        }
    }
    Some((level, kind, size, cpus))
}

pub(super) fn l3_domains() -> Result<Vec<L3Domain>, TopologyError> {
    let buffer = query(RELATION_CACHE)?;
    let mut records: Vec<(Vec<usize>, u64)> = Vec::new();
    walk(&buffer, |record| {
        if let Some((level, kind, size, cpus)) = cache_record(record) {
            if level == L3_LEVEL && kind == CACHE_UNIFIED && !cpus.is_empty() {
                records.push((cpus, size));
            }
        }
    });
    Ok(build_domains(records))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one synthetic record with `groups` group masks.
    fn record(level: u8, kind: u32, size: u32, masks: &[(u64, u16)]) -> Vec<u8> {
        let total = GROUP_MASK_OFFSET + masks.len() * GROUP_AFFINITY_LEN;
        let mut bytes = vec![0u8; total];
        bytes[..4].copy_from_slice(&RELATION_CACHE.to_le_bytes());
        bytes[4..8].copy_from_slice(&(total as u32).to_le_bytes());
        bytes[8] = level;
        bytes[12..16].copy_from_slice(&size.to_le_bytes());
        bytes[16..20].copy_from_slice(&kind.to_le_bytes());
        bytes[38..40].copy_from_slice(&(masks.len() as u16).to_le_bytes());
        for (index, (mask, group)) in masks.iter().enumerate() {
            let base = GROUP_MASK_OFFSET + index * GROUP_AFFINITY_LEN;
            bytes[base..base + 8].copy_from_slice(&mask.to_le_bytes());
            bytes[base + 8..base + 10].copy_from_slice(&group.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn cache_record_reads_level_type_size_and_cpus() {
        let bytes = record(3, CACHE_UNIFIED, 33_554_432, &[(0b1011, 0)]);
        let (level, kind, size, cpus) =
            cache_record(&bytes).expect("a full record must parse");
        assert_eq!(level, 3);
        assert_eq!(kind, CACHE_UNIFIED);
        assert_eq!(size, 33_554_432);
        assert_eq!(cpus, vec![0, 1, 3]);
    }

    #[test]
    fn a_second_group_mask_is_ignored() {
        let bytes = record(3, CACHE_UNIFIED, 4096, &[(0b11, 0), (0b11, 1)]);
        let (_, _, _, cpus) = cache_record(&bytes).expect("parse");
        assert_eq!(cpus, vec![0, 1], "only processor group 0 is reported");
    }

    #[test]
    fn a_truncated_record_is_rejected() {
        let bytes = record(3, CACHE_UNIFIED, 4096, &[(0b1, 0)]);
        assert!(cache_record(&bytes[..8]).is_none());
    }

    #[test]
    fn walk_visits_every_record() {
        let first = record(3, CACHE_UNIFIED, 4096, &[(0b1, 0)]);
        let second = record(1, 2, 1024, &[(0b1, 0)]);
        let mut buffer = first.clone();
        buffer.extend_from_slice(&second);
        let mut count = 0usize;
        walk(&buffer, |_| count += 1);
        assert_eq!(count, 2);
    }
}
