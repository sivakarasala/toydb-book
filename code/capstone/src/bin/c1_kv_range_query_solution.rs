/// Capstone Challenge 1: KV Range Query — SOLUTION

use std::collections::BTreeMap;

pub struct SortedKvStore { data: BTreeMap<String, String> }

impl SortedKvStore {
    pub fn new() -> Self { SortedKvStore { data: BTreeMap::new() } }
    pub fn set(&mut self, key: String, value: String) { self.data.insert(key, value); }

    pub fn range(&self, start: &str, end: &str) -> Vec<(String, String)> {
        self.data.range(start.to_string()..end.to_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn prefix_scan(&self, prefix: &str) -> Vec<(String, String)> {
        let mut end = prefix.as_bytes().to_vec();
        if let Some(last) = end.last_mut() {
            *last += 1;
        }
        let end_str = String::from_utf8(end).unwrap_or_default();
        self.range(prefix, &end_str)
    }

    pub fn min(&self) -> Option<(String, String)> {
        self.data.iter().next().map(|(k, v)| (k.clone(), v.clone()))
    }

    pub fn max(&self) -> Option<(String, String)> {
        self.data.iter().next_back().map(|(k, v)| (k.clone(), v.clone()))
    }
}

fn main() {
    println!("Capstone Challenge 1: KV Range Query — Solution");
    let mut s = SortedKvStore::new();
    for i in 1..=5 { s.set(format!("k:{:02}", i), format!("v{}", i)); }
    println!("Range [k:02, k:04): {:?}", s.range("k:02", "k:04"));
    println!("Min: {:?}, Max: {:?}", s.min(), s.max());
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> SortedKvStore { let mut s = SortedKvStore::new(); for i in 1..=10 { s.set(format!("key:{:02}", i), format!("val{}", i)); } s }

    #[test] fn test_range() { let r = sample().range("key:03", "key:07"); assert_eq!(r.len(), 4); }
    #[test] fn test_prefix() {
        let mut s = SortedKvStore::new();
        s.set("user:1".into(), "A".into()); s.set("user:2".into(), "B".into()); s.set("item:1".into(), "W".into());
        assert_eq!(s.prefix_scan("user:").len(), 2);
    }
    #[test] fn test_min_max() { let s = sample(); assert_eq!(s.min().unwrap().0, "key:01"); assert_eq!(s.max().unwrap().0, "key:10"); }
}
