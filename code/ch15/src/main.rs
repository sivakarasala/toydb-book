/// Chapter 15: Raft — Log Replication
/// Exercise: Build a replicated log with AppendEntries.

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry { pub term: u64, pub index: u64, pub command: String }

pub struct RaftLog { entries: Vec<LogEntry> }

impl RaftLog {
    pub fn new() -> Self { RaftLog { entries: Vec::new() } }

    /// Append a new entry.
    pub fn append(&mut self, term: u64, command: String) -> u64 {
        // TODO: Create LogEntry with next index, push to entries, return index
        todo!("Implement append")
    }

    /// Get entry at index (1-based).
    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        // TODO: Return entry at index-1
        todo!("Implement get")
    }

    /// Get term at index, or 0 if out of range.
    pub fn term_at(&self, index: u64) -> u64 {
        self.get(index).map(|e| e.term).unwrap_or(0)
    }

    /// Last index in the log.
    pub fn last_index(&self) -> u64 { self.entries.len() as u64 }

    /// AppendEntries: check consistency, then append new entries.
    pub fn append_entries(&mut self, prev_index: u64, prev_term: u64, entries: Vec<LogEntry>) -> bool {
        // TODO:
        // 1. If prev_index > 0 and term_at(prev_index) != prev_term, return false
        // 2. Remove any entries after prev_index (truncate)
        // 3. Append new entries
        // 4. Return true
        todo!("Implement append_entries")
    }
}

/// Shared state for multi-threaded access.
pub type SharedLog = Arc<Mutex<RaftLog>>;

fn main() {
    println!("=== Chapter 15: Raft Log Replication ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_get() {
        let mut log = RaftLog::new();
        let idx = log.append(1, "SET x 1".into());
        assert_eq!(idx, 1);
        assert_eq!(log.get(1).unwrap().command, "SET x 1");
    }

    #[test]
    fn test_append_entries_success() {
        let mut log = RaftLog::new();
        log.append(1, "cmd1".into());
        let entries = vec![LogEntry { term: 1, index: 2, command: "cmd2".into() }];
        assert!(log.append_entries(1, 1, entries));
        assert_eq!(log.last_index(), 2);
    }

    #[test]
    fn test_append_entries_consistency_fail() {
        let mut log = RaftLog::new();
        log.append(1, "cmd1".into());
        // Wrong prev_term
        let entries = vec![LogEntry { term: 2, index: 2, command: "cmd2".into() }];
        assert!(!log.append_entries(1, 99, entries));
    }

    #[test]
    fn test_shared_log() {
        let log: SharedLog = Arc::new(Mutex::new(RaftLog::new()));
        let log2 = log.clone();
        std::thread::spawn(move || {
            log2.lock().unwrap().append(1, "from thread".into());
        }).join().unwrap();
        assert_eq!(log.lock().unwrap().last_index(), 1);
    }
}
