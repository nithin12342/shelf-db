// state.rs - one deterministic Raft node (NF-07).
//
// A node owns its term, its vote, and its log. It does not send messages; the
// cluster does. This keeps the role transitions testable without time or a
// socket.

use super::log::{Entry, Log};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    RequestVote {
        term: u64,
        candidate: u64,
        last_index: usize,
        last_term: u64,
    },
    VoteResponse {
        term: u64,
        granted: bool,
    },
    AppendEntries {
        term: u64,
        leader: u64,
        prev_index: usize,
        prev_term: u64,
        entries: Vec<Entry>,
        leader_commit: usize,
    },
    Ack {
        term: u64,
        success: bool,
        match_index: usize,
    },
}

/// The durable state a node restores after a restart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persisted {
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Log,
    pub commit_index: usize,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: u64,
    pub role: Role,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Log,
    pub commit_index: usize,
}

impl Node {
    pub fn new(id: u64) -> Self {
        Self { id, role: Role::Follower, current_term: 0, voted_for: None, log: Log::default(), commit_index: 0 }
    }

    pub fn restore(id: u64, state: Persisted) -> Self {
        Self {
            id,
            role: Role::Follower,
            current_term: state.current_term,
            voted_for: state.voted_for,
            log: state.log,
            commit_index: state.commit_index,
        }
    }

    pub fn persist(&self) -> Persisted {
        Persisted {
            current_term: self.current_term,
            voted_for: self.voted_for,
            log: self.log.clone(),
            commit_index: self.commit_index,
        }
    }

    /// Become a candidate for the next term and vote for self.
    pub fn start_election(&mut self) {
        self.current_term += 1;
        self.role = Role::Candidate;
        self.voted_for = Some(self.id);
    }

    /// Grant a vote once per term, and only to a candidate whose log is at
    /// least as up to date as this node's log.
    pub fn request_vote(&mut self, term: u64, candidate: u64, last_index: usize, last_term: u64) -> bool {
        if term < self.current_term {
            return false;
        }
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
            self.role = Role::Follower;
        }
        if self.voted_for.is_some_and(|id| id != candidate) {
            return false;
        }
        let own = (self.log.last_term(), self.log.len());
        if (last_term, last_index) < own {
            return false;
        }
        self.voted_for = Some(candidate);
        true
    }

    /// Accept an append from a current leader. A newer term forces Follower.
    pub fn append_entries(
        &mut self,
        term: u64,
        leader: u64,
        prev_index: usize,
        prev_term: u64,
        entries: &[Entry],
        leader_commit: usize,
    ) -> bool {
        if term < self.current_term {
            return false;
        }
        let _ = leader;
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }
        self.role = Role::Follower;
        if !self.log.append_after(prev_index, prev_term, entries) {
            return false;
        }
        let last_new = self.log.len();
        self.commit_index = self.commit_index.max(leader_commit.min(last_new));
        true
    }

    /// Append a command on the leader only.
    pub fn append(&mut self, command: &str) -> Entry {
        let entry = Entry { term: self.current_term, command: command.to_owned() };
        self.log.entries.push(entry.clone());
        entry
    }
}
