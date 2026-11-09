//! kernel_parity - bit-level and tolerance parity gates for the Tier 1 port.
//!
//! The oracle is a dense materialized coboundary matrix D built straight
//! from the definition. Every backend must match D s, D^T v, and D^T D s.
//! This is the "permanent naive scan oracle" pattern from spec Pillar 5:
//! the optimized path is forever compared against the unfolded reference.
//!
//! When the native library is linked, both adapters run through the same
//! assertions, which makes this file the native-vs-fallback parity gate.

use sheaf_native::kernel::{fallback::FallbackKernel, ComplexSpec, KernelBackend, KernelError};

// Deterministic pseudo-random generator so failures reproduce exactly.
struct Lcg(u64);
impl Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

struct Fixture {
    src: Vec<u32>,
    dst: Vec<u32>,
    r_src: Vec<f32>,
    r_dst: Vec<f32>,
    s: Vec<f32>,
    v: Vec<f32>,
    vertices: u32,
    dim: u32,
    identity: bool,
}

impl Fixture {
    fn new(seed: u64, identity: bool) -> Self {
        let mut rng = Lcg(seed);
        let vertices = 9u32;
        let dim = 3u32;
        let edges = 14usize;
        let mut src = Vec::new();
        let mut dst = Vec::new();
        for _ in 0..edges {
            let a = rng.next_u32() % vertices;
            let mut b = rng.next_u32() % vertices;
            if b == a {
                b = (a + 1) % vertices;
            }
            src.push(a);
            dst.push(b);
        }
        let blocks = edges * dim as usize * dim as usize;
        let r_src: Vec<f32> = (0..blocks).map(|_| rng.next_f32()).collect();
        let r_dst: Vec<f32> = (0..blocks).map(|_| rng.next_f32()).collect();
        let s: Vec<f32> = (0..vertices as usize * dim as usize).map(|_| rng.next_f32()).collect();
        let v: Vec<f32> = (0..edges * dim as usize).map(|_| rng.next_f32()).collect();
        Self { src, dst, r_src, r_dst, s, v, vertices, dim, identity }
    }

    fn spec(&self) -> ComplexSpec<'_> {
        ComplexSpec {
            num_vertices: self.vertices,
            stalk_dim: self.dim,
            edge_src: &self.src,
            edge_dst: &self.dst,
            r_src: if self.identity { None } else { Some(&self.r_src) },
            r_dst: if self.identity { None } else { Some(&self.r_dst) },
        }
    }

    // Dense coboundary matrix D, shape (E*d) x (V*d), from the definition.
    fn dense_delta(&self) -> Vec<f32> {
        let d = self.dim as usize;
        let rows = self.src.len() * d;
        let cols = self.vertices as usize * d;
        let mut m = vec![0.0f32; rows * cols];
        for (e, (&a, &b)) in self.src.iter().zip(&self.dst).enumerate() {
            for i in 0..d {
                for j in 0..d {
                    let (rd, rs) = if self.identity {
                        (if i == j { 1.0 } else { 0.0 }, if i == j { 1.0 } else { 0.0 })
                    } else {
                        (self.r_dst[e * d * d + i * d + j], self.r_src[e * d * d + i * d + j])
                    };
                    m[(e * d + i) * cols + b as usize * d + j] += rd;
                    m[(e * d + i) * cols + a as usize * d + j] -= rs;
                }
            }
        }
        m
    }
}

fn dense_matvec(m: &[f32], rows: usize, cols: usize, x: &[f32]) -> Vec<f32> {
    (0..rows)
        .map(|r| (0..cols).map(|c| m[r * cols + c] * x[c]).sum())
        .collect()
}

fn dense_transpose_matvec(m: &[f32], rows: usize, cols: usize, x: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0f32; cols];
    for r in 0..rows {
        for c in 0..cols {
            out[c] += m[r * cols + c] * x[r];
        }
    }
    out
}

fn assert_close(a: &[f32], b: &[f32], tol: f32, ctx: &str) {
    assert_eq!(a.len(), b.len(), "{ctx}: length mismatch");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(
            (x - y).abs() <= tol * (1.0 + x.abs().max(y.abs())),
            "{ctx}: slot {i}: {x} vs {y}"
        );
    }
}

// Every available backend runs through the same gate.
fn backends() -> Vec<Box<dyn KernelBackend>> {
    let mut list: Vec<Box<dyn KernelBackend>> = vec![Box::new(FallbackKernel)];
    #[cfg(sheaf_native_kernel)]
    list.push(Box::new(sheaf_native::kernel::native::NativeKernel));
    list
}

