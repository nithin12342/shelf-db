// raft.rs - deterministic Raft safety scenarios (NF-07).
//
// Five scenarios: election, replication, a minority write, restart, and
// partition healing. Every expected result is stated in the test.

use sheaf_runtime::consensus::{Cluster, ConsensusError, Role};

fn five() -> Cluster {
    Cluster::new(&[1, 2, 3, 4, 5])
}

#[test]
fn an_election_produces_exactly_one_leader_per_term() {
    let mut cluster = five();
    cluster.elect(1).expect("elect 1");
    assert_eq!(cluster.node(1).unwrap().role, Role::Leader);
    let term = cluster.node(1).unwrap().current_term;
    assert_eq!(cluster.leaders_in(term), vec![1]);
    assert!(cluster.node(2).unwrap().voted_for == Some(1));
}

#[test]
fn a_committed_entry_is_replicated_to_a_majority() {
    let mut cluster = five();
    cluster.elect(1).expect("elect");
    let index = cluster.write(1, "insert A").expect("write");
    assert_eq!(index, 1);
    for id in [1, 2, 3, 4, 5] {
        let entry = cluster.committed_entry(id, 1).expect("committed everywhere");
        assert_eq!(entry.command, "insert A");
    }
    assert!(cluster.node(1).unwrap().commit_index == 1);
}

#[test]
fn a_minority_partition_cannot_commit() {
    let mut cluster = five();
    cluster.elect(1).expect("elect");
    cluster.partition(&[vec![1, 2], vec![3, 4, 5]]);
    assert_eq!(
        cluster.write(1, "minority"),
        Err(ConsensusError::NoQuorum)
    );
    assert_eq!(cluster.node(1).unwrap().commit_index, 0);
    assert!(cluster.committed_entry(1, 1).is_none());
}

#[test]
fn a_restart_preserves_the_term_vote_and_log() {
    let mut cluster = five();
    cluster.elect(1).expect("elect");
    cluster.write(1, "durable").expect("write");
    let before = cluster.node(1).unwrap().persist();
    cluster.restart(1).expect("restart");
    let after = cluster.node(1).unwrap();
    assert_eq!(after.role, Role::Follower);
    assert_eq!(after.current_term, before.current_term);
    assert_eq!(after.voted_for, before.voted_for);
    assert_eq!(after.log, before.log);
    assert_eq!(after.commit_index, before.commit_index);
}

#[test]
fn a_healed_partition_commits_again() {
    let mut cluster = five();
    cluster.elect(1).expect("elect");
    cluster.partition(&[vec![1, 2], vec![3, 4, 5]]);
    assert_eq!(cluster.write(1, "blocked"), Err(ConsensusError::NoQuorum));
    cluster.heal();
    let index = cluster.write(1, "after heal").expect("write");
    assert_eq!(index, 1, "the rolled-back minority write left no committed entry");
    for id in 1..=5 {
        assert!(cluster.committed_entry(id, 1).is_some(), "node {id} has the entry");
    }
    assert_eq!(cluster.leaders_in(cluster.node(1).unwrap().current_term), vec![1]);
}
