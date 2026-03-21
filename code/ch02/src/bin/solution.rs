/// Chapter 2: In-Memory Storage Engine — SOLUTION

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

pub trait Storage {
    fn set(&mut self, key: &str, value: Value);
    fn get(&self, key: &str) -> Option<&Value>;
    fn delete(&mut self, key: &str) -> Option<Value>;
    fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Value)>;
}

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
        self.data.insert(key.to_string(), value);
    }

    fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    fn scan(&self, start: &str, end: &str) -> Vec<(&String, &Value)> {
        self.data
            .range(start.to_string()..end.to_string())
            .collect()
    }
}

pub struct Database<S: Storage> {
    storage: S,
}

impl<S: Storage> Database<S> {
    pub fn new(storage: S) -> Self {
        Database { storage }
    }

    pub fn execute(&mut self, cmd: &str) -> String {
        let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
        match parts[0] {
            "SET" if parts.len() == 3 => {
                self.storage
                    .set(parts[1], Value::String(parts[2].to_string()));
                "OK".to_string()
            }
            "GET" if parts.len() == 2 => match self.storage.get(parts[1]) {
                Some(v) => format!("{v}"),
                None => "NULL".to_string(),
            },
            "DELETE" if parts.len() == 2 => {
                self.storage.delete(parts[1]);
                "OK".to_string()
            }
            _ => "ERROR: unknown command".to_string(),
        }
    }
}

fn main() {
    println!("=== Chapter 2: Storage Engine — Solution ===");
    let storage = MemoryStorage::new();
    let mut db = Database::new(storage);

    println!("{}", db.execute("SET name ToyDB"));
    println!("{}", db.execute("SET version 1"));
    println!("name = {}", db.execute("GET name"));
    println!("version = {}", db.execute("GET version"));
    println!("{}", db.execute("DELETE version"));
    println!("version = {}", db.execute("GET version"));
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
