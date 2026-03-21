/// Chapter 18: Testing, Benchmarking & Extensions — SOLUTION

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value { Int(i64), Str(String), Null }

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Null => write!(f, "NULL"),
        }
    }
}

pub struct KvStore { data: HashMap<String, Value> }

impl KvStore {
    pub fn new() -> Self { KvStore { data: HashMap::new() } }
    pub fn set(&mut self, key: &str, value: Value) { self.data.insert(key.to_string(), value); }
    pub fn get(&self, key: &str) -> Value { self.data.get(key).cloned().unwrap_or(Value::Null) }
    pub fn delete(&mut self, key: &str) -> bool { self.data.remove(key).is_some() }
    pub fn len(&self) -> usize { self.data.len() }
    pub fn keys(&self) -> Vec<String> { let mut k: Vec<_> = self.data.keys().cloned().collect(); k.sort(); k }
}

pub struct GoldenRunner { store: KvStore, output: Vec<String> }

impl GoldenRunner {
    pub fn new() -> Self {
        GoldenRunner { store: KvStore::new(), output: Vec::new() }
    }

    pub fn execute(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.trim().splitn(3, ' ').collect();
        match parts[0] {
            "SET" => {
                if parts.len() >= 3 {
                    self.store.set(parts[1], Value::Str(parts[2].to_string()));
                    self.output.push("OK".into());
                } else {
                    self.output.push("ERROR: SET requires key and value".into());
                }
            }
            "GET" => {
                if parts.len() >= 2 {
                    self.output.push(self.store.get(parts[1]).to_string());
                } else {
                    self.output.push("ERROR: GET requires a key".into());
                }
            }
            "DELETE" => {
                if parts.len() >= 2 {
                    if self.store.delete(parts[1]) {
                        self.output.push("OK".into());
                    } else {
                        self.output.push("NOT_FOUND".into());
                    }
                } else {
                    self.output.push("ERROR: DELETE requires a key".into());
                }
            }
            "LEN" => {
                self.output.push(self.store.len().to_string());
            }
            "KEYS" => {
                let keys = self.store.keys();
                if keys.is_empty() {
                    self.output.push("(empty)".into());
                } else {
                    self.output.push(keys.join(","));
                }
            }
            _ => {
                self.output.push("ERROR: unknown command".into());
            }
        }
    }

    pub fn output(&self) -> String { self.output.join("\n") }
}

pub fn prop_set_get(key: &str, value: Value) -> bool {
    let mut store = KvStore::new();
    store.set(key, value.clone());
    store.get(key) == value
}

pub fn prop_delete_get(key: &str, value: Value) -> bool {
    let mut store = KvStore::new();
    store.set(key, value);
    store.delete(key);
    store.get(key) == Value::Null
}

pub fn prop_len_tracks_keys(ops: &[(String, Value)]) -> bool {
    let mut store = KvStore::new();
    for (k, v) in ops {
        store.set(k, v.clone());
    }
    let unique_keys: std::collections::HashSet<_> = ops.iter().map(|(k, _)| k).collect();
    store.len() == unique_keys.len()
}

fn main() {
    println!("=== Chapter 18: Testing — Solution ===");
    let mut r = GoldenRunner::new();
    r.execute("SET name Alice");
    r.execute("GET name");
    r.execute("DELETE name");
    r.execute("GET name");
    println!("{}", r.output());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_golden_basic() {
        let mut r = GoldenRunner::new();
        r.execute("SET name Alice"); r.execute("GET name"); r.execute("LEN");
        assert_eq!(r.output(), "OK\nAlice\n1");
    }

    #[test] fn test_golden_delete() {
        let mut r = GoldenRunner::new();
        r.execute("SET x hello"); r.execute("DELETE x"); r.execute("GET x"); r.execute("DELETE x");
        assert_eq!(r.output(), "OK\nOK\nNULL\nNOT_FOUND");
    }

    #[test] fn test_prop_set_get() {
        assert!(prop_set_get("a", Value::Int(42)));
        assert!(prop_set_get("hello", Value::Str("world".into())));
    }

    #[test] fn test_prop_len() {
        let ops = vec![("a".into(), Value::Int(1)), ("b".into(), Value::Int(2)), ("a".into(), Value::Int(3))];
        assert!(prop_len_tracks_keys(&ops));
    }
}
