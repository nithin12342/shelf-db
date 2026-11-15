// repo.rs - the version-control composition root (spec Pillars 8 and 10).
//
// This module wires the focused modules together behind a small public API:
//   complex  - the topology and coboundary operator
//   section  - the zero-coboundary commit rule
//   merkle   - the content-addressed commit DAG
//   lca      - star-neighborhood write routing
//   chart    - chronological delta chains
//   diff     - time-travel diff and cohomological merge
//
// Callers depend on Repository, not on the concrete modules. That keeps the
// dependency direction inward (the SOLID dependency-inversion rule).

use crate::chart::{ChartError, ChartRegistry};
use crate::complex::Complex;
use crate::diff::{self, MergeConflict};
use crate::lca::CollisionInspector;
use crate::merkle::{Hash, MerkleDag};
use crate::section::Section;

/// The default tolerance for the zero-coboundary global-section test.
pub const DEFAULT_TOL: f32 = 1e-6;

/// An error produced by a repository operation.
#[derive(Clone, Debug, PartialEq)]
pub enum RepoError {
    /// A proposed commit is not a global section. Carries its energy.
    NotGlobalSection { energy: f32 },
    /// A referenced commit hash is not in the DAG.
    UnknownCommit,
    /// A merge produced a cohomological obstruction.
    Merge(MergeConflict),
    /// A chronological chart append failed.
    Chart(ChartError),
}

impl From<ChartError> for RepoError {
    fn from(e: ChartError) -> Self {
        RepoError::Chart(e)
    }
}

/// A sheaf-based repository: a complex, a commit DAG, and per-cell charts.
pub struct Repository {
    complex: Complex,
    dag: MerkleDag,
    charts: ChartRegistry,
    head: Option<Hash>,
    tol: f32,
}

impl Repository {
    /// Create a repository over a complex with an empty history.
    pub fn new(complex: Complex) -> Self {
        let charts = ChartRegistry::new(&complex);
        Self { complex, dag: MerkleDag::new(), charts, head: None, tol: DEFAULT_TOL }
    }

    /// The underlying topology.
    pub fn complex(&self) -> &Complex {
        &self.complex
    }

    /// The current head commit, if any.
    pub fn head(&self) -> Option<Hash> {
        self.head
    }

    /// Read access to the commit DAG.
    pub fn dag(&self) -> &MerkleDag {
        &self.dag
    }

    /// Record a commit. The section must be a zero-coboundary global section.
    /// The new commit's parent is the current head, and the head advances.
    pub fn commit(&mut self, section: &Section) -> Result<Hash, RepoError> {
        let energy = section.energy(&self.complex);
        if energy > self.tol {
            return Err(RepoError::NotGlobalSection { energy });
        }
        let hash = self.dag.commit(&self.complex, section, self.head);
        self.head = Some(hash);
        Ok(hash)
    }

    /// Restore the section stored at a commit.
    pub fn checkout(&self, hash: &Hash) -> Result<Section, RepoError> {
        self.dag.get(hash).map(|n| n.section.clone()).ok_or(RepoError::UnknownCommit)
    }

    /// Point a branch name at a commit (O(1) metadata).
    pub fn branch(&mut self, name: &str, hash: Hash) {
        self.dag.branch(name, hash);
    }

    /// Point a tag name at a commit (O(1) metadata).
    pub fn tag(&mut self, name: &str, hash: Hash) {
        self.dag.tag(name, hash);
    }

    /// Resolve a branch name to its commit hash.
    pub fn resolve_branch(&self, name: &str) -> Option<Hash> {
        self.dag.resolve_branch(name)
    }

    /// Merge two commits over a common base. On success the merged global
    /// section is committed and its hash returned. On obstruction the conflict
    /// is returned and no commit is recorded.
    pub fn merge_commits(&mut self, base: &Hash, a: &Hash, b: &Hash) -> Result<Hash, RepoError> {
        let base_s = self.checkout(base)?;
        let a_s = self.checkout(a)?;
        let b_s = self.checkout(b)?;
        match diff::merge(&self.complex, &base_s, &a_s, &b_s, self.tol) {
            Ok(merged) => self.commit(&merged),
            Err(conflict) => Err(RepoError::Merge(conflict)),
        }
    }

    /// Plan concurrent writes into parallel waves using star-neighborhood
    /// overlap. Orthogonal cells share a wave. Conflicting cells serialize.
    pub fn plan_writes(&self, cells: &[u64]) -> Vec<Vec<u64>> {
        CollisionInspector::new(&self.complex).parallel_waves(cells)
    }

