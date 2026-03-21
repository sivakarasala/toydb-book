/// Chapter 10: Query Executor — The Volcano Model
/// Exercise: Build pull-based executors using the Iterator pattern.

pub type Value = String;
pub type Row = Vec<Value>;

/// A pull-based executor.
pub trait Executor {
    fn next(&mut self) -> Option<Row>;

    fn collect_all(&mut self) -> Vec<Row> {
        let mut rows = Vec::new();
        while let Some(row) = self.next() { rows.push(row); }
        rows
    }
}

/// Yields rows from a data source.
pub struct ScanExecutor {
    rows: Vec<Row>,
    pos: usize,
}

impl ScanExecutor {
    pub fn new(rows: Vec<Row>) -> Self { ScanExecutor { rows, pos: 0 } }
}

impl Executor for ScanExecutor {
    fn next(&mut self) -> Option<Row> {
        // TODO: Return the next row, advance pos
        todo!("Implement ScanExecutor::next")
    }
}

/// Filters rows by a predicate (column index == value).
pub struct FilterExecutor {
    source: Box<dyn Executor>,
    column_idx: usize,
    value: Value,
}

impl FilterExecutor {
    pub fn new(source: Box<dyn Executor>, column_idx: usize, value: Value) -> Self {
        FilterExecutor { source, column_idx, value }
    }
}

impl Executor for FilterExecutor {
    fn next(&mut self) -> Option<Row> {
        // TODO: Pull from source, skip rows where row[column_idx] != value
        todo!("Implement FilterExecutor::next")
    }
}

/// Projects specific columns from each row.
pub struct ProjectExecutor {
    source: Box<dyn Executor>,
    column_indices: Vec<usize>,
}

impl ProjectExecutor {
    pub fn new(source: Box<dyn Executor>, column_indices: Vec<usize>) -> Self {
        ProjectExecutor { source, column_indices }
    }
}

impl Executor for ProjectExecutor {
    fn next(&mut self) -> Option<Row> {
        // TODO: Pull from source, keep only the specified column indices
        todo!("Implement ProjectExecutor::next")
    }
}

fn main() {
    println!("=== Chapter 10: Query Executor ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> Vec<Row> {
        vec![
            vec!["1".into(), "Alice".into(), "25".into()],
            vec!["2".into(), "Bob".into(), "30".into()],
            vec!["3".into(), "Carol".into(), "25".into()],
        ]
    }

    #[test]
    fn test_scan() {
        let mut scan = ScanExecutor::new(sample_data());
        assert_eq!(scan.collect_all().len(), 3);
    }

    #[test]
    fn test_filter() {
        let scan = Box::new(ScanExecutor::new(sample_data()));
        let mut filter = FilterExecutor::new(scan, 2, "25".into());
        let results = filter.collect_all();
        assert_eq!(results.len(), 2); // Alice and Carol
    }

    #[test]
    fn test_project() {
        let scan = Box::new(ScanExecutor::new(sample_data()));
        let mut project = ProjectExecutor::new(scan, vec![1]); // name only
        let results = project.collect_all();
        assert_eq!(results[0], vec!["Alice".to_string()]);
        assert_eq!(results[1], vec!["Bob".to_string()]);
    }

    #[test]
    fn test_pipeline() {
        let scan = Box::new(ScanExecutor::new(sample_data()));
        let filter = Box::new(FilterExecutor::new(scan, 2, "25".into()));
        let mut project = ProjectExecutor::new(filter, vec![1]);
        let results = project.collect_all();
        assert_eq!(results, vec![vec!["Alice".to_string()], vec!["Carol".to_string()]]);
    }
}
