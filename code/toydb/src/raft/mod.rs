/// Raft Consensus Layer (Ch14-16)
///
/// Ch14: Leader election state machine
/// Ch15: Log replication
/// Ch16: Write-ahead log for durability
///
/// This is a simplified single-node Raft that logs commands
/// for durability and replay. In a real system, this would
/// replicate across multiple nodes.

pub mod wal;

use crate::error::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: String,
}

/// A single-node Raft that provides command ordering and durability.
pub struct RaftLog {
    term: u64,
    log: Vec<LogEntry>,
    commit_index: u64,
    wal: Option<wal::WalWriter>,
}

impl RaftLog {
    /// Create an in-memory-only Raft log (no persistence).
    pub fn new() -> Self {
        RaftLog { term: 1, log: Vec::new(), commit_index: 0, wal: None }
    }

    /// Create a persistent Raft log backed by a WAL file.
    pub fn with_wal(path: &Path) -> Result<Self> {
        // Replay existing entries
        let entries = if path.exists() {
            wal::WalReader::read_all(path)?
        } else {
            Vec::new()
        };
        let commit_index = entries.len() as u64;
        let term = entries.last().map(|e| e.term).unwrap_or(1);
        let writer = wal::WalWriter::new(path)?;
        Ok(RaftLog {
            term,
            log: entries,
            commit_index,
            wal: Some(writer),
        })
    }

    /// Propose a command — appends to log, writes to WAL if enabled.
    pub fn propose(&mut self, command: String) -> Result<&LogEntry> {
        let index = self.log.len() as u64 + 1;
        let entry = LogEntry { term: self.term, index, command };

        if let Some(ref mut wal) = self.wal {
            wal.append(&entry)?;
        }

        self.log.push(entry);
        self.commit_index = index;
        Ok(self.log.last().unwrap())
    }

    pub fn term(&self) -> u64 { self.term }
    pub fn commit_index(&self) -> u64 { self.commit_index }
    pub fn entries(&self) -> &[LogEntry] { &self.log }

    /// Get all committed commands (for replay).
    pub fn committed_commands(&self) -> Vec<String> {
        self.log[..self.commit_index as usize]
            .iter()
            .map(|e| e.command.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propose_and_commit() {
        let mut raft = RaftLog::new();
        raft.propose("SET x 1".into()).unwrap();
        raft.propose("SET y 2".into()).unwrap();
        assert_eq!(raft.commit_index(), 2);
        assert_eq!(raft.entries().len(), 2);
    }

    #[test]
    fn test_wal_persistence() {
        let path = std::env::temp_dir().join("toydb_raft_test");
        let _ = std::fs::remove_file(&path);

        {
            let mut raft = RaftLog::with_wal(&path).unwrap();
            raft.propose("SET a 1".into()).unwrap();
            raft.propose("SET b 2".into()).unwrap();
        }

        // Reopen — should recover entries
        let raft = RaftLog::with_wal(&path).unwrap();
        assert_eq!(raft.entries().len(), 2);
        assert_eq!(raft.entries()[0].command, "SET a 1");

        let _ = std::fs::remove_file(&path);
    }
}
