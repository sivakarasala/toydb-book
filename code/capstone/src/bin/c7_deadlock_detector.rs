/// Capstone Challenge 7: Deadlock Detector (Cycle Detection)
/// Detect cycles in a wait-for graph of transactions.

use std::collections::{HashMap, HashSet};

pub struct WaitForGraph {
    /// edges[a] = b means transaction `a` is waiting for `b`
    edges: HashMap<u64, Vec<u64>>,
}

impl WaitForGraph {
    pub fn new() -> Self {
        WaitForGraph { edges: HashMap::new() }
    }

    pub fn add_wait(&mut self, waiter: u64, holder: u64) {
        self.edges.entry(waiter).or_default().push(holder);
    }

    /// Detect if there is any cycle in the wait-for graph.
    pub fn has_cycle(&self) -> bool {
        // TODO: Use DFS with coloring (white/gray/black) to detect back edges
        // White = unvisited, Gray = in current path, Black = fully processed
        // A cycle exists if we visit a Gray node
        todo!("Implement cycle detection")
    }

    /// Find all transactions involved in cycles (for victim selection).
    pub fn find_cycle_members(&self) -> Vec<u64> {
        // TODO: Find and return all node IDs that are part of a cycle
        // Hint: Run DFS, when you find a back edge, trace back to collect the cycle
        todo!("Implement find_cycle_members")
    }

    /// Remove a transaction (and its edges) to break deadlocks.
    pub fn remove_transaction(&mut self, txn_id: u64) {
        // TODO: Remove all edges from and to txn_id
        todo!("Implement remove_transaction")
    }
}

fn main() {
    println!("Capstone Challenge 7: Deadlock Detector");
    println!("Run `cargo test --bin c7-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle() {
        let mut g = WaitForGraph::new();
        g.add_wait(1, 2);
        g.add_wait(2, 3);
        assert!(!g.has_cycle());
    }

    #[test]
    fn test_simple_cycle() {
        let mut g = WaitForGraph::new();
        g.add_wait(1, 2);
        g.add_wait(2, 3);
        g.add_wait(3, 1);
        assert!(g.has_cycle());
    }

    #[test]
    fn test_cycle_members() {
        let mut g = WaitForGraph::new();
        g.add_wait(1, 2);
        g.add_wait(2, 3);
        g.add_wait(3, 1);
        g.add_wait(4, 1); // 4 waits for 1 but is NOT in the cycle
        let members = g.find_cycle_members();
        assert!(members.contains(&1));
        assert!(members.contains(&2));
        assert!(members.contains(&3));
    }

    #[test]
    fn test_break_deadlock() {
        let mut g = WaitForGraph::new();
        g.add_wait(1, 2);
        g.add_wait(2, 3);
        g.add_wait(3, 1);
        assert!(g.has_cycle());
        g.remove_transaction(2);
        assert!(!g.has_cycle());
    }
}