    /// Append a delta to a cell's chronological chart (the shared-state path).
    pub fn append_delta(&mut self, cell: usize, timestamp: u64, values: Vec<f32>) -> Result<(), RepoError> {
        self.charts.append(cell, timestamp, values)?;
        Ok(())
    }

    /// Fold every cell chart at logical time `at` into a section.
    pub fn snapshot(&self, at: u64) -> Section {
        self.charts.snapshot_section(&self.complex, at)
    }

    /// Fold the charts at time `at` and commit the result if it is global.
    pub fn commit_snapshot(&mut self, at: u64) -> Result<Hash, RepoError> {
        let section = self.snapshot(at);
        self.commit(&section)
    }

    /// The algebraic time-travel diff delta(s_b - s_a) between two commits.
    pub fn diff_commits(&self, a: &Hash, b: &Hash) -> Result<Vec<f32>, RepoError> {
        let sa = self.checkout(a)?;
        let sb = self.checkout(b)?;
        Ok(diff::coboundary_diff(&self.complex, &sa, &sb))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Restriction;

    fn line() -> Complex {
        let mut c = Complex::new(vec![1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c
    }

    fn sec(c: &Complex, v: Vec<f32>) -> Section {
        Section::new(c, v).unwrap()
    }

    #[test]
    fn commit_rejects_non_global_section() {
        let mut repo = Repository::new(line());
        let bad = sec(repo.complex(), vec![1.0, 2.0]);
        let err = repo.commit(&bad).unwrap_err();
        assert_eq!(err, RepoError::NotGlobalSection { energy: 1.0 });
        assert!(repo.head().is_none());
    }

    #[test]
    fn commit_and_checkout_round_trip() {
        let mut repo = Repository::new(line());
        let s = sec(repo.complex(), vec![3.0, 3.0]);
        let h = repo.commit(&s).unwrap();
        assert_eq!(repo.head(), Some(h));
        assert_eq!(repo.checkout(&h).unwrap().values, vec![3.0, 3.0]);
    }

    #[test]
    fn branch_and_tag_resolve() {
        let mut repo = Repository::new(line());
        let h = repo.commit(&Section::constant(repo.complex(), 1.0)).unwrap();
        repo.branch("main", h);
        repo.tag("v1", h);
        assert_eq!(repo.resolve_branch("main"), Some(h));
        assert_eq!(repo.dag().resolve_tag("v1"), Some(h));
    }

    #[test]
    fn merge_commits_succeeds_for_compatible_branches() {
        let mut repo = Repository::new(line());
        let base = repo.commit(&sec(repo.complex(), vec![0.0, 0.0])).unwrap();
        let a = repo.commit(&sec(repo.complex(), vec![1.0, 1.0])).unwrap();
        let b = repo.commit(&sec(repo.complex(), vec![2.0, 2.0])).unwrap();
        let merged = repo.merge_commits(&base, &a, &b).unwrap();
        assert_eq!(repo.checkout(&merged).unwrap().values, vec![3.0, 3.0]);
    }

    #[test]
    fn chart_deltas_fold_into_a_commit() {
        let mut repo = Repository::new(line());
        repo.append_delta(0, 1, vec![4.0]).unwrap();
        repo.append_delta(1, 1, vec![4.0]).unwrap();
        let snap = repo.snapshot(1);
        assert_eq!(snap.values, vec![4.0, 4.0]);
        let h = repo.commit_snapshot(1).unwrap();
        assert_eq!(repo.checkout(&h).unwrap().values, vec![4.0, 4.0]);
    }

    #[test]
    fn plan_writes_separates_orthogonal_cells() {
        let mut c = Complex::new(vec![1, 1, 1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(2, 3, Restriction::identity(1), Restriction::identity(1));
        let repo = Repository::new(c);
        assert_eq!(repo.plan_writes(&[0, 2]), vec![vec![0u64, 2]]);
    }

    #[test]
    fn diff_commits_isolates_the_change() {
        let mut repo = Repository::new(line());
        let a = repo.commit(&sec(repo.complex(), vec![1.0, 1.0])).unwrap();
        let b = repo.commit(&sec(repo.complex(), vec![4.0, 4.0])).unwrap();
        // difference [3,3] is global, so its coboundary is zero.
        assert_eq!(repo.diff_commits(&a, &b).unwrap(), vec![0.0]);
    }
}