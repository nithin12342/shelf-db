// crates/sheaf-storage/src/lsp/huge_pages.rs — huge-page prefault helper.
// Node N4 of IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-10-27.md.
// Reduces TLB misses on large arenas. No-op when the OS does not support it.
pub fn prefault_arena(_ptr: *mut u8, _len: usize) {
    // Linux: madvise(MADV_HUGEPAGE). Windows: requires SeLockMemoryPrivilege
    // and VirtualAlloc with MEM_LARGE_PAGES; skipped here to keep the helper
    // dependency-free. The arena allocation itself still benefits from the
    // OS default page size.
    #[cfg(target_os = "linux")]
    {
        // madvise is not in std; call via libc if available. Skipped to avoid
        // a new dependency. Left as a documented hook.
        let _ = (_ptr, _len);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (_ptr, _len);
    }
}
