/// Chapter 18: Testing, Benchmarking & Extensions
/// Exercise: Implement a test harness with golden testing and property-based checks.

use std::collections::HashMap;
use std::fmt;

// ── A simple KV store to test ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Str(String),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Null => write!(f, "NULL"),
        }
    }
}

pub struct KvStore {
    data: HashMap<String, Value>,
}

impl KvStore {
    pub fn new() -> Self {
        KvStore { data: HashMap::new() }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Value {
        self.data.get(key).cloned().unwrap_or(Value::Null)
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.data.keys().cloned().collect();
        keys.sort();
        keys
    }
}

// ── Golden test runner ─────────────────────────────────────────────

/// A golden test runner that executes commands and compares output.
pub struct GoldenRunner {
    store: KvStore,
    output: Vec<String>,
}

impl GoldenRunner {
    pub fn new() -> Self {
        // TODO: Create a runner with a fresh KvStore and empty output
        todo!("Implement GoldenRunner::new")
    }

    /// Execute a command string like "SET key value" or "GET key"
    pub fn execute(&mut self, cmd: &str) {
        // TODO: Parse and execute commands, pushing results to self.output
        // Commands:
        //   "SET key value" → set key to Value::Str(value), push "OK"
        //   "GET key" → push the Display string of the value
        //   "DELETE key" → push "OK" if existed, "NOT_FOUND" if not
        //   "LEN" → push the count as a string
        //   "KEYS" → push comma-separated sorted keys (or "(empty)" if none)
        //   anything else → push "ERROR: unknown command"
        todo!("Implement GoldenRunner::execute")
    }

    pub fn output(&self) -> String {
        self.output.join("\n")
    }
}

// ── Property helpers ───────────────────────────────────────────────

/// Check that the KV store satisfies the "set-get" property:
/// After setting a key, getting it returns the same value.
pub fn prop_set_get(key: &str, value: Value) -> bool {
    // TODO: Create a KvStore, set the key, get it back, check equality
    todo!("Implement prop_set_get")
}

/// Check that delete-then-get returns Null.
pub fn prop_delete_get(key: &str, value: Value) -> bool {
    // TODO: Create a KvStore, set the key, delete it, verify get returns Null
    todo!("Implement prop_delete_get")
}

/// Check that len tracks the number of unique keys.
pub fn prop_len_tracks_keys(ops: &[(String, Value)]) -> bool {
    // TODO: Apply all ops to a KvStore
    // Expected len = number of unique keys in ops
    todo!("Implement prop_len_tracks_keys")
}

fn main() {
    println!("=== Chapter 18: Testing & Benchmarking ===");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Golden Tests ────────────────────────────────────────────

    #[test]
    fn test_golden_basic() {
        let mut r = GoldenRunner::new();
        r.execute("SET name Alice");
        r.execute("GET name");
        r.execute("LEN");
        let expected = "OK\nAlice\n1";
        assert_eq!(r.output(), expected);
    }

    #[test]
    fn test_golden_delete() {
        let mut r = GoldenRunner::new();
        r.execute("SET x hello");
        r.execute("DELETE x");
        r.execute("GET x");
        r.execute("DELETE x");
        let expected = "OK\nOK\nNULL\nNOT_FOUND";
        assert_eq!(r.output(), expected);
    }

    #[test]
    fn test_golden_keys() {
        let mut r = GoldenRunner::new();
        r.execute("KEYS");
        r.execute("SET b 2");
        r.execute("SET a 1");
        r.execute("KEYS");
        let expected = "(empty)\nOK\nOK\na,b";
        assert_eq!(r.output(), expected);
    }

    #[test]
    fn test_golden_unknown() {
        let mut r = GoldenRunner::new();
        r.execute("FROBNICATE");
        assert_eq!(r.output(), "ERROR: unknown command");
    }

    // ── Property Tests ──────────────────────────────────────────

    #[test]
    fn test_prop_set_get() {
        assert!(prop_set_get("a", Value::Int(42)));
        assert!(prop_set_get("hello", Value::Str("world".into())));
        assert!(prop_set_get("empty", Value::Str(String::new())));
    }

    #[test]
    fn test_prop_delete_get() {
        assert!(prop_delete_get("x", Value::Int(1)));
        assert!(prop_delete_get("name", Value::Str("test".into())));
    }

    #[test]
    fn test_prop_len_tracks_keys() {
        let ops = vec![
            ("a".into(), Value::Int(1)),
            ("b".into(), Value::Int(2)),
            ("a".into(), Value::Int(3)), // overwrite
        ];
        assert!(prop_len_tracks_keys(&ops));

        let empty: Vec<(String, Value)> = vec![];
        assert!(prop_len_tracks_keys(&empty));
    }
}
