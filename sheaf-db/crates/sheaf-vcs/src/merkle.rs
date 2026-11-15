// merkle.rs - the content-addressable Merkle-Sheaf DAG (spec Pillar 8).
//
// Versions form an immutable DAG. Every commit node carries a BLAKE3 hash:
//     Hash = H(Poset_Topology_Root || Stalk_Data_Root || Parent_Hash)
// The topology root hashes the cell complex. The stalk root hashes the section
// values. The parent links the history. Common sub-sheaves share immutable
// references, so branching and tagging are O(1) metadata operations.

use std::collections::HashMap;

use crate::complex::Complex;
use crate::section::Section;

/// A 32-byte BLAKE3 content hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// The all-zero hash, used as the parent of a genesis commit.
    pub fn zero() -> Self {
        Hash([0u8; 32])
    }

    /// Lower-case hexadecimal rendering, for logs and test assertions.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }
}

/// Hash a sequence of byte parts in order with BLAKE3.
fn hash_parts(parts: &[&[u8]]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    Hash(out)
}

/// The topology root: BLAKE3 over the deterministic complex encoding.
pub fn topology_root(c: &Complex) -> Hash {
    hash_parts(&[&c.topology_bytes()])
}

/// The stalk data root: BLAKE3 over the little-endian f32 section lanes.
pub fn stalk_root(section: &Section) -> Hash {
    let mut bytes = Vec::with_capacity(section.values.len() * 4);
    for v in &section.values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    hash_parts(&[&bytes])
}

/// The commit hash from its three components. A missing parent uses zero.
pub fn commit_hash(topology: &Hash, stalk: &Hash, parent: Option<&Hash>) -> Hash {
    let parent_bytes = parent.map(|p| p.0).unwrap_or([0u8; 32]);
    hash_parts(&[&topology.0, &stalk.0, &parent_bytes])
}

/// One immutable commit node in the DAG. It stores the section so a checkout
/// can restore the exact stalk values.
#[derive(Clone, Debug)]
pub struct CommitNode {
    pub hash: Hash,
    pub topology_root: Hash,
    pub stalk_root: Hash,
    pub parent: Option<Hash>,
    pub section: Section,
}

/// The in-memory Merkle-Sheaf DAG with named branches and tags.
#[derive(Default)]
pub struct MerkleDag {
    nodes: HashMap<Hash, CommitNode>,
    branches: HashMap<String, Hash>,
    tags: HashMap<String, Hash>,
}

impl MerkleDag {
    /// An empty DAG.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a commit. Computes the roots and the hash, stores the node, and
    /// returns the hash. The caller (repo.rs) enforces the global-section rule.
    pub fn commit(&mut self, c: &Complex, section: &Section, parent: Option<Hash>) -> Hash {
        let topology_root = topology_root(c);
        let stalk_root = stalk_root(section);
        let hash = commit_hash(&topology_root, &stalk_root, parent.as_ref());
        let node = CommitNode { hash, topology_root, stalk_root, parent, section: section.clone() };
        self.nodes.insert(hash, node);
        hash
    }

    /// Look up a commit node by hash.
    pub fn get(&self, hash: &Hash) -> Option<&CommitNode> {
        self.nodes.get(hash)
    }

    /// Number of commit nodes stored.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True when the DAG holds no commits.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Point a branch name at a commit. This is an O(1) metadata write.
    pub fn branch(&mut self, name: &str, hash: Hash) {
        self.branches.insert(name.to_string(), hash);
    }

    /// Point a tag name at a commit. This is an O(1) metadata write.
    pub fn tag(&mut self, name: &str, hash: Hash) {
        self.tags.insert(name.to_string(), hash);
    }

    /// Resolve a branch name to its commit hash.
    pub fn resolve_branch(&self, name: &str) -> Option<Hash> {
        self.branches.get(name).copied()
    }

    /// Resolve a tag name to its commit hash.
    pub fn resolve_tag(&self, name: &str) -> Option<Hash> {
        self.tags.get(name).copied()
    }

    /// True when `ancestor` is reachable from `descendant` by parent links.
    pub fn is_ancestor(&self, ancestor: &Hash, descendant: &Hash) -> bool {
        let mut current = Some(*descendant);
        while let Some(h) = current {
            if h == *ancestor {
                return true;
            }
            current = self.nodes.get(&h).and_then(|n| n.parent);
        }
        false
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

    #[test]
    fn hashing_is_deterministic() {
        let c = line();
        let s = Section::constant(&c, 3.0);
        assert_eq!(topology_root(&c), topology_root(&c));
        assert_eq!(stalk_root(&s), stalk_root(&s));
        let h1 = commit_hash(&topology_root(&c), &stalk_root(&s), None);
        let h2 = commit_hash(&topology_root(&c), &stalk_root(&s), None);
        assert_eq!(h1, h2);
        assert_eq!(h1.to_hex().len(), 64);
    }

    #[test]
    fn different_stalk_changes_the_hash() {
        let c = line();
        let a = Section::constant(&c, 1.0);
        let b = Section::constant(&c, 2.0);
        let t = topology_root(&c);
        assert_ne!(commit_hash(&t, &stalk_root(&a), None), commit_hash(&t, &stalk_root(&b), None));
    }

    #[test]
    fn different_parent_changes_the_hash() {
        let c = line();
        let s = Section::constant(&c, 1.0);
        let t = topology_root(&c);
        let st = stalk_root(&s);
        let p = Hash([7u8; 32]);
        assert_ne!(commit_hash(&t, &st, None), commit_hash(&t, &st, Some(&p)));
    }

    #[test]
    fn branch_and_tag_are_o1_metadata() {
        let c = line();
        let mut dag = MerkleDag::new();
        let h = dag.commit(&c, &Section::constant(&c, 5.0), None);
        dag.branch("main", h);
        dag.tag("v1", h);
        assert_eq!(dag.resolve_branch("main"), Some(h));
        assert_eq!(dag.resolve_tag("v1"), Some(h));
        assert_eq!(dag.len(), 1);
        assert_eq!(dag.get(&h).unwrap().section.values, vec![5.0, 5.0]);
    }

    #[test]
    fn ancestry_follows_parent_links() {
        let c = line();
        let mut dag = MerkleDag::new();
        let a = dag.commit(&c, &Section::constant(&c, 1.0), None);
        let b = dag.commit(&c, &Section::constant(&c, 2.0), Some(a));
        let d = dag.commit(&c, &Section::constant(&c, 3.0), Some(b));
        assert!(dag.is_ancestor(&a, &d));
        assert!(dag.is_ancestor(&b, &d));
        assert!(!dag.is_ancestor(&d, &a));
        assert!(dag.is_ancestor(&a, &a));
    }
}