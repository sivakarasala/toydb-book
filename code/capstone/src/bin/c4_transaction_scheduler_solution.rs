/// Capstone Challenge 4: Transaction Scheduler — SOLUTION

use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct Transaction { pub id: u64, pub depends_on: Vec<u64> }

pub fn schedule(transactions: &[Transaction]) -> Option<Vec<u64>> {
    let ids: HashSet<u64> = transactions.iter().map(|t| t.id).collect();
    let mut in_degree: HashMap<u64, usize> = ids.iter().map(|&id| (id, 0)).collect();
    let mut adj: HashMap<u64, Vec<u64>> = ids.iter().map(|&id| (id, Vec::new())).collect();

    for txn in transactions {
        for &dep in &txn.depends_on {
            adj.entry(dep).or_default().push(txn.id);
            *in_degree.entry(txn.id).or_default() += 1;
        }
    }

    let mut queue: VecDeque<u64> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0).map(|(&id, _)| id).collect();
    let mut order = Vec::new();

    while let Some(id) = queue.pop_front() {
        order.push(id);
        if let Some(neighbors) = adj.get(&id) {
            for &next in neighbors {
                let deg = in_degree.get_mut(&next).unwrap();
                *deg -= 1;
                if *deg == 0 { queue.push_back(next); }
            }
        }
    }

    if order.len() == ids.len() { Some(order) } else { None }
}

pub fn has_deadlock(transactions: &[Transaction]) -> bool { schedule(transactions).is_none() }

fn main() { println!("Capstone 4: Transaction Scheduler — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_order() {
        let t = vec![Transaction{id:1,depends_on:vec![]},Transaction{id:2,depends_on:vec![1]},Transaction{id:3,depends_on:vec![1]},Transaction{id:4,depends_on:vec![2,3]}];
        let o = schedule(&t).unwrap(); assert_eq!(o[0], 1); assert_eq!(*o.last().unwrap(), 4);
    }
    #[test] fn test_cycle() {
        let t = vec![Transaction{id:1,depends_on:vec![3]},Transaction{id:2,depends_on:vec![1]},Transaction{id:3,depends_on:vec![2]}];
        assert!(has_deadlock(&t));
    }
}
