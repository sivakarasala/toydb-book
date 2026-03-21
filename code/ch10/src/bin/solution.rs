/// Chapter 10: Query Executor — SOLUTION

pub type Value = String;
pub type Row = Vec<Value>;

pub trait Executor {
    fn next(&mut self) -> Option<Row>;
    fn collect_all(&mut self) -> Vec<Row> {
        let mut rows = Vec::new();
        while let Some(row) = self.next() { rows.push(row); }
        rows
    }
}

pub struct ScanExecutor { rows: Vec<Row>, pos: usize }
impl ScanExecutor { pub fn new(rows: Vec<Row>) -> Self { ScanExecutor { rows, pos: 0 } } }
impl Executor for ScanExecutor {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.rows.len() { let r = self.rows[self.pos].clone(); self.pos += 1; Some(r) } else { None }
    }
}

pub struct FilterExecutor { source: Box<dyn Executor>, column_idx: usize, value: Value }
impl FilterExecutor {
    pub fn new(source: Box<dyn Executor>, column_idx: usize, value: Value) -> Self {
        FilterExecutor { source, column_idx, value }
    }
}
impl Executor for FilterExecutor {
    fn next(&mut self) -> Option<Row> {
        loop {
            let row = self.source.next()?;
            if row[self.column_idx] == self.value { return Some(row); }
        }
    }
}

pub struct ProjectExecutor { source: Box<dyn Executor>, column_indices: Vec<usize> }
impl ProjectExecutor {
    pub fn new(source: Box<dyn Executor>, column_indices: Vec<usize>) -> Self {
        ProjectExecutor { source, column_indices }
    }
}
impl Executor for ProjectExecutor {
    fn next(&mut self) -> Option<Row> {
        let row = self.source.next()?;
        Some(self.column_indices.iter().map(|&i| row[i].clone()).collect())
    }
}

fn main() {
    println!("=== Chapter 10: Executor — Solution ===");
    let data = vec![
        vec!["1".into(), "Alice".into(), "25".into()],
        vec!["2".into(), "Bob".into(), "30".into()],
        vec!["3".into(), "Carol".into(), "25".into()],
    ];
    let scan = Box::new(ScanExecutor::new(data));
    let filter = Box::new(FilterExecutor::new(scan, 2, "25".into()));
    let mut project = ProjectExecutor::new(filter, vec![1]);
    println!("SELECT name FROM data WHERE age = 25:");
    for row in project.collect_all() { println!("  {:?}", row); }
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
    fn test_scan() { assert_eq!(ScanExecutor::new(sample_data()).collect_all().len(), 3); }

    #[test]
    fn test_filter() {
        let mut f = FilterExecutor::new(Box::new(ScanExecutor::new(sample_data())), 2, "25".into());
        assert_eq!(f.collect_all().len(), 2);
    }

    #[test]
    fn test_project() {
        let mut p = ProjectExecutor::new(Box::new(ScanExecutor::new(sample_data())), vec![1]);
        assert_eq!(p.collect_all()[0], vec!["Alice".to_string()]);
    }

    #[test]
    fn test_pipeline() {
        let scan = Box::new(ScanExecutor::new(sample_data()));
        let filter = Box::new(FilterExecutor::new(scan, 2, "25".into()));
        let mut project = ProjectExecutor::new(filter, vec![1]);
        assert_eq!(project.collect_all(), vec![vec!["Alice".to_string()], vec!["Carol".to_string()]]);
    }
}
