//! registered - pre-registered io_uring write buffers (MF-08, Pillar 3).
//!
//! A durable WAL write pays one cost per record when the kernel maps and pins
//! the payload on each submit. A registered pool removes that cost. The kernel
//! pins the buffers once with `io_uring_register_buffers`, and each submit
//! carries only a buffer index through `IORING_OP_WRITE_FIXED`.
//!
//! Port and adapters, the same shape as the rest of this crate:
//!
//! ```text
//! commit code
//!        |  depends on RegisteredBuffers only
//!        v
//! RegisteredBuffers                 (this file, one public type)
//!        |---------------------------|
//!        v                           v
//! sys::sheaf_* (C11 WAL library)   sys::sheaf_* (pure-Rust pool)
//! cfg(sheaf_native_wal)            cfg(not(sheaf_native_wal))
//! ```
//!
//! The pure-Rust adapter is not a stub that pretends. It allocates the same
//! 64-byte aligned pool, so the alignment invariant holds on every machine,
//! and it reports `Capability::Unsupported` with the real cause for register
//! and for a fixed write. The caller records the capability and keeps the
//! normal write path. No call returns a fake success.
//!
//! Windows has no io_uring. On Windows `capability()` returns
//! `Capability::Unsupported { reason }` and `register` returns the same
//! `Unsupported` case. The reason names the platform.

use std::ffi::CStr;
use std::fmt;
use std::path::{Path, PathBuf};

/// Every buffer starts on a boundary this wide.
pub const ALIGNMENT: usize = 64;

/// The largest pool that one registration accepts.
pub const MAX_BUFFERS: u32 = 64;

/// What the platform offers for fixed writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    /// io_uring is present. `register` and `write_fixed` can succeed.
    IoUring,
    /// io_uring is absent. The reason names the cause. The caller keeps the
    /// normal blocking write path and records this value.
    Unsupported { reason: String },
}

impl Capability {
    /// True when the fixed-write path is available.
    pub fn is_supported(&self) -> bool {
        matches!(self, Capability::IoUring)
    }

    /// The cause of an absent capability, or None when it is present.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Capability::IoUring => None,
            Capability::Unsupported { reason } => Some(reason.as_str()),
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::IoUring => write!(f, "io-uring"),
            Capability::Unsupported { reason } => write!(f, "unsupported: {reason}"),
        }
    }
}

/// Why a registered-buffer call failed. Every case is explicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegBufError {
    /// The platform has no io_uring. The reason names the cause.
    Unsupported { reason: String },
    /// The pool is registered already, or was registered twice.
    AlreadyRegistered,
    /// A write was asked for before the pool was registered.
    NotRegistered,
    /// The buffer index is outside the pool.
    BufferIndexOutOfRange { index: u32, count: u32 },
    /// The call site passed a value that the pool rejects.
    InvalidArgument(&'static str),
    /// A system call failed. This is the raw errno, unchanged.
    RawErrno(i32),
}

impl fmt::Display for RegBufError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegBufError::Unsupported { reason } => {
                write!(f, "registered buffers are unsupported: {reason}")
            }
            RegBufError::AlreadyRegistered => write!(f, "the buffer pool is registered already"),
            RegBufError::NotRegistered => write!(f, "the buffer pool is not registered"),
            RegBufError::BufferIndexOutOfRange { index, count } => write!(
                f,
                "buffer index {index} is outside the pool of {count} buffers"
            ),
            RegBufError::InvalidArgument(detail) => write!(f, "invalid argument: {detail}"),
            RegBufError::RawErrno(code) => write!(f, "the system call failed with errno {code}"),
        }
    }
}

impl std::error::Error for RegBufError {}

/// What this build and this kernel offer. It does not allocate a pool.
pub fn platform_capability() -> Capability {
    // SAFETY: the call takes no arguments and only probes the kernel.
    if unsafe { sys::sheaf_regbuf_capability() } == sys::CAP_IO_URING {
        Capability::IoUring
    } else {
        Capability::Unsupported { reason: platform_reason() }
    }
}

