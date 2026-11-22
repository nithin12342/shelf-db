// rdma_native.rs - Rust adapter for the C11 RoCEv2 replication tier (C-06).
//
// Timestamp: 2026-11-22_12-48-16 +05:30
// Remote: origin https://github.com/nithin12342/shelf-db.git
// Branch: combined-stack-feature-improvement
// Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
//
// This file selects one of two paths at build time. When build.rs sets the
// cfg flag sheaf_native_rdma, the adapter calls the C ABI in native/rdma-c.
// Without that flag the adapter calls no C function and returns the same named
// unsupported reason that the C probe reports when libibverbs is absent. It
// never returns an acknowledgement that it did not earn from a posted write.
//
// The parent module owns wire registration of this file. This file only depends
// on the replication port and the section delta from that module.

#![allow(unexpected_cfgs)]

use super::port::{Ack, Capability, RdmaTransport, ReplicationError, Result};
use super::SectionDelta;

/// The named reason when libibverbs is absent. This text is byte-identical to
/// SHEAF_RDMA_REASON_NO_VERBS in native/include/sheaf_rdma.h.
pub const REASON: &str =
    "libibverbs is not available on this host (SHEAF_HAVE_LIBIBVERBS is not defined)";

/// A probe that answered none but gave no reason. Kept separate so a silent
/// result never masquerades as the named library reason.
const REASON_PROBE_SILENT: &str = "the RDMA capability probe returned no named reason";

const CAP_NONE: u32 = 0;
const CAP_ROCEV2: u32 = 1;

const OK: i32 = 0;
const EINVAL: i32 = -1;
const ECAP: i32 = -2;
const EIO: i32 = -3;
const ENOMEM: i32 = -4;

/// The fixed staging region. It is registered once and never resized, so its
/// address stays valid for the life of the endpoint.
const STAGING_BYTES: usize = super::frame::MAX_PATCH_BYTES;

/// The adapter. The state field exists only in a build that links the C tier.
/// A default value on any other build is a valid unsupported adapter.
#[derive(Default)]
pub struct RdmaNative {
    #[cfg(sheaf_native_rdma)]
    state: Option<NativeState>,
}

#[cfg(sheaf_native_rdma)]
mod ffi {
    //! Rust mirror of native/include/sheaf_rdma.h. Field order and types are
    //! part of the C ABI. The endpoint is opaque and owned by the C tier.

    use std::os::raw::c_char;

    #[repr(C)]
    pub struct SheafRdmaEndpoint {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct SheafRdmaBuffer {
        pub addr: *mut u8,
        pub length: usize,
        pub lkey: u32,
        pub rkey: u32,
        pub handle: u64,
    }

    #[repr(C)]
    pub struct SheafRdmaWriteDesc {
        pub remote_addr: u64,
        pub rkey: u32,
        pub data: *const u8,
        pub len: usize,
    }

    extern "C" {
        pub fn sheaf_rdma_capability() -> u32;
        pub fn sheaf_rdma_capability_reason() -> *const c_char;
        pub fn sheaf_rdma_endpoint_open(out: *mut *mut SheafRdmaEndpoint) -> i32;
        pub fn sheaf_rdma_endpoint_close(endpoint: *mut SheafRdmaEndpoint) -> i32;
        pub fn sheaf_rdma_register(
            endpoint: *mut SheafRdmaEndpoint,
            out: *mut SheafRdmaBuffer,
            addr: *mut u8,
            length: usize,
        ) -> i32;
        pub fn sheaf_rdma_deregister(
            endpoint: *mut SheafRdmaEndpoint,
            buffer: *mut SheafRdmaBuffer,
        ) -> i32;
        pub fn sheaf_rdma_post_write(
            endpoint: *mut SheafRdmaEndpoint,
            local: *const SheafRdmaBuffer,
            desc: *const SheafRdmaWriteDesc,
        ) -> i32;
    }
}

#[cfg(sheaf_native_rdma)]
struct NativeState {
    endpoint: *mut ffi::SheafRdmaEndpoint,
    local: ffi::SheafRdmaBuffer,
    staging: std::cell::RefCell<Vec<u8>>,
    remote_addr: u64,
    remote_rkey: u32,
}

/// Read the reason that the C probe stored. An empty string is not a named
/// reason, so the caller gets a separate explicit cause in that case.
#[cfg(sheaf_native_rdma)]
unsafe fn c_reason() -> String {
    let pointer = ffi::sheaf_rdma_capability_reason();
    if pointer.is_null() {
        return REASON_PROBE_SILENT.to_owned();
    }
    let text = std::ffi::CStr::from_ptr(pointer).to_string_lossy().into_owned();
    if text.is_empty() {
        REASON_PROBE_SILENT.to_owned()
    } else {
        text
    }
}

/// Map a C return code to the replication error vocabulary. An ECAP code is
/// always reported as Unsupported with the reason from the C tier.
#[cfg(sheaf_native_rdma)]
fn map_error(code: i32, reason: String) -> ReplicationError {
    match code {
        ECAP => ReplicationError::Unsupported(reason),
        EINVAL => ReplicationError::Framing("the RDMA request had a bad argument".to_owned()),
        EIO => ReplicationError::Io("the verbs provider rejected the request".to_owned()),
        ENOMEM => ReplicationError::Io("the verbs provider could not allocate an object".to_owned()),
        other => ReplicationError::Io(format!("the RDMA tier returned code {other}")),
    }
}

impl RdmaNative {
    /// Open the C tier and register one staging region for the follower at
    /// `remote_addr` with remote key `remote_rkey`. On a build without the C
    /// tier this returns the explicit named unsupported reason.
    pub fn open(remote_addr: u64, remote_rkey: u32) -> Result<Self> {
        #[cfg(not(sheaf_native_rdma))]
        {
            let _ = (remote_addr, remote_rkey);
            Err(ReplicationError::Unsupported(REASON.to_owned()))
        }

        #[cfg(sheaf_native_rdma)]
        {
            let mut endpoint: *mut ffi::SheafRdmaEndpoint = std::ptr::null_mut();
            let code = unsafe { ffi::sheaf_rdma_endpoint_open(&mut endpoint) };
            if code != OK {
                return Err(map_error(code, unsafe { c_reason() }));
            }

            let mut staging = vec![0u8; STAGING_BYTES];
            let mut local = ffi::SheafRdmaBuffer {
                addr: std::ptr::null_mut(),
                length: 0,
                lkey: 0,
                rkey: 0,
                handle: 0,
            };
            let code = unsafe {
                ffi::sheaf_rdma_register(
                    endpoint,
                    &mut local,
                    staging.as_mut_ptr(),
                    staging.len(),
                )
            };
            if code != OK {
                unsafe { ffi::sheaf_rdma_endpoint_close(endpoint) };
                return Err(map_error(code, unsafe { c_reason() }));
            }

            let state = NativeState {
                endpoint,
                local,
                staging: std::cell::RefCell::new(staging),
                remote_addr,
                remote_rkey,
            };
            Ok(Self { state: Some(state) })
        }
    }
}

impl RdmaTransport for RdmaNative {
    fn capability(&self) -> Capability {
        #[cfg(not(sheaf_native_rdma))]
        {
            Capability::Unsupported { reason: REASON.to_owned() }
        }

        #[cfg(sheaf_native_rdma)]
        {
            if self.state.is_none() {
                return Capability::Unsupported { reason: REASON.to_owned() };
            }
            let capability = unsafe { ffi::sheaf_rdma_capability() };
            if capability == CAP_ROCEV2 {
                Capability::Available
            } else {
                Capability::Unsupported { reason: unsafe { c_reason() } }
            }
        }
    }

