/// Capstone Challenge 8: Distributed Counter (CRDTs)
/// Implement a G-Counter (grow-only counter) CRDT.

use std::collections::HashMap;

/// A G-Counter where each node has its own counter.
/// The global count is the sum of all node counters.
/// Merging takes the max of each node's counter.
#[derive(Debug, Clone)]
pub struct GCounter {
    node_id: String,
    counters: HashMap<String, u64>,
}

impl GCounter {
    pub fn new(node_id: &str) -> Self {
        // TODO: Create a new GCounter for this node
        todo!("Implement GCounter::new")
    }

    /// Increment this node's counter by 1
    pub fn increment(&mut self) {
        // TODO: Increment the counter for self.node_id
        todo!("Implement increment")
    }

    /// Get the total count across all nodes
    pub fn value(&self) -> u64 {
        // TODO: Sum all counters
        todo!("Implement value")
    }

    /// Merge another GCounter into this one (take max per node)
    pub fn merge(&mut self, other: &GCounter) {
        // TODO: For each node in other.counters, take max(self[node], other[node])
        todo!("Implement merge")
    }

    /// Get the counter map (for inspection)
    pub fn counters(&self) -> &HashMap<String, u64> {
        &self.counters
    }
}

/// A PN-Counter (positive-negative counter) built from two G-Counters.
#[derive(Debug, Clone)]
pub struct PNCounter {
    positive: GCounter,
    negative: GCounter,
}

impl PNCounter {
    pub fn new(node_id: &str) -> Self {
        // TODO: Create with two fresh GCounters
        todo!("Implement PNCounter::new")
    }

    pub fn increment(&mut self) {
        // TODO: Increment positive counter
        todo!("Implement PNCounter increment")
    }

    pub fn decrement(&mut self) {
        // TODO: Increment negative counter
        todo!("Implement PNCounter decrement")
    }

    pub fn value(&self) -> i64 {
        // TODO: positive.value() - negative.value()
        todo!("Implement PNCounter value")
    }

    pub fn merge(&mut self, other: &PNCounter) {
        // TODO: Merge both positive and negative counters
        todo!("Implement PNCounter merge")
    }
}

fn main() {
    println!("Capstone Challenge 8: Distributed Counter (CRDTs)");
    println!("Run `cargo test --bin c8-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcounter_basic() {
        let mut c = GCounter::new("node1");
        c.increment();
        c.increment();
        c.increment();
        assert_eq!(c.value(), 3);
    }

    #[test]
    fn test_gcounter_merge() {
        let mut c1 = GCounter::new("node1");
        let mut c2 = GCounter::new("node2");
        c1.increment(); c1.increment();
        c2.increment(); c2.increment(); c2.increment();
        c1.merge(&c2);
        assert_eq!(c1.value(), 5); // 2 + 3
    }

    #[test]
    fn test_gcounter_merge_idempotent() {
        let mut c1 = GCounter::new("node1");
        let mut c2 = GCounter::new("node2");
        c1.increment();
        c2.increment();
        c1.merge(&c2);
        c1.merge(&c2); // merge again — should be idempotent
        assert_eq!(c1.value(), 2);
    }

    #[test]
    fn test_pncounter() {
        let mut c = PNCounter::new("node1");
        c.increment();
        c.increment();
        c.decrement();
        assert_eq!(c.value(), 1);
    }

    #[test]
    fn test_pncounter_merge() {
        let mut c1 = PNCounter::new("node1");
        let mut c2 = PNCounter::new("node2");
        c1.increment(); c1.increment();
        c2.increment(); c2.decrement();
        c1.merge(&c2);
        assert_eq!(c1.value(), 2); // (2+1) - (0+1) = 2
    }
}
