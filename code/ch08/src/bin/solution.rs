/// Chapter 8: Query Planner — SOLUTION

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
    pub fn plan(stmt: Statement) -> Plan {
        match stmt {
            Statement::Select { columns, from, filter } => {
                let mut plan = Plan::Scan { table: from };
                if let Some(pred) = filter {
                    plan = Plan::Filter { predicate: pred, source: Box::new(plan) };
                }
                Plan::Project { columns, source: Box::new(plan) }
            }
            Statement::Insert { table, values } => Plan::Insert { table, values },
            Statement::CreateTable { name, columns } => Plan::CreateTable { name, columns },
        }
    }

    pub fn explain(plan: &Plan, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match plan {
            Plan::Scan { table } => format!("{pad}Scan: {table}\n"),
            Plan::Filter { predicate, source } => {
                format!("{pad}Filter: {predicate:?}\n{}", Self::explain(source, indent + 1))
            }
            Plan::Project { columns, source } => {
                format!("{pad}Project: [{}]\n{}", columns.join(", "), Self::explain(source, indent + 1))
            }
            Plan::Insert { table, values } => format!("{pad}Insert into {table}: {values:?}\n"),
            Plan::CreateTable { name, columns } => format!("{pad}CreateTable {name}: {columns:?}\n"),
        }
    }
}

fn main() {
    println!("=== Chapter 8: Query Planner — Solution ===");
    let stmt = Statement::Select {
        columns: vec!["name".into(), "age".into()],
        from: "users".into(),
        filter: Some(Expression::BinaryOp {
            left: Box::new(Expression::Column("age".into())),
            op: ">".into(),
            right: Box::new(Expression::Number(21.0)),
        }),
    };
    let plan = Planner::plan(stmt);
    println!("EXPLAIN:\n{}", Planner::explain(&plan, 0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_plan() {
        let stmt = Statement::Select { columns: vec!["name".into()], from: "users".into(), filter: None };
        match Planner::plan(stmt) {
            Plan::Project { columns, source } => {
                assert_eq!(columns, vec!["name"]);
                match *source { Plan::Scan { table } => assert_eq!(table, "users"), _ => panic!() }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_select_with_filter() {
        let stmt = Statement::Select {
            columns: vec!["name".into()], from: "users".into(),
            filter: Some(Expression::BinaryOp { left: Box::new(Expression::Column("age".into())), op: ">".into(), right: Box::new(Expression::Number(21.0)) }),
        };
        match Planner::plan(stmt) {
            Plan::Project { source, .. } => match *source { Plan::Filter { source, .. } => match *source { Plan::Scan { .. } => {} _ => panic!() }, _ => panic!() },
            _ => panic!(),
        }
    }

    #[test]
    fn test_explain() {
        let plan = Plan::Project { columns: vec!["name".into()], source: Box::new(Plan::Scan { table: "users".into() }) };
        let out = Planner::explain(&plan, 0);
        assert!(out.contains("Project"));
        assert!(out.contains("Scan"));
    }
}