#[test]
fn coboundary_matches_dense_oracle() {
    for identity in [true, false] {
        let fx = Fixture::new(42, identity);
        let cx = fx.spec();
        let d = fx.dim as usize;
        let rows = fx.src.len() * d;
        let cols = fx.vertices as usize * d;
        let dense = fx.dense_delta();
        let want = dense_matvec(&dense, rows, cols, &fx.s);
        for be in backends() {
            let mut got = vec![0.0f32; rows];
            be.coboundary(&cx, &fx.s, &mut got).unwrap();
            assert_close(&got, &want, 1e-4, &format!("{} coboundary identity={identity}", be.name()));
        }
    }
}

#[test]
fn adjoint_matches_dense_oracle_and_inner_product_identity() {
    for identity in [true, false] {
        let fx = Fixture::new(7, identity);
        let cx = fx.spec();
        let d = fx.dim as usize;
        let rows = fx.src.len() * d;
        let cols = fx.vertices as usize * d;
        let dense = fx.dense_delta();
        let want = dense_transpose_matvec(&dense, rows, cols, &fx.v);
        for be in backends() {
            let mut got = vec![0.0f32; cols];
            be.coboundary_adjoint(&cx, &fx.v, &mut got).unwrap();
            assert_close(&got, &want, 1e-4, &format!("{} adjoint identity={identity}", be.name()));

            // <delta s, v> must equal <s, delta* v> for the same backend.
            let mut ds = vec![0.0f32; rows];
            be.coboundary(&cx, &fx.s, &mut ds).unwrap();
            let lhs: f32 = ds.iter().zip(&fx.v).map(|(a, b)| a * b).sum();
            let rhs: f32 = fx.s.iter().zip(&got).map(|(a, b)| a * b).sum();
            assert!((lhs - rhs).abs() <= 1e-3 * (1.0 + lhs.abs()), "{} adjoint identity", be.name());
        }
    }
}

#[test]
fn laplacian_and_energy_match_dense_oracle() {
    for identity in [true, false] {
        let fx = Fixture::new(99, identity);
        let cx = fx.spec();
        let d = fx.dim as usize;
        let rows = fx.src.len() * d;
        let cols = fx.vertices as usize * d;
        let dense = fx.dense_delta();
        let ds = dense_matvec(&dense, rows, cols, &fx.s);
        let want = dense_transpose_matvec(&dense, rows, cols, &ds);
        let want_energy: f32 = ds.iter().map(|x| x * x).sum();
        for be in backends() {
            let mut scratch = vec![0.0f32; rows];
            let mut got = vec![0.0f32; cols];
            be.laplacian_apply(&cx, &fx.s, &mut scratch, &mut got).unwrap();
            assert_close(&got, &want, 1e-3, &format!("{} laplacian identity={identity}", be.name()));
            let energy = be.energy(&cx, &fx.s, &mut scratch);
            assert!(
                (energy - want_energy).abs() <= 1e-3 * (1.0 + want_energy),
                "{} energy identity={identity}: {energy} vs {want_energy}",
                be.name()
            );
        }
    }
}

#[test]
fn xor_delta_roundtrip_and_commutativity() {
    let mut rng = Lcg(5);
    let base: Vec<u8> = (0..257).map(|_| rng.next_u32() as u8).collect();
    let target: Vec<u8> = (0..257).map(|_| rng.next_u32() as u8).collect();
    let mid: Vec<u8> = (0..257).map(|_| rng.next_u32() as u8).collect();
    for be in backends() {
        let mut delta = vec![0u8; base.len()];
        be.xor_encode(&base, &target, &mut delta).unwrap();
        let mut restored = vec![0u8; base.len()];
        be.xor_fold(&base, &[&delta], &mut restored).unwrap();
        assert_eq!(restored, target, "{} xor roundtrip", be.name());

        // Two-step fold must be order independent (commutative cochains).
        let mut d1 = vec![0u8; base.len()];
        let mut d2 = vec![0u8; base.len()];
        be.xor_encode(&base, &mid, &mut d1).unwrap();
        be.xor_encode(&mid, &target, &mut d2).unwrap();
        let mut ab = vec![0u8; base.len()];
        let mut ba = vec![0u8; base.len()];
        be.xor_fold(&base, &[&d1, &d2], &mut ab).unwrap();
        be.xor_fold(&base, &[&d2, &d1], &mut ba).unwrap();
        assert_eq!(ab, target, "{} xor chain", be.name());
        assert_eq!(ab, ba, "{} xor commutativity", be.name());
    }
}

#[test]
fn morton_keys_roundtrip() {
    for be in backends() {
        assert_eq!(be.morton_encode(0, 0), 0);
        assert_eq!(be.morton_encode(1, 0), 1);
        assert_eq!(be.morton_encode(0, 1), 2);
        assert_eq!(be.morton_encode(1, 1), 3);
        assert_eq!(be.morton_encode(2, 0), 4);
        let mut rng = Lcg(11);
        for _ in 0..200 {
            let x = rng.next_u32();
            let y = rng.next_u32();
            assert_eq!(be.morton_decode(be.morton_encode(x, y)), (x, y), "{} morton", be.name());
        }
    }
}

