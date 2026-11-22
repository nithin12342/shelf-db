// log.rs - the replicated log (NF-07).
//
// An entry carries the term in which the leader appended it. A log never
// rewrites an existing entry; a conflicting append truncates the suffix.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub term: u64,
    pub command: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub entries: Vec<Entry>,
}

impl Log {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_term(&self) -> u64 {
        self.entries.last().map_or(0, |entry| entry.term)
    }

    /// Append after `prev_index`. A conflicting suffix is truncated first.
    /// False means the previous entry does not match.
    pub fn append_after(&mut self, prev_index: usize, prev_term: u64, incoming: &[Entry]) -> bool {
        if prev_index > self.entries.len() {
            return false;
        }
        if prev_index > 0 {
            let last = &self.entries[prev_index - 1];
            if last.term != prev_term {
                return false;
            }
        } else if prev_term != 0 {
            return false;
        }
        self.entries.truncate(prev_index);
        self.entries.extend_from_slice(incoming);
        true
    }

    /// The term at index, where zero means before the log starts.
    pub fn term_at(&self, index: usize) -> u64 {
        if index == 0 {
            0
        } else {
            self.entries.get(index - 1).map_or(0, |entry| entry.term)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conflicting_append_truncates_the_suffix() {
        let mut log = Log::default();
        log.append_after(0, 0, &[Entry { term: 1, command: "a".into() }, Entry { term: 1, command: "b".into() }]);
        assert!(log.append_after(1, 1, &[Entry { term: 2, command: "c".into() }]));
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries[1].command, "c");
    }

    #[test]
    fn a_bad_previous_entry_is_rejected() {
        let mut log = Log::default();
        assert!(!log.append_after(1, 9, &[]));
    }
}
