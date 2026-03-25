/// Chapter 1: What Is a Database?
/// Exercise: Build a HashMap-based key-value store with a Value enum.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use std::collections::HashMap;
use std::fmt;

// ── Value Enum ──────────────────────────────────────────────────────

/// A dynamically-typed value that our database can store.
/// TODO: Add variants: Null, Bool(bool), Int(i64), Float(f64), String(String)
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
        // TODO: Display each variant nicely
        // Null → "NULL", Bool → "true"/"false", Int → number, Float → number, String → the string
        match self {
            Value::String(s) => write!(f, "{}", s),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "NULL"),
        }
    }
}

// ── KvStore ─────────────────────────────────────────────────────────

/// A simple key-value store backed by a HashMap.
pub struct KvStore {
    data: HashMap<String, Value>,
}

impl KvStore {
    pub fn new() -> Self {
        KvStore {
            data: HashMap::new(),
        }
    }

    /// Insert or update a key-value pair.
    pub fn set(&mut self, key: &str, value: Value) {
        // TODO: Insert the key-value pair into self.data
        self.data.insert(key.to_string(), value);
    }

    /// Retrieve a value by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        // TODO: Look up the key in self.data
        self.data.get(key)
    }

    /// Delete a key-value pair. Returns the old value if it existed.
    pub fn delete(&mut self, key: &str) -> Option<Value> {
        // TODO: Remove the key from self.data
        self.data.remove(key)
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        // TODO: Return the count of entries
        self.data.len()
    }
}

fn main() {
    println!("=== Chapter 1: What Is a Database? ===");
    println!("Exercise: Implement a HashMap-based KV store with Value enum.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut store = KvStore::new();
        store.set("name", Value::String("Alice".to_string()));
        assert_eq!(store.get("name"), Some(&Value::String("Alice".to_string())));
    }

    #[test]
    fn test_get_missing_key() {
        let store = KvStore::new();
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn test_overwrite() {
        let mut store = KvStore::new();
        store.set("x", Value::Int(1));
        store.set("x", Value::Int(2));
        assert_eq!(store.get("x"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_delete() {
        let mut store = KvStore::new();
        store.set("key", Value::Bool(true));
        let old = store.delete("key");
        assert_eq!(old, Some(Value::Bool(true)));
        assert_eq!(store.get("key"), None);
    }

    #[test]
    fn test_delete_missing() {
        let mut store = KvStore::new();
        assert_eq!(store.delete("nope"), None);
    }

    #[test]
    fn test_len() {
        let mut store = KvStore::new();
        assert_eq!(store.len(), 0);
        store.set("a", Value::Int(1));
        store.set("b", Value::Int(2));
        assert_eq!(store.len(), 2);
        store.delete("a");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::Null), "NULL");
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::String("hello".to_string())), "hello");
    }
}
