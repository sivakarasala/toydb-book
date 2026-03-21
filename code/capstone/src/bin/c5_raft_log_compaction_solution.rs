/// Capstone Challenge 5: Raft Log Compaction — SOLUTION

#[derive(Debug, Clone)]
pub struct LogEntry { pub index: u64, pub term: u64, pub command: String }

pub struct CompactableLog {
    entries: Vec<LogEntry>,
    snapshot_index: u64,
    snapshot_state: Vec<(String, String)>,
}

impl CompactableLog {
    pub fn new() -> Self { CompactableLog { entries: Vec::new(), snapshot_index: 0, snapshot_state: Vec::new() } }

    pub fn append(&mut self, term: u64, command: String) {
        let index = self.snapshot_index + self.entries.len() as u64 + 1;
        self.entries.push(LogEntry { index, term, command });
    }

    pub fn compact(&mut self, up_to_index: u64) -> Result<(), String> {
        if up_to_index <= self.snapshot_index {
            return Err("Already compacted past this index".into());
        }
        let last_entry_index = self.snapshot_index + self.entries.len() as u64;
        if up_to_index > last_entry_index {
            return Err("Index beyond log end".into());
        }

        let entries_to_compact = (up_to_index - self.snapshot_index) as usize;
        for entry in &self.entries[..entries_to_compact] {
            if entry.command.starts_with("SET ") {
                let parts: Vec<&str> = entry.command.splitn(3, ' ').collect();
                if parts.len() == 3 {
                    let key = parts[1].to_string();
                    let value = parts[2].to_string();
                    if let Some(existing) = self.snapshot_state.iter_mut().find(|(k, _)| *k == key) {
                        existing.1 = value;
                    } else {
                        self.snapshot_state.push((key, value));
                    }
                }
            }
        }

        self.entries = self.entries.split_off(entries_to_compact);
        self.snapshot_index = up_to_index;
        Ok(())
    }

    pub fn log_len(&self) -> usize { self.entries.len() }
    pub fn snapshot_index(&self) -> u64 { self.snapshot_index }
    pub fn snapshot_state(&self) -> &[(String, String)] { &self.snapshot_state }

    pub fn entry_at(&self, index: u64) -> Option<&LogEntry> {
        if index <= self.snapshot_index { return None; }
        let pos = (index - self.snapshot_index - 1) as usize;
        self.entries.get(pos)
    }
}

fn main() { println!("Capstone 5: Raft Log Compaction — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_compact() {
        let mut l = CompactableLog::new();
        l.append(1, "SET x 1".into()); l.append(1, "SET y 2".into());
        l.append(1, "SET z 3".into()); l.append(2, "SET x 4".into());
        l.compact(2).unwrap();
        assert_eq!(l.log_len(), 2); assert_eq!(l.snapshot_index(), 2);
    }
    #[test] fn test_entry_at() {
        let mut l = CompactableLog::new();
        l.append(1, "SET a 1".into()); l.append(1, "SET b 2".into()); l.append(1, "SET c 3".into());
        l.compact(1).unwrap();
        assert!(l.entry_at(1).is_none()); assert_eq!(l.entry_at(2).unwrap().command, "SET b 2");
    }
    #[test] fn test_overwrite() {
        let mut l = CompactableLog::new();
        l.append(1, "SET x 1".into()); l.append(1, "SET x 2".into());
        l.compact(2).unwrap();
        assert_eq!(l.snapshot_state().iter().find(|(k,_)| k=="x").unwrap().1, "2");
    }
}
