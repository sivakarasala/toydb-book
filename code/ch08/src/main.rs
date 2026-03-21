/// Chapter 8: Query Planner
/// Exercise: Convert an AST into a query plan tree.

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Number(f64), String(String), Column(String),
    BinaryOp { left: Box<Expression>, op: String, right: Box<Expression> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select { columns: Vec<String>, from: String, filter: Option<Expression> },
    Insert { table: String, values: Vec<Expression> },
    CreateTable { name: String, columns: Vec<(String, String)> },
}

/// A query plan node.
#[derive(Debug, Clone)]
pub enum Plan {
    Scan { table: String },
    Filter { predicate: Expression, source: Box<Plan> },
    Project { columns: Vec<String>, source: Box<Plan> },
    Insert { table: String, values: Vec<Expression> },
    CreateTable { name: String, columns: Vec<(String, String)> },
}

pub struct Planner;

impl Planner {
    /// Convert a Statement into a Plan.
    pub fn plan(stmt: Statement) -> Plan {
        // TODO: Match on statement type:
        // Select → Scan, optionally wrap in Filter, then wrap in Project
        // Insert → Plan::Insert
        // CreateTable → Plan::CreateTable
        todo!("Implement plan")
    }

    /// Format a plan for EXPLAIN output.
    pub fn explain(plan: &Plan, indent: usize) -> String {
        // TODO: Recursively format the plan tree with indentation
        todo!("Implement explain")
    }
}

fn main() {
    println!("=== Chapter 8: Query Planner ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_plan() {
        let stmt = Statement::Select {
            columns: vec!["name".into()],
            from: "users".into(),
            filter: None,
        };
        let plan = Planner::plan(stmt);
        match plan {
            Plan::Project { columns, source } => {
                assert_eq!(columns, vec!["name"]);
                match *source { Plan::Scan { table } => assert_eq!(table, "users"), _ => panic!() }
            }
            _ => panic!("Expected Project(Scan)"),
        }
    }

    #[test]
    fn test_select_with_filter() {
        let stmt = Statement::Select {
            columns: vec!["name".into()],
            from: "users".into(),
            filter: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".into())),
                op: ">".into(),
                right: Box::new(Expression::Number(21.0)),
            }),
        };
        let plan = Planner::plan(stmt);
        match plan {
            Plan::Project { source, .. } => {
                match *source { Plan::Filter { source, .. } => {
                    match *source { Plan::Scan { .. } => {} _ => panic!() }
                } _ => panic!() }
            }
            _ => panic!("Expected Project(Filter(Scan))"),
        }
    }

    #[test]
    fn test_explain() {
        let plan = Plan::Project {
            columns: vec!["name".into()],
            source: Box::new(Plan::Scan { table: "users".into() }),
        };
        let output = Planner::explain(&plan, 0);
        assert!(output.contains("Project"));
        assert!(output.contains("Scan"));
    }
}
