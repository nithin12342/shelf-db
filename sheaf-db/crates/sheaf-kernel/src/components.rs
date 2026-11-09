// crates/sheaf-kernel/src/components.rs — P2 prerequisite: b₀ via union-find
// b₀ = number of connected components of the cell complex (constant sheaf).
// Cells sharing a hypercell face-list are united; isolated cells are their
// own component. Union-find with path halving: O(n·α(n)). This is H₀ of the
// complex, NOT H⁰ of a general sheaf (global sections need the solve) —
// the distinction is load-bearing, hence the name.
fn find(parent: &mut [u32], mut a: u32) -> u32 {
    while parent[a as usize] != a {
        parent[a as usize] = parent[parent[a as usize] as usize];
        a = parent[a as usize];
    }
    a
}

pub fn betti_zero(n: usize, faces_per_op: &[Vec<u32>]) -> usize {
    let mut parent: Vec<u32> = (0..n as u32).collect();
    for faces in faces_per_op {
        for w in faces.windows(2) {
            let (ra, rb) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            if ra != rb {
                parent[ra as usize] = rb;
            }
        }
    }
    let mut roots = 0usize;
    for i in 0..n as u32 {
        if find(&mut parent, i) == i {
            roots += 1;
        }
    }
    roots
}
