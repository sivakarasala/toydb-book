/// Capstone Challenge 4: Transaction Scheduler (Topological Sort)
/// Given transactions with dependencies, find a valid execution order.

use std::collections::{HashMap, HashSet, VecDeque};

/// A transaction with an id and a list of dependency ids.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub depends_on: Vec<u64>,
}

/// Find a valid execution order using topological sort (Kahn's algorithm).
/// Returns None if there is a cycle (deadlock).
pub fn schedule(transactions: &[Transaction]) -> Option<Vec<u64>> {
    // TODO:
    // 1. Build adjacency list and in-degree map
    // 2. Start with nodes that have in-degree 0
    // 3. BFS: process each node, decrement in-degrees of neighbors
    // 4. If all nodes processed, return order; otherwise cycle detected
    todo!("Implement topological sort scheduler")
}

/// Check if a set of transactions has a cycle (deadlock).
pub fn has_deadlock(transactions: &[Transaction]) -> bool {
    // TODO: Return true if schedule() returns None
    todo!("Implement deadlock detection")
}

fn main() {
    println!("Capstone Challenge 4: Transaction Scheduler");
    println!("Run `cargo test --bin c4-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_order() {
        let txns = vec![
            Transaction { id: 1, depends_on: vec![] },
            Transaction { id: 2, depends_on: vec![1] },
            Transaction { id: 3, depends_on: vec![1] },
            Transaction { id: 4, depends_on: vec![2, 3] },
        ];
        let order = schedule(&txns).unwrap();
        assert_eq!(order[0], 1); // must come first
        assert_eq!(*order.last().unwrap(), 4); // must come last
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn test_cycle_detection() {
        let txns = vec![
            Transaction { id: 1, depends_on: vec![3] },
            Transaction { id: 2, depends_on: vec![1] },
            Transaction { id: 3, depends_on: vec![2] },
        ];
        assert!(has_deadlock(&txns));
        assert!(schedule(&txns).is_none());
    }

    #[test]
    fn test_independent() {
        let txns = vec![
            Transaction { id: 1, depends_on: vec![] },
            Transaction { id: 2, depends_on: vec![] },
            Transaction { id: 3, depends_on: vec![] },
        ];
        let order = schedule(&txns).unwrap();
        assert_eq!(order.len(), 3);
    }
}
