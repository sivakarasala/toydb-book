/// Capstone Challenge 7: Deadlock Detector — SOLUTION

use std::collections::{HashMap, HashSet};

pub struct WaitForGraph {
    edges: HashMap<u64, Vec<u64>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Color { White, Gray, Black }

impl WaitForGraph {
    pub fn new() -> Self { WaitForGraph { edges: HashMap::new() } }

    pub fn add_wait(&mut self, waiter: u64, holder: u64) {
        self.edges.entry(waiter).or_default().push(holder);
    }

    pub fn has_cycle(&self) -> bool {
        let nodes: HashSet<u64> = self.edges.keys().copied()
            .chain(self.edges.values().flatten().copied()).collect();
        let mut color: HashMap<u64, Color> = nodes.iter().map(|&n| (n, Color::White)).collect();

        for &node in &nodes {
            if color[&node] == Color::White {
                if self.dfs_has_cycle(node, &mut color) { return true; }
            }
        }
        false
    }

    fn dfs_has_cycle(&self, node: u64, color: &mut HashMap<u64, Color>) -> bool {
        color.insert(node, Color::Gray);
        if let Some(neighbors) = self.edges.get(&node) {
            for &next in neighbors {
                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::Gray => return true,
                    Color::White => {
                        if self.dfs_has_cycle(next, color) { return true; }
                    }
                    Color::Black => {}
                }
            }
        }
        color.insert(node, Color::Black);
        false
    }

    pub fn find_cycle_members(&self) -> Vec<u64> {
        let nodes: HashSet<u64> = self.edges.keys().copied()
            .chain(self.edges.values().flatten().copied()).collect();
        let mut color: HashMap<u64, Color> = nodes.iter().map(|&n| (n, Color::White)).collect();
        let mut path = Vec::new();
        let mut cycle_members = HashSet::new();

        for &node in &nodes {
            if color[&node] == Color::White {
                self.dfs_find_cycles(node, &mut color, &mut path, &mut cycle_members);
            }
        }
        cycle_members.into_iter().collect()
    }

    fn dfs_find_cycles(&self, node: u64, color: &mut HashMap<u64, Color>,
                        path: &mut Vec<u64>, members: &mut HashSet<u64>) {
        color.insert(node, Color::Gray);
        path.push(node);
        if let Some(neighbors) = self.edges.get(&node) {
            for &next in neighbors {
                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::Gray => {
                        // Found cycle — collect all nodes from `next` to end of path
                        let pos = path.iter().position(|&n| n == next).unwrap();
                        for &n in &path[pos..] { members.insert(n); }
                    }
                    Color::White => self.dfs_find_cycles(next, color, path, members),
                    Color::Black => {}
                }
            }
        }
        path.pop();
        color.insert(node, Color::Black);
    }

    pub fn remove_transaction(&mut self, txn_id: u64) {
        self.edges.remove(&txn_id);
        for edges in self.edges.values_mut() {
            edges.retain(|&e| e != txn_id);
        }
    }
}

fn main() { println!("Capstone 7: Deadlock Detector — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_no_cycle() { let mut g = WaitForGraph::new(); g.add_wait(1,2); g.add_wait(2,3); assert!(!g.has_cycle()); }
    #[test] fn test_cycle() { let mut g = WaitForGraph::new(); g.add_wait(1,2); g.add_wait(2,3); g.add_wait(3,1); assert!(g.has_cycle()); }
    #[test] fn test_members() {
        let mut g = WaitForGraph::new(); g.add_wait(1,2); g.add_wait(2,3); g.add_wait(3,1); g.add_wait(4,1);
        let m = g.find_cycle_members(); assert!(m.contains(&1) && m.contains(&2) && m.contains(&3));
    }
    #[test] fn test_break() { let mut g = WaitForGraph::new(); g.add_wait(1,2); g.add_wait(2,3); g.add_wait(3,1); g.remove_transaction(2); assert!(!g.has_cycle()); }
}
