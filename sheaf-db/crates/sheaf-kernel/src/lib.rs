// sheaf-kernel root — Phase 1 exposes simd only.
// Phase 3 adds operators (coboundary/Laplacian/energy) + solvers (PCG).
pub mod operators;
pub mod simd;
pub mod solvers;
pub mod components;
