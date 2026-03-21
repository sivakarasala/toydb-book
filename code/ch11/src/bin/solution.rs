/// Chapter 11: SQL Features — SOLUTION

use std::collections::HashMap;

pub type Value = String;
pub type Row = Vec<Value>;

pub fn hash_join(left: Vec<Row>, right: Vec<Row>, left_col: usize, right_col: usize) -> Vec<Row> {
    let mut map: HashMap<String, Vec<Row>> = HashMap::new();
    for row in &left {
        map.entry(row[left_col].clone()).or_default().push(row.clone());
    }
    let mut result = Vec::new();
    for rrow in &right {
        if let Some(lrows) = map.get(&rrow[right_col]) {
            for lrow in lrows {
                let mut combined = lrow.clone();
                combined.extend(rrow.iter().cloned());
                result.push(combined);
            }
        }
    }
    result
}

pub fn nested_loop_join(left: Vec<Row>, right: Vec<Row>, left_col: usize, right_col: usize) -> Vec<Row> {
    let mut result = Vec::new();
    for lrow in &left {
        for rrow in &right {
            if lrow[left_col] == rrow[right_col] {
                let mut combined = lrow.clone();
                combined.extend(rrow.iter().cloned());
                result.push(combined);
            }
        }
    }
    result
}

pub fn group_by_count(rows: Vec<Row>, group_col: usize) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for row in &rows {
        *counts.entry(row[group_col].clone()).or_default() += 1;
    }
    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort();
    result
}

pub fn sort_rows(mut rows: Vec<Row>, col: usize, ascending: bool) -> Vec<Row> {
    rows.sort_by(|a, b| {
        let cmp = a[col].cmp(&b[col]);
        if ascending { cmp } else { cmp.reverse() }
    });
    rows
}

fn main() {
    println!("=== Chapter 11: SQL Features — Solution ===");
    let users = vec![vec!["1".into(), "Alice".into()], vec!["2".into(), "Bob".into()]];
    let orders = vec![vec!["1".into(), "Book".into()], vec!["1".into(), "Pen".into()], vec!["2".into(), "Notebook".into()]];
    println!("Hash Join:");
    for row in hash_join(users, orders, 0, 0) { println!("  {:?}", row); }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn users() -> Vec<Row> { vec![vec!["1".into(), "Alice".into()], vec!["2".into(), "Bob".into()]] }
    fn orders() -> Vec<Row> { vec![vec!["1".into(), "Book".into()], vec!["1".into(), "Pen".into()], vec!["2".into(), "Notebook".into()]] }

    #[test] fn test_hash_join() { assert_eq!(hash_join(users(), orders(), 0, 0).len(), 3); }
    #[test] fn test_nested_loop_join() { assert_eq!(nested_loop_join(users(), orders(), 0, 0).len(), 3); }
    #[test] fn test_joins_match() {
        let mut hj = hash_join(users(), orders(), 0, 0); hj.sort();
        let mut nlj = nested_loop_join(users(), orders(), 0, 0); nlj.sort();
        assert_eq!(hj, nlj);
    }
    #[test] fn test_group_by_count() {
        let rows = vec![vec!["A".into()], vec!["B".into()], vec!["A".into()], vec!["A".into()], vec!["B".into()]];
        let mut r = group_by_count(rows, 0); r.sort();
        assert_eq!(r, vec![("A".into(), 3), ("B".into(), 2)]);
    }
    #[test] fn test_sort() {
        let rows = vec![vec!["banana".into()], vec!["apple".into()], vec!["cherry".into()]];
        let s = sort_rows(rows, 0, true);
        assert_eq!(s[0][0], "apple"); assert_eq!(s[2][0], "cherry");
    }
}