    fn post_write(&self, delta: &SectionDelta) -> Result<Ack> {
        #[cfg(not(sheaf_native_rdma))]
        {
            let _ = delta;
            Err(ReplicationError::Unsupported(REASON.to_owned()))
        }

        #[cfg(sheaf_native_rdma)]
        {
            let state = match &self.state {
                Some(state) => state,
                None => return Err(ReplicationError::Unsupported(REASON.to_owned())),
            };

            // Check capability again at the call, never cache a past result.
            let capability = unsafe { ffi::sheaf_rdma_capability() };
            if capability != CAP_ROCEV2 || capability == CAP_NONE {
                return Err(ReplicationError::Unsupported(unsafe { c_reason() }));
            }
            if delta.xor_patch.len() > state.local.length {
                return Err(ReplicationError::Framing(
                    "the delta exceeds the registered staging region".to_owned(),
                ));
            }

            // The region is fixed at open, so this copy does not move it.
            {
                let mut staging = state.staging.borrow_mut();
                staging[..delta.xor_patch.len()].copy_from_slice(&delta.xor_patch);
            }
            let descriptor = ffi::SheafRdmaWriteDesc {
                remote_addr: state.remote_addr,
                rkey: state.remote_rkey,
                data: state.staging.borrow().as_ptr(),
                len: delta.xor_patch.len(),
            };
            let code = unsafe {
                ffi::sheaf_rdma_post_write(state.endpoint, &state.local, &descriptor)
            };
            if code != OK {
                return Err(map_error(code, unsafe { c_reason() }));
            }
            Ok(Ack { sequence: delta.sequence })
        }
    }
}

impl Drop for RdmaNative {
    fn drop(&mut self) {
        #[cfg(sheaf_native_rdma)]
        if let Some(mut state) = self.state.take() {
            // SAFETY: the endpoint and region came from open in this object.
            // Cleanup errors are ignored by Drop, never reported as success.
            unsafe {
                let _ = ffi::sheaf_rdma_deregister(state.endpoint, &mut state.local);
                let _ = ffi::sheaf_rdma_endpoint_close(state.endpoint);
            }
        }
    }
}

#[cfg(all(test, not(sheaf_native_rdma)))]
mod tests {
    use super::*;

    #[test]
    fn default_adapter_reports_the_named_reason() {
        let adapter = RdmaNative::default();
        assert_eq!(
            adapter.capability(),
            Capability::Unsupported { reason: REASON.to_owned() }
        );
    }

    #[test]
    fn post_write_returns_unsupported_without_a_fake_ack() {
        let adapter = RdmaNative::default();
        let delta = SectionDelta::new(7, 42, vec![0x5a, 0xa5]);
        assert_eq!(
            adapter.post_write(&delta),
            Err(ReplicationError::Unsupported(REASON.to_owned()))
        );
    }

    #[test]
    fn open_reports_unsupported_on_a_build_without_the_c_tier() {
        assert_eq!(
            RdmaNative::open(0x1000, 9).err(),
            Some(ReplicationError::Unsupported(REASON.to_owned()))
        );
    }
}