/// The sentence that names why the capability is absent.
fn platform_reason() -> String {
    // SAFETY: the C tier returns a pointer to a static string, or null.
    let ptr = unsafe { sys::sheaf_regbuf_capability_reason() };
    if ptr.is_null() {
        return "the native tier reported no reason".to_string();
    }
    // SAFETY: a non-null pointer addresses a NUL-terminated string that lives
    // for the process life.
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

/// One pool of pre-registered write buffers.
///
/// The pool owns the memory and the file handle. `Drop` unregisters and frees
/// both, so a caller cannot leak a kernel pin.
pub struct RegisteredBuffers {
    path: PathBuf,
    handle: *mut sys::SheafRegBufPool,
    count: u32,
    buffer_bytes: u32,
    registered: bool,
}

impl RegisteredBuffers {
    /// Allocate a pool of `count` buffers of `buffer_bytes` bytes each, and
    /// open `path` for writing.
    ///
    /// `count` must be in `1..=MAX_BUFFERS` and `buffer_bytes` must be a
    /// non-zero multiple of `ALIGNMENT`. Every buffer then starts on a 64-byte
    /// boundary.
    pub fn new(
        path: impl AsRef<Path>,
        count: u32,
        buffer_bytes: u32,
    ) -> Result<Self, RegBufError> {
        if count == 0 || count > MAX_BUFFERS {
            return Err(RegBufError::InvalidArgument(
                "the buffer count must be between one and 64",
            ));
        }
        if buffer_bytes == 0 {
            return Err(RegBufError::InvalidArgument("the buffer size must be above zero"));
        }
        if buffer_bytes as usize % ALIGNMENT != 0 {
            return Err(RegBufError::InvalidArgument(
                "the buffer size must be a multiple of 64 bytes",
            ));
        }
        let path = path.as_ref().to_path_buf();
        let c_path = c_path(&path)?;
        // SAFETY: the C string lives for the call; the C tier copies the path.
        let handle =
            unsafe { sys::sheaf_regbuf_pool_create(c_path.as_ptr(), count, buffer_bytes) };
        if handle.is_null() {
            return Err(RegBufError::InvalidArgument(
                "the pool could not be allocated or the file could not be opened",
            ));
        }
        Ok(Self { path, handle, count, buffer_bytes, registered: false })
    }

    /// The path that the pool writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The number of buffers.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// The bytes of one buffer.
    pub fn buffer_bytes(&self) -> u32 {
        self.buffer_bytes
    }

    /// The pool geometry as a pair, for a report.
    pub fn geometry(&self) -> (u32, u32) {
        (self.count, self.buffer_bytes)
    }

    /// True when the pool is registered in the kernel.
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// What the platform offers. On Windows this is always `Unsupported`.
    pub fn capability(&self) -> Capability {
        platform_capability()
    }

    /// Pin every buffer with one `io_uring_register_buffers` call.
    ///
    /// A second call returns `AlreadyRegistered`. On a platform without
    /// io_uring the call returns `Unsupported` with the cause.
    pub fn register(&mut self) -> Result<(), RegBufError> {
        if self.registered {
            return Err(RegBufError::AlreadyRegistered);
        }
        // SAFETY: the handle is live and owned by this value.
        let code = unsafe { sys::sheaf_regbuf_register(self.handle) };
        match code {
            sys::OK => {
                self.registered = true;
                Ok(())
            }
            sys::EALREADY => Err(RegBufError::AlreadyRegistered),
            sys::ECAP => Err(RegBufError::Unsupported { reason: platform_reason() }),
            sys::EINVAL => Err(RegBufError::InvalidArgument("the native tier rejected the pool")),
            other => Err(RegBufError::RawErrno(other)),
        }
    }

    /// Release the kernel pin. A call on an unregistered pool does nothing.
    pub fn unregister(&mut self) -> Result<(), RegBufError> {
        if !self.registered {
            return Ok(());
        }
        // SAFETY: the handle is live and owned by this value.
        let code = unsafe { sys::sheaf_regbuf_unregister(self.handle) };
        match code {
            sys::OK => {
                self.registered = false;
                Ok(())
            }
            other => Err(RegBufError::RawErrno(other)),
        }
    }

    /// The writable bytes of buffer `index`.
    pub fn buffer(&mut self, index: u32) -> Result<&mut [u8], RegBufError> {
        if index >= self.count {
            return Err(RegBufError::BufferIndexOutOfRange { index, count: self.count });
        }
        // SAFETY: the handle is live and the index is inside the pool.
        let ptr = unsafe { sys::sheaf_regbuf_buffer(self.handle, index) };
        if ptr.is_null() {
            return Err(RegBufError::InvalidArgument("the pool holds no buffer at this index"));
        }
        // SAFETY: the C tier allocated buffer_bytes bytes at this address, and
        // the mutable borrow of self keeps the pool alive for the slice life.
        Ok(unsafe { std::slice::from_raw_parts_mut(ptr, self.buffer_bytes as usize) })
    }

    /// Submit one `IORING_OP_WRITE_FIXED` of `len` bytes from buffer `index`,
    /// then wait for the completion. Returns the byte count that the kernel
    /// reported.
    ///
    /// Buffer `index` writes at file offset `index * buffer_bytes`, so each
    /// buffer owns one slot.
    pub fn write_fixed(&mut self, index: u32, len: u32) -> Result<u32, RegBufError> {
        if index >= self.count {
            return Err(RegBufError::BufferIndexOutOfRange { index, count: self.count });
        }
        if len == 0 || len > self.buffer_bytes {
            return Err(RegBufError::InvalidArgument(
                "the length is zero or above the buffer size",
            ));
        }
        if !self.capability().is_supported() {
            return Err(RegBufError::Unsupported { reason: platform_reason() });
        }
        if !self.registered {
            return Err(RegBufError::NotRegistered);
        }
        let mut written = 0u32;
        // SAFETY: the handle is live, the index is inside the pool, and the
        // kernel writes at most buffer_bytes bytes.
        let code =
            unsafe { sys::sheaf_regbuf_write_fixed(self.handle, index, len, &mut written) };
        match code {
            sys::OK => Ok(written),
            sys::ECAP => Err(RegBufError::Unsupported { reason: platform_reason() }),
            sys::EINDEX => Err(RegBufError::BufferIndexOutOfRange { index, count: self.count }),
            other => Err(RegBufError::RawErrno(other)),
        }
    }

    /// Force the target file to stable storage. The caller needs this before a
    /// durability claim.
    pub fn fsync(&mut self) -> Result<(), RegBufError> {
        // SAFETY: the handle is live and owned by this value.
        let code = unsafe { sys::sheaf_regbuf_fsync(self.handle) };
        match code {
            sys::OK => Ok(()),
            sys::ECAP => Err(RegBufError::Unsupported { reason: platform_reason() }),
            other => Err(RegBufError::RawErrno(other)),
        }
    }
}

impl Drop for RegisteredBuffers {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        // SAFETY: the handle came from pool_create and is destroyed once. The
        // destroy call unregisters first, so no kernel pin survives.
        unsafe { sys::sheaf_regbuf_pool_destroy(self.handle) };
        self.handle = std::ptr::null_mut();
        self.registered = false;
    }
}

