// crates/sheaf-core/src/stalk/aligned_buf.rs — allocate 64B aligned float buffers
// FILE-007. Must never define schema semantics.
use std::alloc::{alloc_zeroed, Layout};

/// 64-byte aligned float buffer (AVX-512 cache-line residency).
pub struct AlignedBuf64 {
    ptr: *mut f32,
    len: usize,
    layout: Layout,
}

impl AlignedBuf64 {
    pub fn zeros(len: usize) -> Self {
        let layout = Layout::from_size_align(len * 4, 64).expect("layout");
        let ptr = unsafe { alloc_zeroed(layout) as *mut f32 };
        assert!(!ptr.is_null());
        debug_assert_eq!(ptr as usize % 64, 0);
        Self { ptr, len, layout }
    }
    pub fn as_slice(&self) -> &[f32] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for AlignedBuf64 {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr as *mut u8, self.layout) };
    }
}
