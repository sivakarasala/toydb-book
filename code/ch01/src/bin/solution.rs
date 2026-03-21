/// Chapter 1: What Is a Database? — SOLUTION
/// A HashMap-based key-value store with a Value enum.

use std::collections::HashMap;
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
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

pub struct KvStore {
    data: HashMap<String, Value>,
}

impl KvStore {
    pub fn new() -> Self {
        KvStore {
            data: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

fn main() {
    println!("=== Chapter 1: KV Store — Solution ===");
    let mut store = KvStore::new();
    store.set("name", Value::String("ToyDB".to_string()));
    store.set("version", Value::Int(1));
    store.set("stable", Value::Bool(true));

    println!("name    = {}", store.get("name").unwrap());
    println!("version = {}", store.get("version").unwrap());
    println!("stable  = {}", store.get("stable").unwrap());
    println!("entries = {}", store.len());

    store.delete("stable");
    println!("After delete: entries = {}", store.len());
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
