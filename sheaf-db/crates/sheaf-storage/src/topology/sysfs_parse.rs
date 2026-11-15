// topology/sysfs_parse.rs - parsers for the Linux sysfs cache text files.
//
// The two text forms are small but easy to get wrong:
//   shared_cpu_list   "0-7,16-23" or "0"
//   size              "32768K", "2M", "1G", or a plain byte count
//
// The parsers live here, not in linux.rs, so every target compiles and tests
// them. The Linux adapter is the only caller, and a host without sysfs still
// proves that the text handling is correct.

use super::TopologyError;

/// Parse a CPU list. Ranges and single entries can mix. The result is sorted
/// and deduplicated.
pub fn parse_cpu_list(text: &str) -> Result<Vec<usize>, TopologyError> {
    let mut out: Vec<usize> = Vec::new();
    let bad = || TopologyError::Io {
        path: "shared_cpu_list".to_string(),
        detail: format!("bad CPU list {text:?}"),
    };
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('-') {
            Some((low, high)) => {
                let low: usize = low.trim().parse().map_err(|_| bad())?;
                let high: usize = high.trim().parse().map_err(|_| bad())?;
                if low > high {
                    return Err(bad());
                }
                out.extend(low..=high);
            }
            None => out.push(part.parse().map_err(|_| bad())?),
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Strip a size suffix and return the multiplier.
fn split_suffix(text: &str) -> (&str, u64) {
    if let Some(rest) = text.strip_suffix('K').or_else(|| text.strip_suffix('k')) {
        return (rest, 1024);
    }
    if let Some(rest) = text.strip_suffix('M').or_else(|| text.strip_suffix('m')) {
        return (rest, 1024 * 1024);
    }
    if let Some(rest) = text.strip_suffix('G').or_else(|| text.strip_suffix('g')) {
        return (rest, 1024 * 1024 * 1024);
    }
    (text, 1)
}

/// Parse a cache size. A suffix of K, M, or G is a power of 1024.
pub fn parse_size(text: &str) -> Result<u64, TopologyError> {
    let text = text.trim();
    let (digits, scale) = split_suffix(text);
    let value: u64 = digits.trim().parse().map_err(|_| TopologyError::Io {
        path: "cache/size".to_string(),
        detail: format!("bad cache size {text:?}"),
    })?;
    Ok(value * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_list_parses_ranges_and_singles() {
        assert_eq!(parse_cpu_list("0-3").expect("range"), vec![0, 1, 2, 3]);
        assert_eq!(parse_cpu_list("0,2,4").expect("singles"), vec![0, 2, 4]);
        assert_eq!(parse_cpu_list("0-1,8-9").expect("mixed"), vec![0, 1, 8, 9]);
        assert_eq!(parse_cpu_list("5").expect("one"), vec![5]);
        assert_eq!(parse_cpu_list("3,3,2").expect("dedup"), vec![2, 3]);
    }

    #[test]
    fn a_bad_cpu_list_is_rejected() {
        assert!(matches!(parse_cpu_list("7-3"), Err(TopologyError::Io { .. })));
        assert!(matches!(parse_cpu_list("x"), Err(TopologyError::Io { .. })));
    }

    #[test]
    fn cache_size_parses_every_suffix() {
        assert_eq!(parse_size("32768K").expect("kilobytes"), 33_554_432);
        assert_eq!(parse_size("2M").expect("megabytes"), 2_097_152);
        assert_eq!(parse_size("1G").expect("gigabytes"), 1_073_741_824);
        assert_eq!(parse_size("512").expect("bytes"), 512);
    }

    #[test]
    fn a_bad_size_is_rejected() {
        assert!(matches!(parse_size("large"), Err(TopologyError::Io { .. })));
    }
}
