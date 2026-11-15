// complex.rs - the cellular sheaf over a 1-dimensional cell complex.
//
// Spec Pillar 8 models a repository as a sheaf F over a graph X:
//   0-cells (vertices) are atomic entities. Each carries a stalk F(v) = R^d.
//   1-cells (edges) are dependencies. Each carries a stalk F(e) = R^k.
//   Restriction maps F_{u<=e}: F(u) -> F(e) are linear morphisms.
//
// The coboundary operator delta: C^0 -> C^1 couples neighboring stalks:
//   (delta x)_e = R_v x_v - R_u x_u
// A 0-cochain x with delta x = 0 is a global section. The spec defines a
// commit as exactly such a zero-coboundary section.
//
// Cell ids share one space: vertex i has id i, edge j has id n + j. The star
// neighborhood used by LCA-CW (Pillar 10) is computed over this id space.

use std::collections::BTreeSet;

/// A linear restriction map stored as a row-major matrix. It maps a vertex
/// stalk of `cols` lanes to an edge stalk of `rows` lanes.
#[derive(Clone, Debug, PartialEq)]
pub struct Restriction {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f32>,
}

impl Restriction {
    /// Build a map from explicit row-major data. Panics on a length mismatch,
    /// because a malformed matrix is a programming error, not runtime input.
    pub fn new(rows: usize, cols: usize, data: Vec<f32>) -> Self {
        assert_eq!(data.len(), rows * cols, "restriction data length mismatch");
        Self { rows, cols, data }
    }

    /// The identity map on a d-dimensional stalk.
    pub fn identity(d: usize) -> Self {
        let mut data = vec![0.0; d * d];
        for i in 0..d {
            data[i * d + i] = 1.0;
        }
        Self { rows: d, cols: d, data }
    }

    /// A scaled identity map w * I on a d-dimensional stalk.
    pub fn scale(d: usize, w: f32) -> Self {
        let mut r = Self::identity(d);
        for v in r.data.iter_mut() {
            *v *= w;
        }
        r
    }

    /// Apply the map to a vertex stalk. Returns an edge stalk of `rows` lanes.
    pub fn apply(&self, x: &[f32]) -> Vec<f32> {
        assert_eq!(x.len(), self.cols, "restriction input dimension mismatch");
        if self.cols == 0 {
            return vec![0.0; self.rows];
        }
        // Each row of the matrix dots with the input stalk. chunks_exact walks
        // the rows without manual indexing.
        self.data
            .chunks_exact(self.cols)
            .map(|row| row.iter().zip(x).map(|(w, v)| w * v).sum())
            .collect()
    }
}

/// A 1-cell (edge) from vertex u to vertex v with two restriction maps.
#[derive(Clone, Debug)]
pub struct Edge {
    pub u: usize,
    pub v: usize,
    pub ru: Restriction,
    pub rv: Restriction,
}

/// The cell complex topology. Stalk values live in sections, not here.
#[derive(Clone, Debug)]
pub struct Complex {
    vertex_dims: Vec<usize>,
    edges: Vec<Edge>,
    vertex_offsets: Vec<usize>,
    c0_len: usize,
}

impl Complex {
    /// Start a complex with the given vertex stalk dimensions.
    pub fn new(vertex_dims: Vec<usize>) -> Self {
        let mut offsets = Vec::with_capacity(vertex_dims.len());
        let mut total = 0;
        for &d in &vertex_dims {
            offsets.push(total);
            total += d;
        }
        Self { vertex_dims, edges: Vec::new(), vertex_offsets: offsets, c0_len: total }
    }

