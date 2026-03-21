/// Capstone Challenge 6: Index Scan Optimizer — SOLUTION

pub struct Table { pub name: String, pub rows: Vec<Vec<String>>, pub columns: Vec<String>, pub row_count: usize }
pub struct Index { pub column: String, pub distinct_values: usize }

#[derive(Debug, Clone, PartialEq)]
pub enum ScanStrategy { FullScan, IndexScan { column: String } }

pub fn selectivity(index: &Index) -> f64 {
    if index.distinct_values == 0 { 1.0 } else { 1.0 / index.distinct_values as f64 }
}

pub fn choose_strategy(_table: &Table, indexes: &[Index], predicate_column: &str, threshold: f64) -> ScanStrategy {
    for idx in indexes {
        if idx.column == predicate_column && selectivity(idx) < threshold {
            return ScanStrategy::IndexScan { column: predicate_column.to_string() };
        }
    }
    ScanStrategy::FullScan
}

pub fn estimate_cost(table: &Table, strategy: &ScanStrategy, indexes: &[Index]) -> usize {
    match strategy {
        ScanStrategy::FullScan => table.row_count,
        ScanStrategy::IndexScan { column } => {
            if let Some(idx) = indexes.iter().find(|i| i.column == *column) {
                (table.row_count as f64 * selectivity(idx)).ceil() as usize
            } else {
                table.row_count
            }
        }
    }
}

fn main() { println!("Capstone 6: Index Scan Optimizer — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    fn t() -> Table { Table { name: "users".into(), rows: vec![], columns: vec![], row_count: 10000 } }
    #[test] fn test_sel() { assert!((selectivity(&Index { column: "id".into(), distinct_values: 10000 }) - 0.0001).abs() < 0.0001); }
    #[test] fn test_idx() { assert_eq!(choose_strategy(&t(), &[Index{column:"id".into(),distinct_values:10000}], "id", 0.1), ScanStrategy::IndexScan{column:"id".into()}); }
    #[test] fn test_full() { assert_eq!(choose_strategy(&t(), &[Index{column:"c".into(),distinct_values:5}], "c", 0.1), ScanStrategy::FullScan); }
    #[test] fn test_cost() { assert_eq!(estimate_cost(&t(), &ScanStrategy::IndexScan{column:"id".into()}, &[Index{column:"id".into(),distinct_values:10000}]), 1); }
}
