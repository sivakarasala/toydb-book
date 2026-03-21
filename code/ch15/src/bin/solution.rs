/// Chapter 15: Raft Log Replication — SOLUTION

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry { pub term: u64, pub index: u64, pub command: String }

pub struct RaftLog { entries: Vec<LogEntry> }

impl RaftLog {
    pub fn new() -> Self { RaftLog { entries: Vec::new() } }

    pub fn append(&mut self, term: u64, command: String) -> u64 {
        let index = self.entries.len() as u64 + 1;
        self.entries.push(LogEntry { term, index, command });
        index
    }

    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 { return None; }
        self.entries.get((index - 1) as usize)
    }

    pub fn term_at(&self, index: u64) -> u64 { self.get(index).map(|e| e.term).unwrap_or(0) }
    pub fn last_index(&self) -> u64 { self.entries.len() as u64 }

    pub fn append_entries(&mut self, prev_index: u64, prev_term: u64, entries: Vec<LogEntry>) -> bool {
        if prev_index > 0 && self.term_at(prev_index) != prev_term { return false; }
        self.entries.truncate(prev_index as usize);
        self.entries.extend(entries);
        true
    }
}

pub type SharedLog = Arc<Mutex<RaftLog>>;

fn main() {
    println!("=== Chapter 15: Raft Log — Solution ===");
    let mut log = RaftLog::new();
    log.append(1, "SET x 1".into());
    log.append(1, "SET y 2".into());
    log.append(2, "SET x 3".into());
    for i in 1..=log.last_index() {
        let e = log.get(i).unwrap();
        println!("  [{}] term={} cmd={}", e.index, e.term, e.command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_append() { let mut l = RaftLog::new(); assert_eq!(l.append(1, "c".into()), 1); assert_eq!(l.get(1).unwrap().command, "c"); }
    #[test] fn test_ae_ok() { let mut l = RaftLog::new(); l.append(1, "c1".into()); assert!(l.append_entries(1, 1, vec![LogEntry { term: 1, index: 2, command: "c2".into() }])); assert_eq!(l.last_index(), 2); }
    #[test] fn test_ae_fail() { let mut l = RaftLog::new(); l.append(1, "c".into()); assert!(!l.append_entries(1, 99, vec![])); }
    #[test] fn test_shared() {
        let l: SharedLog = Arc::new(Mutex::new(RaftLog::new()));
        let l2 = l.clone();
        std::thread::spawn(move || { l2.lock().unwrap().append(1, "t".into()); }).join().unwrap();
        assert_eq!(l.lock().unwrap().last_index(), 1);
    }
}
