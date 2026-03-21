/// Capstone Challenge 1: KV Range Query
/// Implement range queries over a sorted key-value store using BTreeMap.

use std::collections::BTreeMap;

pub struct SortedKvStore {
    data: BTreeMap<String, String>,
}

impl SortedKvStore {
    pub fn new() -> Self {
        SortedKvStore { data: BTreeMap::new() }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    /// Return all key-value pairs where start <= key < end
    pub fn range(&self, start: &str, end: &str) -> Vec<(String, String)> {
        // TODO: Use BTreeMap::range to collect entries in [start, end)
        todo!("Implement range query")
    }

    /// Return all key-value pairs where key starts with prefix
    pub fn prefix_scan(&self, prefix: &str) -> Vec<(String, String)> {
        // TODO: Calculate the range [prefix, prefix_next) and use self.range()
        // Hint: increment the last byte of prefix to get the upper bound
        todo!("Implement prefix scan")
    }

    /// Return the minimum key-value pair, or None if empty
    pub fn min(&self) -> Option<(String, String)> {
        // TODO: Return the first entry
        todo!("Implement min")
    }

    /// Return the maximum key-value pair, or None if empty
    pub fn max(&self) -> Option<(String, String)> {
        // TODO: Return the last entry
        todo!("Implement max")
    }
}

fn main() {
    println!("Capstone Challenge 1: KV Range Query");
    println!("Run `cargo test --bin c1-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_store() -> SortedKvStore {
        let mut s = SortedKvStore::new();
        for i in 1..=10 {
            s.set(format!("key:{:02}", i), format!("val{}", i));
        }
        s
    }

    #[test]
    fn test_range() {
        let s = sample_store();
        let r = s.range("key:03", "key:07");
        assert_eq!(r.len(), 4); // key:03, key:04, key:05, key:06
        assert_eq!(r[0].0, "key:03");
        assert_eq!(r[3].0, "key:06");
    }

    #[test]
    fn test_prefix_scan() {
        let mut s = SortedKvStore::new();
        s.set("user:1".into(), "Alice".into());
        s.set("user:2".into(), "Bob".into());
        s.set("item:1".into(), "Widget".into());
        let users = s.prefix_scan("user:");
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn test_min_max() {
        let s = sample_store();
        assert_eq!(s.min().unwrap().0, "key:01");
        assert_eq!(s.max().unwrap().0, "key:10");
    }

    #[test]
    fn test_empty() {
        let s = SortedKvStore::new();
        assert!(s.min().is_none());
        assert!(s.max().is_none());
        assert!(s.range("a", "z").is_empty());
    }
}
