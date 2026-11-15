// pinning/linux.rs - Linux thread affinity adapter.
//
// sched_setaffinity changes the CPU mask of a thread. A pid of 0 addresses the
// calling thread. The kernel writes the mask into a cpu_set_t, which is 128
// bytes for the default 1024-CPU limit.

use super::{PinError, PinErrorKind};

/// Size of cpu_set_t on glibc and musl.
const CPU_SET_BYTES: usize = 128;

extern "C" {
    fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u8) -> i32;
    fn sched_getaffinity(pid: i32, cpusetsize: usize, mask: *mut u8) -> i32;
}

fn read_bytes() -> Result<[u8; CPU_SET_BYTES], PinError> {
    let mut mask = [0u8; CPU_SET_BYTES];
    let rc = unsafe { sched_getaffinity(0, CPU_SET_BYTES, mask.as_mut_ptr()) };
    if rc != 0 {
        return Err(PinError::os(std::io::Error::last_os_error().raw_os_error().unwrap_or(0), "sched_getaffinity"));
    }
    Ok(mask)
}

fn set_bytes(mask: &[u8; CPU_SET_BYTES]) -> Result<(), PinError> {
    let rc = unsafe { sched_setaffinity(0, CPU_SET_BYTES, mask.as_ptr()) };
    if rc != 0 {
        return Err(PinError::os(std::io::Error::last_os_error().raw_os_error().unwrap_or(0), "sched_setaffinity"));
    }
    Ok(())
}

fn list(mask: &[u8; CPU_SET_BYTES]) -> Vec<usize> {
    let mut out = Vec::new();
    for core in 0..CPU_SET_BYTES * 8 {
        if mask[core / 8] & (1u8 << (core % 8)) != 0 {
            out.push(core);
        }
    }
    out
}

pub(super) fn allowed_cores() -> Result<Vec<usize>, PinError> {
    Ok(list(&read_bytes()?))
}

pub(super) fn current_cores() -> Result<Vec<usize>, PinError> {
    Ok(list(&read_bytes()?))
}

pub(super) fn set_cores(cores: &[usize]) -> Result<(), PinError> {
    let mut mask = [0u8; CPU_SET_BYTES];
    for &core in cores {
        if core >= CPU_SET_BYTES * 8 {
            return Err(PinError::new(
                PinErrorKind::CoreUnavailable,
                format!("core {core} is outside cpu_set_t"),
            ));
        }
        mask[core / 8] |= 1u8 << (core % 8);
    }
    set_bytes(&mask)
}
