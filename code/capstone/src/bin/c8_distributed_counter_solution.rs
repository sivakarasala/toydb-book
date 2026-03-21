/// Capstone Challenge 8: Distributed Counter (CRDTs) — SOLUTION

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GCounter { node_id: String, counters: HashMap<String, u64> }

impl GCounter {
    pub fn new(node_id: &str) -> Self {
        let mut counters = HashMap::new();
        counters.insert(node_id.to_string(), 0);
        GCounter { node_id: node_id.to_string(), counters }
    }

    pub fn increment(&mut self) {
        *self.counters.entry(self.node_id.clone()).or_default() += 1;
    }

    pub fn value(&self) -> u64 { self.counters.values().sum() }

    pub fn merge(&mut self, other: &GCounter) {
        for (node, &count) in &other.counters {
            let entry = self.counters.entry(node.clone()).or_default();
            *entry = (*entry).max(count);
        }
    }
}

#[derive(Debug, Clone)]
pub struct PNCounter { positive: GCounter, negative: GCounter }

impl PNCounter {
    pub fn new(node_id: &str) -> Self { PNCounter { positive: GCounter::new(node_id), negative: GCounter::new(node_id) } }
    pub fn increment(&mut self) { self.positive.increment(); }
    pub fn decrement(&mut self) { self.negative.increment(); }
    pub fn value(&self) -> i64 { self.positive.value() as i64 - self.negative.value() as i64 }
    pub fn merge(&mut self, other: &PNCounter) { self.positive.merge(&other.positive); self.negative.merge(&other.negative); }
}

fn main() { println!("Capstone 8: Distributed Counter — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_g_basic() { let mut c = GCounter::new("n1"); c.increment(); c.increment(); assert_eq!(c.value(), 2); }
    #[test] fn test_g_merge() {
        let mut c1 = GCounter::new("n1"); let mut c2 = GCounter::new("n2");
        c1.increment(); c1.increment(); c2.increment(); c2.increment(); c2.increment();
        c1.merge(&c2); assert_eq!(c1.value(), 5);
    }
    #[test] fn test_idempotent() {
        let mut c1 = GCounter::new("n1"); let mut c2 = GCounter::new("n2");
        c1.increment(); c2.increment(); c1.merge(&c2); c1.merge(&c2);
        assert_eq!(c1.value(), 2);
    }
    #[test] fn test_pn() { let mut c = PNCounter::new("n1"); c.increment(); c.increment(); c.decrement(); assert_eq!(c.value(), 1); }
    #[test] fn test_pn_merge() {
        let mut c1 = PNCounter::new("n1"); let mut c2 = PNCounter::new("n2");
        c1.increment(); c1.increment(); c2.increment(); c2.decrement();
        c1.merge(&c2); assert_eq!(c1.value(), 2);
    }
}
