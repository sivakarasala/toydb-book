/// Chapter 9: Query Optimizer
/// Exercise: Build optimizer rules using trait objects.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64), Column(String), Bool(bool),
    BinaryOp { left: Box<Expr>, op: String, right: Box<Expr> },
}

#[derive(Debug, Clone)]
pub enum Plan {
    Scan { table: String },
    Filter { predicate: Expr, source: Box<Plan> },
    Project { columns: Vec<String>, source: Box<Plan> },
}

/// An optimization rule.
pub trait OptimizerRule {
    fn name(&self) -> &str;
    fn optimize(&self, plan: Plan) -> Plan;
}

/// Folds constant expressions: 1 + 2 → 3, true AND true → true
pub struct ConstantFolding;

impl OptimizerRule for ConstantFolding {
    fn name(&self) -> &str { "ConstantFolding" }

    fn optimize(&self, plan: Plan) -> Plan {
        // TODO: Walk the plan tree. For Filter nodes, try to fold the predicate.
        // If predicate is BinaryOp with two Number operands and op is "+", compute the result.
        // If predicate folds to Bool(true), remove the Filter entirely.
        todo!("Implement ConstantFolding")
    }
}

/// Pushes filters below projections.
pub struct FilterPushdown;

impl OptimizerRule for FilterPushdown {
    fn name(&self) -> &str { "FilterPushdown" }

    fn optimize(&self, plan: Plan) -> Plan {
        // TODO: If plan is Project(Filter(source)), rewrite to Filter(Project(source))
        // This is a simplified pushdown — in real DBs it's more nuanced
        todo!("Implement FilterPushdown")
    }
}

/// The optimizer applies rules in sequence.
pub struct Optimizer {
    rules: Vec<Box<dyn OptimizerRule>>,
}

impl Optimizer {
    pub fn new() -> Self {
        // TODO: Create with ConstantFolding and FilterPushdown rules
        todo!("Implement new")
    }

    pub fn optimize(&self, mut plan: Plan) -> Plan {
        // TODO: Apply each rule in sequence
        todo!("Implement optimize")
    }
}

fn main() {
    println!("=== Chapter 9: Query Optimizer ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding() {
        let plan = Plan::Filter {
            predicate: Expr::BinaryOp {
                left: Box::new(Expr::Number(1.0)),
                op: "+".into(),
                right: Box::new(Expr::Number(2.0)),
            },
            source: Box::new(Plan::Scan { table: "t".into() }),
        };
        let folded = ConstantFolding.optimize(plan);
        match folded {
            Plan::Filter { predicate: Expr::Number(n), .. } => assert_eq!(n, 3.0),
            _ => panic!("Expected folded number"),
        }
    }

    #[test]
    fn test_true_filter_removal() {
        let plan = Plan::Filter {
            predicate: Expr::Bool(true),
            source: Box::new(Plan::Scan { table: "t".into() }),
        };
        let optimized = ConstantFolding.optimize(plan);
        match optimized {
            Plan::Scan { table } => assert_eq!(table, "t"),
            _ => panic!("Expected Filter removed"),
        }
    }

    #[test]
    fn test_optimizer_applies_rules() {
        let optimizer = Optimizer::new();
        let plan = Plan::Filter {
            predicate: Expr::Bool(true),
            source: Box::new(Plan::Scan { table: "t".into() }),
        };
        let optimized = optimizer.optimize(plan);
        match optimized {
            Plan::Scan { .. } => {}
            _ => panic!("Expected optimized away"),
        }
    }
}