    /// Add an edge and return its edge index. The restriction column counts
    /// must match the endpoint stalk dimensions.
    pub fn add_edge(&mut self, u: usize, v: usize, ru: Restriction, rv: Restriction) -> usize {
        assert!(u < self.vertex_count() && v < self.vertex_count(), "edge endpoint out of range");
        assert_eq!(ru.cols, self.vertex_dims[u], "ru cols must match stalk dim of u");
        assert_eq!(rv.cols, self.vertex_dims[v], "rv cols must match stalk dim of v");
        assert_eq!(ru.rows, rv.rows, "both restrictions must target the same edge stalk");
        let index = self.edges.len();
        self.edges.push(Edge { u, v, ru, rv });
        index
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_dims.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn vertex_dim(&self, i: usize) -> usize {
        self.vertex_dims[i]
    }

    pub fn edge_dim(&self, e: usize) -> usize {
        self.edges[e].ru.rows
    }

    /// Total lanes in a 0-cochain (the concatenated vertex stalks).
    pub fn c0_len(&self) -> usize {
        self.c0_len
    }

    /// Total lanes in a 1-cochain (the concatenated edge stalks).
    pub fn c1_len(&self) -> usize {
        self.edges.iter().map(|e| e.ru.rows).sum()
    }

    /// Borrow the stalk of vertex i from a 0-cochain.
    pub fn vertex_stalk<'a>(&self, x: &'a [f32], i: usize) -> &'a [f32] {
        let start = self.vertex_offsets[i];
        &x[start..start + self.vertex_dims[i]]
    }

    /// The coboundary delta x of a 0-cochain. The result is a 1-cochain.
    pub fn coboundary(&self, x: &[f32]) -> Vec<f32> {
        assert_eq!(x.len(), self.c0_len, "0-cochain length mismatch");
        let mut out = Vec::with_capacity(self.c1_len());
        for e in &self.edges {
            let xu = self.vertex_stalk(x, e.u);
            let xv = self.vertex_stalk(x, e.v);
            let pu = e.ru.apply(xu);
            let pv = e.rv.apply(xv);
            for k in 0..e.ru.rows {
                out.push(pv[k] - pu[k]);
            }
        }
        out
    }

    /// The Dirichlet energy ||delta x||^2. A global section has zero energy.
    pub fn energy(&self, x: &[f32]) -> f32 {
        self.coboundary(x).iter().map(|v| v * v).sum()
    }

    /// True when x is a global section within `tol` (delta x = 0).
    pub fn is_global_section(&self, x: &[f32], tol: f32) -> bool {
        self.energy(x) <= tol
    }

    /// The star neighborhood of a cell, as a set of cell ids. For a vertex it
    /// contains the vertex, its incident edges, and the neighboring vertices.
    /// For an edge it contains the edge and its two endpoints. LCA-CW uses the
    /// intersection of two stars to decide parallel versus chronological writes.
    pub fn star(&self, cell_id: u64) -> BTreeSet<u64> {
        let n = self.vertex_count() as u64;
        let mut set = BTreeSet::new();
        if cell_id < n {
            let v = cell_id as usize;
            set.insert(cell_id);
            for (j, e) in self.edges.iter().enumerate() {
                if e.u == v || e.v == v {
                    set.insert(n + j as u64);
                    set.insert(e.u as u64);
                    set.insert(e.v as u64);
                }
            }
        } else {
            let e = &self.edges[(cell_id - n) as usize];
            set.insert(cell_id);
            set.insert(e.u as u64);
            set.insert(e.v as u64);
        }
        set
    }

    /// Deterministic topology bytes for content addressing. Encodes vertex
    /// dimensions and every edge endpoint plus restriction weights.
    pub fn topology_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.vertex_count() as u64).to_le_bytes());
        for &d in &self.vertex_dims {
            out.extend_from_slice(&(d as u64).to_le_bytes());
        }
        out.extend_from_slice(&(self.edge_count() as u64).to_le_bytes());
        for e in &self.edges {
            out.extend_from_slice(&(e.u as u64).to_le_bytes());
            out.extend_from_slice(&(e.v as u64).to_le_bytes());
            for r in [&e.ru, &e.rv] {
                out.extend_from_slice(&(r.rows as u64).to_le_bytes());
                out.extend_from_slice(&(r.cols as u64).to_le_bytes());
                for &w in &r.data {
                    out.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two vertices with 1-dimensional stalks joined by one edge with identity
    // restrictions. delta x = x_v - x_u.
    fn line() -> Complex {
        let mut c = Complex::new(vec![1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c
    }

    #[test]
    fn coboundary_of_line() {
        let c = line();
        assert_eq!(c.c0_len(), 2);
        assert_eq!(c.c1_len(), 1);
        assert_eq!(c.coboundary(&[2.0, 5.0]), vec![3.0]);
        assert_eq!(c.coboundary(&[4.0, 4.0]), vec![0.0]);
    }

    #[test]
    fn global_section_has_zero_energy() {
        let c = line();
        assert!(c.is_global_section(&[3.0, 3.0], 1e-6));
        assert!(!c.is_global_section(&[3.0, 4.0], 1e-6));
        assert_eq!(c.energy(&[1.0, 3.0]), 4.0);
    }

    #[test]
    fn restriction_apply_and_scale() {
        let r = Restriction::scale(2, 2.0);
        assert_eq!(r.apply(&[1.0, 3.0]), vec![2.0, 6.0]);
        let id = Restriction::identity(2);
        assert_eq!(id.apply(&[1.0, 3.0]), vec![1.0, 3.0]);
    }

    #[test]
    fn star_of_vertex_includes_neighbors() {
        // Triangle: vertices 0,1,2 and edges 0-1,1-2,2-0.
        let mut c = Complex::new(vec![1, 1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(1, 2, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(2, 0, Restriction::identity(1), Restriction::identity(1));
        let s = c.star(0);
        // vertex 0, its incident edges 3 (0-1) and 5 (2-0), neighbors 1 and 2.
        // Edge 4 (1-2) is not incident to vertex 0, so it is excluded.
        assert_eq!(s, [0u64, 1, 2, 3, 5].into_iter().collect());
    }

    #[test]
    fn disjoint_vertices_have_disjoint_stars() {
        // Two separate edges: 0-1 and 2-3. Stars of 0 and 2 do not overlap.
        let mut c = Complex::new(vec![1, 1, 1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(2, 3, Restriction::identity(1), Restriction::identity(1));
        let a = c.star(0);
        let b = c.star(2);
        assert!(a.is_disjoint(&b));
    }

    #[test]
    fn topology_bytes_are_deterministic() {
        assert_eq!(line().topology_bytes(), line().topology_bytes());
    }
}