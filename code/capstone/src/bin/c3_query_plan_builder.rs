/// Capstone Challenge 3: Query Plan Builder
/// Build a query plan tree from a simplified query description.

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
    /// Count the total number of nodes in the plan tree
    pub fn node_count(&self) -> usize {
        // TODO: Count this node + recursively count children
        todo!("Implement node_count")
    }

    /// Return the depth (height) of the plan tree
    pub fn depth(&self) -> usize {
        // TODO: Return 1 + max depth of children
        todo!("Implement depth")
    }

    /// Collect all table names referenced in Scan nodes
    pub fn tables(&self) -> Vec<String> {
        // TODO: Walk the tree, collect table names from Scan nodes
        todo!("Implement tables")
    }

    /// Push a filter down as close to the scan as possible (simplified)
    /// If the child is a Project, swap Filter below Project.
    pub fn push_filter_down(self) -> PlanNode {
        // TODO: If self is Filter and child is Project, restructure:
        //   Filter(pred, Project(cols, X)) → Project(cols, Filter(pred, X))
        // Otherwise return self unchanged.
        todo!("Implement push_filter_down")
    }
}

fn main() {
    println!("Capstone Challenge 3: Query Plan Builder");
    println!("Run `cargo test --bin c3-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> PlanNode {
        PlanNode::Limit {
            count: 10,
            child: Box::new(PlanNode::Sort {
                key: "age".into(),
                child: Box::new(PlanNode::Project {
                    columns: vec!["name".into(), "age".into()],
                    child: Box::new(PlanNode::Filter {
                        predicate: "age > 21".into(),
                        child: Box::new(PlanNode::Scan { table: "users".into() }),
                    }),
                }),
            }),
        }
    }

    #[test]
    fn test_node_count() {
        assert_eq!(sample_plan().node_count(), 5);
    }

    #[test]
    fn test_depth() {
        assert_eq!(sample_plan().depth(), 5);
    }

    #[test]
    fn test_tables() {
        let plan = PlanNode::Join {
            left: Box::new(PlanNode::Scan { table: "users".into() }),
            right: Box::new(PlanNode::Scan { table: "orders".into() }),
            on: "user_id".into(),
        };
        let mut tables = plan.tables();
        tables.sort();
        assert_eq!(tables, vec!["orders", "users"]);
    }

    #[test]
    fn test_push_filter_down() {
        let plan = PlanNode::Filter {
            predicate: "age > 21".into(),
            child: Box::new(PlanNode::Project {
                columns: vec!["name".into()],
                child: Box::new(PlanNode::Scan { table: "users".into() }),
            }),
        };
        let optimized = plan.push_filter_down();
        match optimized {
            PlanNode::Project { child, .. } => {
                assert!(matches!(*child, PlanNode::Filter { .. }));
            }
            _ => panic!("Expected Project at top after pushdown"),
        }
    }
}