impl fmt::Debug for RegisteredBuffers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredBuffers")
            .field("path", &self.path)
            .field("count", &self.count)
            .field("buffer_bytes", &self.buffer_bytes)
            .field("registered", &self.registered)
            .finish()
    }
}

/// Turn a path into a C string.
fn c_path(path: &Path) -> Result<std::ffi::CString, RegBufError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| RegBufError::InvalidArgument("the path holds a zero byte"))
    }
    #[cfg(not(unix))]
    {
        let text = path
            .to_str()
            .ok_or(RegBufError::InvalidArgument("the path is not valid Unicode"))?;
        std::ffi::CString::new(text)
            .map_err(|_| RegBufError::InvalidArgument("the path holds a zero byte"))
    }
}

/// The C ABI of the compiled C11 WAL library.
#[cfg(sheaf_native_wal)]
mod sys {
    use std::ffi::c_char;
    use std::os::raw::c_int;

    /// Opaque pool owned by the C tier.
    #[repr(C)]
    pub struct SheafRegBufPool {
        _private: [u8; 0],
    }

    pub const OK: i32 = 0;
    pub const EINVAL: i32 = -1;
    pub const ECAP: i32 = -4;
    pub const EALREADY: i32 = -5;
    pub const EINDEX: i32 = -6;
    pub const CAP_IO_URING: u32 = 1;

