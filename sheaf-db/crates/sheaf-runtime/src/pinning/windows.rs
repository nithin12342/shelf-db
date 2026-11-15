// pinning/windows.rs - Windows thread affinity adapter.
//
// SetThreadAffinityMask changes the mask of the calling thread.
// GetThreadGroupAffinity reads it back. Both calls stay inside processor
// group 0, which holds at most 64 logical CPUs. A core outside that group is
// reported as unavailable instead of being silently mapped.

use super::{PinError, PinErrorKind};

type Handle = *mut core::ffi::c_void;
type Bool = i32;

/// GROUP_AFFINITY from winnt.h.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GroupAffinity {
    mask: usize,
    group: u16,
    reserved: [u16; 3],
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThread() -> Handle;
    fn GetCurrentProcess() -> Handle;
    fn SetThreadAffinityMask(thread: Handle, mask: usize) -> usize;
    fn GetThreadGroupAffinity(thread: Handle, out: *mut GroupAffinity) -> Bool;
    fn GetProcessAffinityMask(process: Handle, process_mask: *mut usize, system_mask: *mut usize)
        -> Bool;
    fn GetActiveProcessorCount(group: u16) -> u32;
    fn GetLastError() -> u32;
}

/// List the set bits of a 64-bit mask.
fn bits(mask: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for core in 0..usize::BITS as usize {
        if mask & (1usize << core) != 0 {
            out.push(core);
        }
    }
    out
}

/// Build a mask from a core list. A core at or above 64 is a hard error.
fn mask(cores: &[usize]) -> Result<usize, PinError> {
    let mut out = 0usize;
    for &core in cores {
        if core >= usize::BITS as usize {
            return Err(PinError::new(
                PinErrorKind::CoreUnavailable,
                format!("core {core} is outside processor group 0"),
            ));
        }
        out |= 1usize << core;
    }
    Ok(out)
}

fn last_error(what: &str) -> PinError {
    let code = unsafe { GetLastError() } as i32;
    PinError::os(code, what)
}

pub(super) fn allowed_cores() -> Result<Vec<usize>, PinError> {
    let mut process_mask = 0usize;
    let mut system_mask = 0usize;
    let ok = unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut process_mask, &mut system_mask) };
    if ok != 0 && process_mask != 0 {
        return Ok(bits(process_mask));
    }
    // A process that spans processor groups reports a zero mask here. Use the
    // processor count of group 0 instead of a guess.
    let count = unsafe { GetActiveProcessorCount(0) } as usize;
    if count == 0 {
        return Err(last_error("GetActiveProcessorCount"));
    }
    Ok((0..count.min(usize::BITS as usize)).collect())
}

pub(super) fn current_cores() -> Result<Vec<usize>, PinError> {
    let mut info = GroupAffinity::default();
    let ok = unsafe { GetThreadGroupAffinity(GetCurrentThread(), &mut info) };
    if ok == 0 {
        return Err(last_error("GetThreadGroupAffinity"));
    }
    Ok(bits(info.mask))
}

pub(super) fn set_cores(cores: &[usize]) -> Result<(), PinError> {
    let want = mask(cores)?;
    let previous = unsafe { SetThreadAffinityMask(GetCurrentThread(), want) };
    if previous == 0 {
        return Err(last_error("SetThreadAffinityMask"));
    }
    Ok(())
}
