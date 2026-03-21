/// Capstone Challenge 3: Query Plan Builder — SOLUTION

#[derive(Debug, Clone)]
pub enum PlanNode {
    Scan { table: String },
    Filter { predicate: String, child: Box<PlanNode> },
    Project { columns: Vec<String>, child: Box<PlanNode> },
    Join { left: Box<PlanNode>, right: Box<PlanNode>, on: String },
    Sort { key: String, child: Box<PlanNode> },
    Limit { count: usize, child: Box<PlanNode> },
}

impl PlanNode {
    pub fn node_count(&self) -> usize {
        1 + match self {
            PlanNode::Scan { .. } => 0,
            PlanNode::Filter { child, .. } | PlanNode::Project { child, .. }
            | PlanNode::Sort { child, .. } | PlanNode::Limit { child, .. } => child.node_count(),
            PlanNode::Join { left, right, .. } => left.node_count() + right.node_count(),
        }
    }

    pub fn depth(&self) -> usize {
        1 + match self {
            PlanNode::Scan { .. } => 0,
            PlanNode::Filter { child, .. } | PlanNode::Project { child, .. }
            | PlanNode::Sort { child, .. } | PlanNode::Limit { child, .. } => child.depth(),
            PlanNode::Join { left, right, .. } => left.depth().max(right.depth()),
        }
    }

    pub fn tables(&self) -> Vec<String> {
        match self {
            PlanNode::Scan { table } => vec![table.clone()],
            PlanNode::Filter { child, .. } | PlanNode::Project { child, .. }
            | PlanNode::Sort { child, .. } | PlanNode::Limit { child, .. } => child.tables(),
            PlanNode::Join { left, right, .. } => {
                let mut t = left.tables();
                t.extend(right.tables());
                t
            }
        }
    }

    pub fn push_filter_down(self) -> PlanNode {
        match self {
            PlanNode::Filter { predicate, child } => match *child {
                PlanNode::Project { columns, child: inner } => PlanNode::Project {
                    columns,
                    child: Box::new(PlanNode::Filter { predicate, child: inner }),
                },
                other => PlanNode::Filter { predicate, child: Box::new(other) },
            },
            other => other,
        }
    }
}

fn main() { println!("Capstone 3: Query Plan Builder — Solution"); }

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> PlanNode {
        PlanNode::Limit { count: 10, child: Box::new(PlanNode::Sort { key: "age".into(),
            child: Box::new(PlanNode::Project { columns: vec!["name".into()],
                child: Box::new(PlanNode::Filter { predicate: "age > 21".into(),
                    child: Box::new(PlanNode::Scan { table: "users".into() }) }) }) }) }
    }
    #[test] fn test_count() { assert_eq!(sample().node_count(), 5); }
    #[test] fn test_depth() { assert_eq!(sample().depth(), 5); }
    #[test] fn test_tables() {
        let p = PlanNode::Join { left: Box::new(PlanNode::Scan { table: "a".into() }), right: Box::new(PlanNode::Scan { table: "b".into() }), on: "id".into() };
        assert_eq!(p.tables().len(), 2);
    }
}