#[test]
fn tiled_matvec_matches_naive() {
    let mut rng = Lcg(21);
    let (rows, cols) = (130usize, 193usize); // deliberately not tile multiples
    let m: Vec<f32> = (0..rows * cols).map(|_| rng.next_f32()).collect();
    let x: Vec<f32> = (0..cols).map(|_| rng.next_f32()).collect();
    let want = dense_matvec(&m, rows, cols, &x);
    for be in backends() {
        let mut got = vec![0.0f32; rows];
        be.matvec_tiled(&m, rows as u32, cols as u32, &x, &mut got).unwrap();
        assert_close(&got, &want, 1e-3, &format!("{} matvec", be.name()));
    }
}

#[test]
fn validity_compaction_matches_bit_filter() {
    let mut rng = Lcg(33);
    let count = 137usize; // straddles three 64-bit words with a partial tail
    let values: Vec<f32> = (0..count).map(|i| i as f32).collect();
    let mut mask = vec![0u64; count.div_ceil(64)];
    for i in 0..count {
        if rng.next_u32() % 3 != 0 {
            mask[i / 64] |= 1u64 << (i % 64);
        }
    }
    let want: Vec<f32> = values
        .iter()
        .enumerate()
        .filter(|(i, _)| (mask[i / 64] >> (i % 64)) & 1 == 1)
        .map(|(_, v)| *v)
        .collect();
    for be in backends() {
        let mut out = vec![0.0f32; count];
        let kept = be.compact_valid(&values, &mask, &mut out);
        assert_eq!(kept, want.len(), "{} compact count", be.name());
        assert_eq!(&out[..kept], &want[..], "{} compact values", be.name());
    }
}

#[test]
fn chebyshev_reduces_residual_on_consistent_system() {
    let fx = Fixture::new(77, false);
    let cx = fx.spec();
    for be in backends() {
        let n = cx.vertex_floats();
        let m = cx.edge_floats();
        // Build a consistent right-hand side b = L x_true so a solution
        // exists even though L has a nontrivial kernel (global sections).
        let mut scratch = vec![0.0f32; m];
        let mut x_true = fx.s.clone();
        let mut b = vec![0.0f32; n];
        be.laplacian_apply(&cx, &x_true, &mut scratch, &mut b).unwrap();

        // Power iteration estimates lambda_max for the spectral window.
        let mut probe = vec![0.0f32; n];
        let mut lmax = 0.0f32;
        for i in 0..n {
            probe[i] = 1.0 + (i % 5) as f32 * 0.1;
        }
        for _ in 0..40 {
            let mut lp = vec![0.0f32; n];
            be.laplacian_apply(&cx, &probe, &mut scratch, &mut lp).unwrap();
            let norm = lp.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm < 1e-12 {
                break;
            }
            lmax = norm;
            for i in 0..n {
                probe[i] = lp[i] / norm;
            }
        }
        let lmax = lmax * 1.2 + 1e-3; // safety margin above the estimate

        let mut x = vec![0.0f32; n];
        let mut arena = vec![0.0f32; be.chebyshev_scratch_floats(&cx)];
        let iters = be
            .chebyshev_solve(&cx, &b, &mut x, 600, 1e-5, 0.0, lmax, &mut arena)
            .unwrap_or_else(|e| panic!("{} chebyshev failed: {e}", be.name()));
        let mut lx = vec![0.0f32; n];
        be.laplacian_apply(&cx, &x, &mut scratch, &mut lx).unwrap();
        let resid: f32 = lx.iter().zip(&b).map(|(a, c)| (a - c) * (a - c)).sum::<f32>().sqrt();
        let bnorm: f32 = b.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            resid <= 5e-4 * bnorm.max(1e-6),
            "{} chebyshev residual {resid} vs |b| {bnorm} after {iters} iters",
            be.name()
        );
        // x_true is one valid solution. The solver may differ by a kernel
        // component, so compare operator images, not the vectors themselves.
        let mut l_true = vec![0.0f32; n];
        x_true = fx.s.clone();
        be.laplacian_apply(&cx, &x_true, &mut scratch, &mut l_true).unwrap();
        assert_close(&lx, &l_true, 1e-3, &format!("{} chebyshev image", be.name()));
    }
}

#[test]
fn invalid_arguments_are_rejected() {
    let fx = Fixture::new(1, true);
    let cx = fx.spec();
    for be in backends() {
        let mut small = [0.0f32; 1];
        assert_eq!(
            be.coboundary(&cx, &fx.s, &mut small),
            Err(KernelError::InvalidArgument),
            "{} short output",
            be.name()
        );
        let mut scratch = [0.0f32; 1];
        let mut out = vec![0.0f32; cx.vertex_floats()];
        assert_eq!(
            be.laplacian_apply(&cx, &fx.s, &mut scratch, &mut out),
            Err(KernelError::ScratchTooSmall),
            "{} short scratch",
            be.name()
        );
    }
}