/// Chapter 17: Integration — SQL over Raft — SOLUTION

use std::collections::HashMap;

pub trait Storage {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: String);
    fn delete(&mut self, key: &str) -> bool;
    fn scan(&self, prefix: &str) -> Vec<(String, String)>;
}

pub struct MemoryStorage { data: HashMap<String, String> }

impl MemoryStorage {
    pub fn new() -> Self { MemoryStorage { data: HashMap::new() } }
}

impl Storage for MemoryStorage {
    fn get(&self, key: &str) -> Option<String> { self.data.get(key).cloned() }
    fn set(&mut self, key: &str, value: String) { self.data.insert(key.to_string(), value); }
    fn delete(&mut self, key: &str) -> bool { self.data.remove(key).is_some() }
    fn scan(&self, prefix: &str) -> Vec<(String, String)> {
        self.data.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlResult { Ok, Value(String), Rows(Vec<Vec<String>>), Error(String) }

#[derive(Debug, Clone)]
pub enum Command { Get { key: String }, Set { key: String, value: String }, Delete { key: String }, Scan { prefix: String } }

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

#[derive(Debug, Clone)]
pub struct LogEntry { pub term: u64, pub index: u64, pub command: String }

pub struct RaftNode { term: u64, log: Vec<LogEntry>, commit_index: u64 }

impl RaftNode {
    pub fn new() -> Self { RaftNode { term: 1, log: Vec::new(), commit_index: 0 } }
    pub fn propose(&mut self, command: String) -> LogEntry {
        let index = self.log.len() as u64 + 1;
        let entry = LogEntry { term: self.term, index, command };
        self.log.push(entry.clone());
        self.commit_index = index;
        entry
    }
    pub fn term(&self) -> u64 { self.term }
    pub fn commit_index(&self) -> u64 { self.commit_index }
}

pub struct Server<S: Storage> { storage: S, raft: RaftNode }

impl<S: Storage> Server<S> {
    pub fn new(storage: S) -> Self {
        Server { storage, raft: RaftNode::new() }
    }

    pub fn execute(&mut self, input: &str) -> SqlResult {
        let cmd = match parse_command(input) {
            Ok(c) => c,
            Err(e) => return SqlResult::Error(e),
        };
        self.raft.propose(input.to_string());
        match cmd {
            Command::Get { key } => match self.storage.get(&key) {
                Some(v) => SqlResult::Value(v),
                None => SqlResult::Error("Key not found".into()),
            },
            Command::Set { key, value } => { self.storage.set(&key, value); SqlResult::Ok }
            Command::Delete { key } => { self.storage.delete(&key); SqlResult::Ok }
            Command::Scan { prefix } => {
                let pairs = self.storage.scan(&prefix);
                let rows: Vec<Vec<String>> = pairs.into_iter().map(|(k, v)| vec![k, v]).collect();
                SqlResult::Rows(rows)
            }
        }
    }

    pub fn raft_status(&self) -> (u64, u64) {
        (self.raft.term(), self.raft.commit_index())
    }
}

fn main() {
    println!("=== Chapter 17: Integration — Solution ===");
    let mut server = Server::new(MemoryStorage::new());
    println!("{:?}", server.execute("SET name Alice"));
    println!("{:?}", server.execute("GET name"));
    println!("Raft: {:?}", server.raft_status());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_set_and_get() {
        let mut s = Server::new(MemoryStorage::new());
        assert_eq!(s.execute("SET name Alice"), SqlResult::Ok);
        assert_eq!(s.execute("GET name"), SqlResult::Value("Alice".into()));
    }

    #[test] fn test_delete() {
        let mut s = Server::new(MemoryStorage::new());
        s.execute("SET x 42");
        assert_eq!(s.execute("DELETE x"), SqlResult::Ok);
        assert!(matches!(s.execute("GET x"), SqlResult::Error(_)));
    }

    #[test] fn test_raft_log_grows() {
        let mut s = Server::new(MemoryStorage::new());
        s.execute("SET a 1"); s.execute("SET b 2"); s.execute("SET c 3");
        assert_eq!(s.raft_status(), (1, 3));
    }
}
