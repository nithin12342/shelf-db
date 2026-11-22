// cluster.rs - deterministic message delivery and quorum rules (NF-07).
//
// The cluster delivers messages in a fixed order and never uses a timer. A
// partition removes links; healing restores them. A commit needs a majority
// of the full membership, so a minority partition cannot commit.

use super::log::Entry;
use super::state::{Node, Role};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsensusError {
    /// The target node does not exist.
    UnknownNode,
    /// A write reached a node that is not the leader.
    NotLeader,
    /// The candidate did not receive a majority of votes.
    NotEnoughVotes,
    /// A write did not reach a majority.
    NoQuorum,
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusError::UnknownNode => f.write_str("unknown node"),
            ConsensusError::NotLeader => f.write_str("the node is not the leader"),
            ConsensusError::NotEnoughVotes => f.write_str("the candidate has no majority of votes"),
            ConsensusError::NoQuorum => f.write_str("the write has no quorum"),
        }
    }
}

impl std::error::Error for ConsensusError {}

type Result<T> = std::result::Result<T, ConsensusError>;

pub struct Cluster {
    nodes: BTreeMap<u64, Node>,
    reachable: BTreeMap<u64, BTreeSet<u64>>,
}

impl Cluster {
    pub fn new(ids: &[u64]) -> Self {
        let nodes: BTreeMap<u64, Node> = ids.iter().map(|id| (*id, Node::new(*id))).collect();
        let all: BTreeSet<u64> = ids.iter().copied().collect();
        let reachable = nodes.keys().map(|id| (*id, all.clone())).collect();
        Self { nodes, reachable }
    }

    pub fn node(&self, id: u64) -> Result<&Node> {
        self.nodes.get(&id).ok_or(ConsensusError::UnknownNode)
    }

    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    pub fn majority(&self) -> usize {
        self.nodes.len() / 2 + 1
    }

    /// Restrict links to members of the same group.
    pub fn partition(&mut self, groups: &[Vec<u64>]) {
        for peers in self.reachable.values_mut() {
            peers.clear();
        }
        for group in groups {
            for id in group {
                if let Some(peers) = self.reachable.get_mut(id) {
                    peers.extend(group.iter().copied());
                }
            }
        }
    }

    pub fn heal(&mut self) {
        let all: BTreeSet<u64> = self.nodes.keys().copied().collect();
        for peers in self.reachable.values_mut() {
            *peers = all.clone();
        }
    }

    /// Restart a node from its durable state. It comes back as a follower.
    pub fn restart(&mut self, id: u64) -> Result<()> {
        let node = self.nodes.get(&id).ok_or(ConsensusError::UnknownNode)?;
        let persisted = node.persist();
        self.nodes.insert(id, Node::restore(id, persisted));
        Ok(())
    }

    /// Run one deterministic election. The candidate votes for itself first.
    pub fn elect(&mut self, id: u64) -> Result<()> {
        let candidate = self.nodes.get_mut(&id).ok_or(ConsensusError::UnknownNode)?;
        candidate.start_election();
        let term = candidate.current_term;
        let last_index = candidate.log.len();
        let last_term = candidate.log.last_term();
        let peers: Vec<u64> = self.reachable.get(&id).ok_or(ConsensusError::UnknownNode)?.iter().copied().collect();

        let mut grants = 1usize;
        for peer in peers {
            if peer == id {
                continue;
            }
            let granted = self.nodes.get_mut(&peer).ok_or(ConsensusError::UnknownNode)?
                .request_vote(term, id, last_index, last_term);
            if granted {
                grants += 1;
            }
        }
        if grants < self.majority() {
            return Err(ConsensusError::NotEnoughVotes);
        }
        if let Some(leader) = self.nodes.get_mut(&id) {
            if leader.current_term == term {
                leader.role = Role::Leader;
            }
        }
        Ok(())
    }

    /// Replicate one command. Returns the commit index on the leader.
    pub fn write(&mut self, leader_id: u64, command: &str) -> Result<usize> {
        let leader = self.nodes.get_mut(&leader_id).ok_or(ConsensusError::UnknownNode)?;
        if leader.role != Role::Leader {
            return Err(ConsensusError::NotLeader);
        }
        let term = leader.current_term;
        let entry = Entry { term, command: command.to_owned() };
        let commit_before = leader.commit_index;
        let prev_index = leader.log.len();
        let prev_term = leader.log.last_term();
        let peers: Vec<u64> = self.reachable.get(&leader_id).ok_or(ConsensusError::UnknownNode)?.iter().copied().collect();

        // Replicate first. An entry that does not reach a majority is never
        // appended, on the leader or on a follower, so a failed write cannot
        // leave a divergent suffix behind.
        let mut acked = Vec::new();
        let mut touched: Vec<(u64, usize, usize)> = Vec::new();
        for &peer in &peers {
            if peer == leader_id {
                continue;
            }
            let target = self.nodes.get_mut(&peer).ok_or(ConsensusError::UnknownNode)?;
            let old_len = target.log.len();
            let old_commit = target.commit_index;
            let ok = target.append_entries(term, leader_id, prev_index, prev_term, std::slice::from_ref(&entry), commit_before);
            if ok {
                acked.push(peer);
                touched.push((peer, old_len, old_commit));
            } else {
                target.log.entries.truncate(old_len);
                target.commit_index = old_commit;
            }
        }
        if acked.len() + 1 < self.majority() {
            for (peer, old_len, old_commit) in touched {
                if let Some(node) = self.nodes.get_mut(&peer) {
                    node.log.entries.truncate(old_len);
                    node.commit_index = old_commit;
                }
            }
            return Err(ConsensusError::NoQuorum);
        }
        let leader = self.nodes.get_mut(&leader_id).ok_or(ConsensusError::UnknownNode)?;
        leader.log.entries.push(entry);
        leader.commit_index = leader.log.len();
        let commit_index = leader.commit_index;
        for peer in acked {
            if let Some(node) = self.nodes.get_mut(&peer) {
                node.commit_index = node.commit_index.max(commit_index.min(node.log.len()));
            }
        }
        Ok(commit_index)
    }

    /// One leader per term. Returns the ids that claim leadership in `term`.
    pub fn leaders_in(&self, term: u64) -> Vec<u64> {
        self.nodes.values().filter(|node| node.role == Role::Leader && node.current_term == term).map(|node| node.id).collect()
    }

    pub fn committed_entry(&self, id: u64, index: usize) -> Option<&Entry> {
        let node = self.nodes.get(&id)?;
        if node.commit_index >= index {
            node.log.entries.get(index - 1)
        } else {
            None
        }
    }
}
