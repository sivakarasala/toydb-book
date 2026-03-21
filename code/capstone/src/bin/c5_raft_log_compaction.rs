/// Capstone Challenge 5: Raft Log Compaction (Sliding Window)
/// Implement log compaction by snapshotting state and truncating old entries.

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub command: String,
}

pub struct CompactableLog {
    entries: Vec<LogEntry>,
    snapshot_index: u64,
    snapshot_state: Vec<(String, String)>, // key-value pairs
}

impl CompactableLog {
    pub fn new() -> Self {
        CompactableLog { entries: Vec::new(), snapshot_index: 0, snapshot_state: Vec::new() }
    }

    pub fn append(&mut self, term: u64, command: String) {
        let index = self.snapshot_index + self.entries.len() as u64 + 1;
        self.entries.push(LogEntry { index, term, command });
    }

    /// Compact the log up to (and including) the given index.
    /// Apply all SET commands to the snapshot state, then remove those entries.
    pub fn compact(&mut self, up_to_index: u64) -> Result<(), String> {
        // TODO:
        // 1. Find entries with index <= up_to_index
        // 2. For each, if command starts with "SET ", parse "SET key value" and upsert into snapshot_state
        // 3. Remove those entries from self.entries
        // 4. Update self.snapshot_index
        // Return Err if up_to_index is out of range
        todo!("Implement compact")
    }

    pub fn log_len(&self) -> usize { self.entries.len() }
    pub fn snapshot_index(&self) -> u64 { self.snapshot_index }
    pub fn snapshot_state(&self) -> &[(String, String)] { &self.snapshot_state }

    pub fn entry_at(&self, index: u64) -> Option<&LogEntry> {
        // TODO: Find an entry by its logical index
        // Remember: entries may have been compacted, so entry.index != position in vec
        todo!("Implement entry_at")
    }
}

fn main() {
    println!("Capstone Challenge 5: Raft Log Compaction");
    println!("Run `cargo test --bin c5-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact() {
        let mut log = CompactableLog::new();
        log.append(1, "SET x 1".into());
        log.append(1, "SET y 2".into());
        log.append(1, "SET z 3".into());
        log.append(2, "SET x 4".into());

        assert_eq!(log.log_len(), 4);
        log.compact(2).unwrap(); // compact first 2 entries
        assert_eq!(log.log_len(), 2); // z and x=4 remain
        assert_eq!(log.snapshot_index(), 2);
        assert_eq!(log.snapshot_state().len(), 2); // x=1, y=2
    }

    #[test]
    fn test_entry_at_after_compact() {
        let mut log = CompactableLog::new();
        log.append(1, "SET a 1".into());
        log.append(1, "SET b 2".into());
        log.append(1, "SET c 3".into());
        log.compact(1).unwrap();

        assert!(log.entry_at(1).is_none()); // compacted
        assert!(log.entry_at(2).is_some());
        assert_eq!(log.entry_at(2).unwrap().command, "SET b 2");
    }

    #[test]
    fn test_snapshot_overwrites() {
        let mut log = CompactableLog::new();
        log.append(1, "SET x 1".into());
        log.append(1, "SET x 2".into());
        log.compact(2).unwrap();
        // x should be 2 (latest value)
        let x_val = log.snapshot_state().iter().find(|(k, _)| k == "x").unwrap();
        assert_eq!(x_val.1, "2");
    }
}
