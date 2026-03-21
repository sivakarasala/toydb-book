/// Chapter 2: In-Memory Storage Engine
/// Exercise: Define a Storage trait, implement MemoryStorage, make Database generic.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(fl) => write!(f, "{fl}"),
            Value::String(s) => write!(f, "{s}"),
        }
    }
}

// ── Storage Trait ───────────────────────────────────────────────────

/// TODO: Define a Storage trait with these methods:
/// - fn set(&mut self, key: &str, value: Value)
/// - fn get(&self, key: &str) -> Option<&Value>
/// - fn delete(&mut self, key: &str) -> Option<Value>
/// - fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Value)>
pub trait Storage {
    fn set(&mut self, key: &str, value: Value);
    fn get(&self, key: &str) -> Option<&Value>;
    fn delete(&mut self, key: &str) -> Option<Value>;
    fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Value)>;
}

// ── MemoryStorage ───────────────────────────────────────────────────

/// An in-memory storage engine using BTreeMap (sorted keys).
pub struct MemoryStorage {
    data: BTreeMap<String, Value>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage {
            data: BTreeMap::new(),
        }
    }
}

impl Storage for MemoryStorage {
    fn set(&mut self, key: &str, value: Value) {
        // TODO: Insert into self.data
        todo!("Implement set")
    }

    fn get(&self, key: &str) -> Option<&Value> {
        // TODO: Look up key in self.data
        todo!("Implement get")
    }

    fn delete(&mut self, key: &str) -> Option<Value> {
        // TODO: Remove key from self.data
        todo!("Implement delete")
    }

    fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Value)> {
        // TODO: Return all key-value pairs where start <= key < end
        // Hint: use self.data.range(start.to_string()..end.to_string())
        todo!("Implement scan")
    }
}

// ── Generic Database ────────────────────────────────────────────────

/// A database that works with any storage engine.
pub struct Database<S: Storage> {
    storage: S,
}

impl<S: Storage> Database<S> {
    pub fn new(storage: S) -> Self {
        // TODO: Create Database with the given storage
        todo!("Implement new")
    }

    pub fn execute(&mut self, cmd: &str) -> String {
        // TODO: Parse simple commands:
        //   "SET key value" → store key=value (as String), return "OK"
        //   "GET key"       → return the value or "NULL"
        //   "DELETE key"    → delete key, return "OK"
        // Hint: use cmd.splitn(3, ' ') for SET, cmd.splitn(2, ' ') for GET/DELETE
        todo!("Implement execute")
    }
}

fn main() {
    println!("=== Chapter 2: In-Memory Storage Engine ===");
    println!("Exercise: Implement Storage trait, MemoryStorage, and generic Database.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage_crud() {
        let mut store = MemoryStorage::new();
        store.set("a", Value::Int(1));
        assert_eq!(store.get("a"), Some(&Value::Int(1)));
        store.delete("a");
        assert_eq!(store.get("a"), None);
    }

    #[test]
    fn test_scan_range() {
        let mut store = MemoryStorage::new();
        store.set("a", Value::Int(1));
        store.set("b", Value::Int(2));
        store.set("c", Value::Int(3));
        store.set("d", Value::Int(4));

        let results = store.scan("b", "d");
        let keys: Vec<&str> = results.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["b", "c"]);
    }

    #[test]
    fn test_generic_database() {
        let storage = MemoryStorage::new();
        let mut db = Database::new(storage);
        assert_eq!(db.execute("SET name Alice"), "OK");
        assert_eq!(db.execute("GET name"), "Alice");
        assert_eq!(db.execute("DELETE name"), "OK");
        assert_eq!(db.execute("GET name"), "NULL");
    }
}