    extern "C" {
        pub fn sheaf_regbuf_capability() -> u32;
        pub fn sheaf_regbuf_capability_reason() -> *const c_char;
        pub fn sheaf_regbuf_pool_create(
            path: *const c_char,
            buffer_count: u32,
            buffer_bytes: u32,
        ) -> *mut SheafRegBufPool;
        pub fn sheaf_regbuf_pool_destroy(pool: *mut SheafRegBufPool);
        pub fn sheaf_regbuf_register(pool: *mut SheafRegBufPool) -> c_int;
        pub fn sheaf_regbuf_unregister(pool: *mut SheafRegBufPool) -> c_int;
        pub fn sheaf_regbuf_write_fixed(
            pool: *mut SheafRegBufPool,
            index: u32,
            len: u32,
            out_written: *mut u32,
        ) -> c_int;
        pub fn sheaf_regbuf_fsync(pool: *mut SheafRegBufPool) -> c_int;
        pub fn sheaf_regbuf_buffer(pool: *mut SheafRegBufPool, index: u32) -> *mut u8;
    }
}

/// The pure-Rust adapter, used when this build did not link the C11 WAL.
///
/// It keeps the same ABI shape. The pool is a real 64-byte aligned
/// allocation, so the alignment invariant is testable on any machine. The
/// kernel registration and the fixed write report the capability as absent,
/// with the true cause. They never report success.
#[cfg(not(sheaf_native_wal))]
mod sys {
    use std::alloc::{alloc, dealloc, Layout};
    use std::ffi::c_char;

    pub const OK: i32 = 0;
    pub const EINVAL: i32 = -1;
    pub const ECAP: i32 = -4;
    pub const EALREADY: i32 = -5;
    pub const EINDEX: i32 = -6;
    pub const CAP_IO_URING: u32 = 1;

    const ALIGN: usize = 64;

    /// A pool of aligned bytes with no kernel registration.
    #[repr(C)]
    pub struct SheafRegBufPool {
        base: *mut u8,
        layout: Layout,
        count: u32,
        buffer_bytes: u32,
    }

    /// The reason string, kept as a static byte array with a terminator.
    const REASON: &[u8] =
        b"the C WAL library is not linked in this build, so no io_uring buffer can be registered\0";

    pub unsafe fn sheaf_regbuf_capability() -> u32 {
        0
    }

    pub unsafe fn sheaf_regbuf_capability_reason() -> *const c_char {
        REASON.as_ptr() as *const c_char
    }

    pub unsafe fn sheaf_regbuf_pool_create(
        _path: *const c_char,
        buffer_count: u32,
        buffer_bytes: u32,
    ) -> *mut SheafRegBufPool {
        if buffer_count == 0 || buffer_bytes == 0 || buffer_bytes as usize % ALIGN != 0 {
            return std::ptr::null_mut();
        }
        let total = match (buffer_count as usize).checked_mul(buffer_bytes as usize) {
            Some(value) => value,
            None => return std::ptr::null_mut(),
        };
        let layout = match Layout::from_size_align(total, ALIGN) {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let base = alloc(layout);
        if base.is_null() {
            return std::ptr::null_mut();
        }
        std::ptr::write_bytes(base, 0, total);
        let pool = Box::new(SheafRegBufPool {
            base,
            layout,
            count: buffer_count,
            buffer_bytes,
        });
        Box::into_raw(pool)
    }

    pub unsafe fn sheaf_regbuf_pool_destroy(pool: *mut SheafRegBufPool) {
        if pool.is_null() {
            return;
        }
        let owned = Box::from_raw(pool);
        dealloc(owned.base, owned.layout);
    }

    pub unsafe fn sheaf_regbuf_register(_pool: *mut SheafRegBufPool) -> i32 {
        ECAP
    }

    pub unsafe fn sheaf_regbuf_unregister(_pool: *mut SheafRegBufPool) -> i32 {
        OK
    }

    pub unsafe fn sheaf_regbuf_write_fixed(
        _pool: *mut SheafRegBufPool,
        _index: u32,
        _len: u32,
        _out_written: *mut u32,
    ) -> i32 {
        ECAP
    }

    pub unsafe fn sheaf_regbuf_fsync(_pool: *mut SheafRegBufPool) -> i32 {
        ECAP
    }

    pub unsafe fn sheaf_regbuf_buffer(pool: *mut SheafRegBufPool, index: u32) -> *mut u8 {
        if pool.is_null() {
            return std::ptr::null_mut();
        }
        let pool = &*pool;
        if index >= pool.count {
            return std::ptr::null_mut();
        }
        pool.base.add(index as usize * pool.buffer_bytes as usize)
    }
}
