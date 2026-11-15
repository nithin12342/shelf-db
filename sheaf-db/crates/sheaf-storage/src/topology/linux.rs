// topology/linux.rs - Linux L3 cache discovery from sysfs.
//
// Each CPU has /sys/devices/system/cpu/cpuN/cache/indexM/ with these files:
//   level           the cache level
//   type            Data, Instruction, or Unified
//   shared_cpu_list the CPUs that share the cache, as "0-7,16-23"
//   size            the cache size, as "32768K"
//
// The CPUs that share one level 3 unified cache form one domain. Two CPUs in
// the same domain report the same shared_cpu_list, so the duplicate records
// collapse in build_domains.

use std::path::{Path, PathBuf};

use super::sysfs_parse::{parse_cpu_list, parse_size};
use super::{build_domains, L3Domain, TopologyError};

/// The sysfs root for the CPU cache files.
const CPU_ROOT: &str = "/sys/devices/system/cpu";

fn io_error(path: &Path, detail: impl ToString) -> TopologyError {
    TopologyError::Io { path: path.display().to_string(), detail: detail.to_string() }
}

fn read_trim(path: &Path) -> Result<String, TopologyError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, e))?;
    Ok(text.trim().to_string())
}

/// The CPU directories, in ascending CPU order.
fn cpu_dirs(root: &Path) -> Result<Vec<(usize, PathBuf)>, TopologyError> {
    let mut out: Vec<(usize, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| io_error(root, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_error(root, e))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.strip_prefix("cpu") {
            if let Ok(cpu) = rest.parse::<usize>() {
                out.push((cpu, entry.path()));
            }
        }
    }
    out.sort_by_key(|(cpu, _)| *cpu);
    Ok(out)
}

/// Read the level of one cache index directory.
fn read_level(base: &Path) -> Result<u8, TopologyError> {
    let path = base.join("level");
    read_trim(&path)?.parse::<u8>().map_err(|_| io_error(&path, "bad cache level"))
}

pub(super) fn l3_domains() -> Result<Vec<L3Domain>, TopologyError> {
    let root = Path::new(CPU_ROOT);
    let mut records: Vec<(Vec<usize>, u64)> = Vec::new();

    for (_, dir) in cpu_dirs(root)? {
        let cache_dir = dir.join("cache");
        // A CPU with no cache directory cannot describe an L3 share. The
        // coverage check in mod.rs reports the gap.
        let Ok(entries) = std::fs::read_dir(&cache_dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.map_err(|e| io_error(&cache_dir, e))?;
            let base = entry.path();
            if !base.join("level").exists() {
                continue;
            }
            if read_level(&base)? != 3 {
                continue;
            }
            if read_trim(&base.join("type"))? != "Unified" {
                continue;
            }
            let cpus = parse_cpu_list(&read_trim(&base.join("shared_cpu_list"))?)?;
            let size = parse_size(&read_trim(&base.join("size"))?)?;
            records.push((cpus, size));
        }
    }

    Ok(build_domains(records))
}
