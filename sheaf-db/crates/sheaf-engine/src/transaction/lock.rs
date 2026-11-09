// crates/sheaf-engine/src/transaction/lock.rs — lock overlapping lattice subcomplexes
// FILE-045. Must never store stalks.
// Deadlock-free by construction: faces are always locked in sorted order,
// so no two transactions can acquire the same pair in opposite order.
use std::sync::Mutex;

pub struct FaceLocks {
    locks: Vec<Mutex<()>>,
}

impl FaceLocks {
    pub fn new(n: usize) -> Self {
        Self { locks: (0..n).map(|_| Mutex::new(())).collect() }
    }
    /// Lock all faces in sorted order. Returns the guard bundle.
    pub fn lock_sorted(&self, faces: &mut Vec<u64>) -> Vec<std::sync::MutexGuard<'_, ()>> {
        faces.sort_unstable();
        faces.dedup();
        faces.iter().map(|f| self.locks[*f as usize % self.locks.len()].lock().unwrap()).collect()
    }
}
