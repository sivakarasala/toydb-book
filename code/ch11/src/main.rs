/// Chapter 11: SQL Features — Joins, Aggregations, GROUP BY
/// Exercise: Build join and aggregation executors.

use std::collections::HashMap;

pub type Value = String;
pub type Row = Vec<Value>;

/// Hash join: build a hash table from the left side, probe with the right.
pub fn hash_join(left: Vec<Row>, right: Vec<Row>, left_col: usize, right_col: usize) -> Vec<Row> {
    // TODO:
    // 1. Build a HashMap from left rows: key = left[left_col], value = Vec<Row>
    // 2. For each right row, look up right[right_col] in the map
    // 3. For each match, concatenate left_row + right_row
    todo!("Implement hash_join")
}

/// Nested loop join: check every pair.
pub fn nested_loop_join(left: Vec<Row>, right: Vec<Row>, left_col: usize, right_col: usize) -> Vec<Row> {
    // TODO: For each left row, for each right row, if left[left_col] == right[right_col], emit combined row
    todo!("Implement nested_loop_join")
}

/// Aggregate: GROUP BY + COUNT/SUM
pub fn group_by_count(rows: Vec<Row>, group_col: usize) -> Vec<(String, usize)> {
    // TODO: Group rows by rows[group_col], count each group
    // Return sorted by key
    todo!("Implement group_by_count")
}

/// Sort rows by a column.
pub fn sort_rows(mut rows: Vec<Row>, col: usize, ascending: bool) -> Vec<Row> {
    // TODO: Sort rows by rows[col], respecting ascending/descending
    todo!("Implement sort_rows")
}

fn main() {
    println!("=== Chapter 11: SQL Features ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users() -> Vec<Row> {
        vec![
            vec!["1".into(), "Alice".into()],
            vec!["2".into(), "Bob".into()],
        ]
    }

    fn orders() -> Vec<Row> {
        vec![
            vec!["1".into(), "Book".into()],
            vec!["1".into(), "Pen".into()],
            vec!["2".into(), "Notebook".into()],
        ]
    }

    #[test]
    fn test_hash_join() {
        let result = hash_join(users(), orders(), 0, 0);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_nested_loop_join() {
        let result = nested_loop_join(users(), orders(), 0, 0);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_joins_match() {
        let mut hj = hash_join(users(), orders(), 0, 0);
        let mut nlj = nested_loop_join(users(), orders(), 0, 0);
        hj.sort();
        nlj.sort();
        assert_eq!(hj, nlj);
    }

    #[test]
    fn test_group_by_count() {
        let rows = vec![
            vec!["A".into()], vec!["B".into()], vec!["A".into()], vec!["A".into()], vec!["B".into()],
        ];
        let mut result = group_by_count(rows, 0);
        result.sort();
        assert_eq!(result, vec![("A".into(), 3), ("B".into(), 2)]);
    }

    #[test]
    fn test_sort() {
        let rows = vec![
            vec!["banana".into()], vec!["apple".into()], vec!["cherry".into()],
        ];
        let sorted = sort_rows(rows, 0, true);
        assert_eq!(sorted[0][0], "apple");
        assert_eq!(sorted[2][0], "cherry");
    }
}
