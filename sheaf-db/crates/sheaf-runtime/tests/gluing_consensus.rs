// gluing_consensus.rs - gluing consensus gate (C-11).
//
// Spec Pillar 7 forbids Raft voting loops. This test drives the gluing port
// with real boundary data. The expected result is delta s = 0 and equal
// values at every shared boundary cell. No leader and no vote exist.

use sheaf_runtime::consensus::{Boundary, GluingCluster, GluingError};

#[test]
fn two_nodes_reach_one_boundary_section() {
    let mut cluster = GluingCluster::new(&[1, 2]);
    cluster
        .add_boundary(Boundary { left: 1, right: 2, cell: "b0".into() })
        .expect("boundary");
    cluster.set(1, "b0", 3.0).expect("set left");
    cluster.set(2, "b0", -1.0).expect("set right");
    assert!(cluster.coboundary_norm() > 1.0);
    let rounds = cluster.agree(1e-9, 64).expect("agree");
    assert!(rounds < 64, "the run must converge");
    assert!(cluster.coboundary_norm() < 1e-9);
    assert!(cluster.agrees());
}

#[test]
fn a_three_node_chain_reaches_zero_coboundary() {
    let mut cluster = GluingCluster::new(&[1, 2, 3]);
    for (left, right) in [(1, 2), (2, 3)] {
        cluster
            .add_boundary(Boundary { left, right, cell: format!("b{left}{right}") })
            .expect("boundary");
    }
    cluster.set(1, "b12", 10.0).expect("set");
    cluster.set(2, "b12", 0.0).expect("set");
    cluster.set(2, "b23", 0.0).expect("set");
    cluster.set(3, "b23", -4.0).expect("set");
    let rounds = cluster.agree(1e-9, 128).expect("agree");
    assert!(rounds < 128);
    assert!(cluster.coboundary_norm() < 1e-9);
    assert!(cluster.agrees());
}

#[test]
fn bad_inputs_are_rejected() {
    let mut cluster = GluingCluster::new(&[1]);
    assert_eq!(
        cluster.add_boundary(Boundary { left: 1, right: 9, cell: "x".into() }),
        Err(GluingError::UnknownNode)
    );
    assert_eq!(cluster.set(9, "x", 1.0), Err(GluingError::UnknownNode));
    assert_eq!(cluster.agree(0.0, 10), Err(GluingError::BadTolerance));
    assert_eq!(cluster.agree(1e-6, 0), Err(GluingError::BadIterationLimit));
}
