/// Capstone Challenge 6: Index Scan Optimizer
/// Decide whether to use a full scan or an index scan based on selectivity.

use std::collections::HashMap;

pub struct Table {
    pub name: String,
    pub rows: Vec<Vec<String>>,    // row data
    pub columns: Vec<String>,
    pub row_count: usize,
}

pub struct Index {
    pub column: String,
    pub distinct_values: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScanStrategy {
    FullScan,
    IndexScan { column: String },
}

/// Estimate selectivity of an equality predicate: 1 / distinct_values
pub fn selectivity(index: &Index) -> f64 {
    // TODO: Return 1.0 / distinct_values (or 1.0 if distinct_values is 0)
    todo!("Implement selectivity")
}

/// Choose scan strategy: use index if selectivity < threshold
pub fn choose_strategy(table: &Table, indexes: &[Index], predicate_column: &str, threshold: f64) -> ScanStrategy {
    // TODO:
    // 1. Find an index on predicate_column
    // 2. If found and selectivity(index) < threshold, use IndexScan
    // 3. Otherwise use FullScan
    todo!("Implement choose_strategy")
}

/// Estimate the cost (number of rows accessed) for each strategy
pub fn estimate_cost(table: &Table, strategy: &ScanStrategy, indexes: &[Index]) -> usize {
    // TODO:
    // FullScan: cost = row_count
    // IndexScan: cost = row_count * selectivity of that index (rounded up)
    todo!("Implement estimate_cost")
}

fn main() {
    println!("Capstone Challenge 6: Index Scan Optimizer");
    println!("Run `cargo test --bin c6-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_table() -> Table {
        Table { name: "users".into(), rows: vec![], columns: vec!["id".into(), "name".into(), "country".into()], row_count: 10000 }
    }

    #[test]
    fn test_selectivity() {
        assert!((selectivity(&Index { column: "id".into(), distinct_values: 10000 }) - 0.0001).abs() < 0.0001);
        assert!((selectivity(&Index { column: "country".into(), distinct_values: 50 }) - 0.02).abs() < 0.001);
    }

    #[test]
    fn test_choose_index_scan() {
        let indexes = vec![Index { column: "id".into(), distinct_values: 10000 }];
        let strategy = choose_strategy(&sample_table(), &indexes, "id", 0.1);
        assert_eq!(strategy, ScanStrategy::IndexScan { column: "id".into() });
    }

    #[test]
    fn test_choose_full_scan() {
        let indexes = vec![Index { column: "country".into(), distinct_values: 5 }];
        let strategy = choose_strategy(&sample_table(), &indexes, "country", 0.1);
        assert_eq!(strategy, ScanStrategy::FullScan); // selectivity 0.2 > 0.1
    }

    #[test]
    fn test_no_index() {
        let strategy = choose_strategy(&sample_table(), &[], "name", 0.1);
        assert_eq!(strategy, ScanStrategy::FullScan);
    }

    #[test]
    fn test_cost_estimate() {
        let indexes = vec![Index { column: "id".into(), distinct_values: 10000 }];
        let full = estimate_cost(&sample_table(), &ScanStrategy::FullScan, &indexes);
        let idx = estimate_cost(&sample_table(), &ScanStrategy::IndexScan { column: "id".into() }, &indexes);
        assert_eq!(full, 10000);
        assert_eq!(idx, 1); // 10000 * 0.0001 = 1
    }
}
