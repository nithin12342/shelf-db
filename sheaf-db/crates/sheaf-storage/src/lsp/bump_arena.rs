// crates/sheaf-storage/src/lsp/bump_arena.rs — ephemeral monotonic bump allocator.
// Node H3 of IMPLEMENTATION_PLAN_HARDWARE_MAPPED_2026-10-27.md.
// Used for transient pullback staging instead of the general allocator.
pub struct BumpArena {
    buf: Vec<u8>,
    off: usize,
}

impl BumpArena {
    pub fn new(capacity: usize) -> Self {
        Self { buf: vec![0u8; capacity], off: 0 }
    }
    pub fn alloc(&mut self, n: usize) -> Option<&mut [u8]> {
        if self.off + n > self.buf.len() {
            return None;
        }
        let start = self.off;
        self.off += n;
        Some(&mut self.buf[start..start + n])
    }
    pub fn reset(&mut self) {
        self.off = 0;
    }
    pub fn used(&self) -> usize {
        self.off
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alloc_and_reset() {
        let mut a = BumpArena::new(64);
        let s = a.alloc(8).unwrap();
        s[0] = 42;
        assert_eq!(a.used(), 8);
        a.reset();
        assert_eq!(a.used(), 0);
        let s2 = a.alloc(8).unwrap();
        let _ = s2;
    }
    #[test]
    fn overflow() {
        let mut a = BumpArena::new(8);
        assert!(a.alloc(16).is_none());
    }
}
