/// Chapter 17: Integration — SQL over Raft
/// Exercise: Wire together Storage, SQL, and Raft layers via the module system.

use std::collections::HashMap;

// ── Storage Layer ──────────────────────────────────────────────────

pub trait Storage {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: String);
    fn delete(&mut self, key: &str) -> bool;
    fn scan(&self, prefix: &str) -> Vec<(String, String)>;
}

pub struct MemoryStorage {
    data: HashMap<String, String>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage { data: HashMap::new() }
    }
}

impl Storage for MemoryStorage {
    fn get(&self, key: &str) -> Option<String> { self.data.get(key).cloned() }
    fn set(&mut self, key: &str, value: String) { self.data.insert(key.to_string(), value); }
    fn delete(&mut self, key: &str) -> bool { self.data.remove(key).is_some() }
    fn scan(&self, prefix: &str) -> Vec<(String, String)> {
        self.data.iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

// ── SQL Layer (simplified) ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SqlResult {
    Ok,
    Value(String),
    Rows(Vec<Vec<String>>),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Command {
    Get { key: String },
    Set { key: String, value: String },
    Delete { key: String },
    Scan { prefix: String },
}

pub fn parse_command(input: &str) -> Result<Command, String> {
    let parts: Vec<&str> = input.trim().splitn(3, ' ').collect();
    match parts.first().map(|s| s.to_uppercase()).as_deref() {
        Some("GET") => {
            if parts.len() < 2 { return Err("GET requires a key".into()); }
            Ok(Command::Get { key: parts[1].to_string() })
        }
        Some("SET") => {
            if parts.len() < 3 { return Err("SET requires key and value".into()); }
            Ok(Command::Set { key: parts[1].to_string(), value: parts[2].to_string() })
        }
        Some("DELETE") => {
            if parts.len() < 2 { return Err("DELETE requires a key".into()); }
            Ok(Command::Delete { key: parts[1].to_string() })
        }
        Some("SCAN") => {
            let prefix = if parts.len() >= 2 { parts[1].to_string() } else { String::new() };
            Ok(Command::Scan { prefix })
        }
        _ => Err(format!("Unknown command: {}", input)),
    }
}

// ── Raft Layer (simplified — single-node) ──────────────────────────

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: String,
}

pub struct RaftNode {
    term: u64,
    log: Vec<LogEntry>,
    commit_index: u64,
}

impl RaftNode {
    pub fn new() -> Self {
        RaftNode { term: 1, log: Vec::new(), commit_index: 0 }
    }

    pub fn propose(&mut self, command: String) -> LogEntry {
        let index = self.log.len() as u64 + 1;
        let entry = LogEntry { term: self.term, index, command };
        self.log.push(entry.clone());
        self.commit_index = index; // single-node: auto-commit
        entry
    }

    pub fn committed_entries(&self) -> &[LogEntry] {
        &self.log[..self.commit_index as usize]
    }

    pub fn term(&self) -> u64 { self.term }
    pub fn commit_index(&self) -> u64 { self.commit_index }
}

// ── Server: Wires All Layers Together ──────────────────────────────

pub struct Server<S: Storage> {
    storage: S,
    raft: RaftNode,
}

impl<S: Storage> Server<S> {
    pub fn new(storage: S) -> Self {
        // TODO: Create a Server with the given storage and a new RaftNode
        todo!("Implement Server::new")
    }

    /// Execute a SQL command: parse → propose to Raft → apply to storage
    pub fn execute(&mut self, input: &str) -> SqlResult {
        // TODO:
        // 1. Parse the input using parse_command()
        // 2. On parse error, return SqlResult::Error
        // 3. Propose the raw input string to Raft via self.raft.propose()
        // 4. Apply the command to self.storage:
        //    - Get: return SqlResult::Value or SqlResult::Error("Key not found")
        //    - Set: return SqlResult::Ok
        //    - Delete: return SqlResult::Ok (even if key didn't exist)
        //    - Scan: return SqlResult::Rows (each row is [key, value])
        todo!("Implement Server::execute")
    }

    pub fn raft_status(&self) -> (u64, u64) {
        // TODO: Return (term, commit_index) from the Raft node
        todo!("Implement Server::raft_status")
    }
}

fn main() {
    println!("=== Chapter 17: Integration ===");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut server = Server::new(MemoryStorage::new());
        assert_eq!(server.execute("SET name Alice"), SqlResult::Ok);
        assert_eq!(server.execute("GET name"), SqlResult::Value("Alice".into()));
    }

    #[test]
    fn test_delete() {
        let mut server = Server::new(MemoryStorage::new());
        server.execute("SET x 42");
        assert_eq!(server.execute("DELETE x"), SqlResult::Ok);
        match server.execute("GET x") {
            SqlResult::Error(_) => {} // expected
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_raft_log_grows() {
        let mut server = Server::new(MemoryStorage::new());
        server.execute("SET a 1");
        server.execute("SET b 2");
        server.execute("SET c 3");
        let (term, commit) = server.raft_status();
        assert_eq!(term, 1);
        assert_eq!(commit, 3);
    }

    #[test]
    fn test_scan() {
        let mut server = Server::new(MemoryStorage::new());
        server.execute("SET user:1 Alice");
        server.execute("SET user:2 Bob");
        server.execute("SET item:1 Widget");
        match server.execute("SCAN user:") {
            SqlResult::Rows(rows) => assert_eq!(rows.len(), 2),
            other => panic!("Expected Rows, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_error() {
        let mut server = Server::new(MemoryStorage::new());
        match server.execute("INVALID cmd") {
            SqlResult::Error(_) => {} // expected
            other => panic!("Expected Error, got {:?}", other),
        }
    }
}
