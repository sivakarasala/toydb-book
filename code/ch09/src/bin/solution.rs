/// Chapter 9: Query Optimizer — SOLUTION

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

pub trait OptimizerRule {
    fn name(&self) -> &str;
    fn optimize(&self, plan: Plan) -> Plan;
}

pub struct ConstantFolding;
impl OptimizerRule for ConstantFolding {
    fn name(&self) -> &str { "ConstantFolding" }
    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            Plan::Filter { predicate, source } => {
                let source = Box::new(self.optimize(*source));
                let folded = Self::fold_expr(predicate);
                if folded == Expr::Bool(true) { return *source; }
                Plan::Filter { predicate: folded, source }
            }
            Plan::Project { columns, source } => {
                Plan::Project { columns, source: Box::new(self.optimize(*source)) }
            }
            other => other,
        }
    }
}

impl ConstantFolding {
    fn fold_expr(expr: Expr) -> Expr {
        match expr {
            Expr::BinaryOp { left, op, right } => {
                let l = Self::fold_expr(*left);
                let r = Self::fold_expr(*right);
                match (&l, op.as_str(), &r) {
                    (Expr::Number(a), "+", Expr::Number(b)) => Expr::Number(a + b),
                    (Expr::Number(a), "-", Expr::Number(b)) => Expr::Number(a - b),
                    (Expr::Number(a), "*", Expr::Number(b)) => Expr::Number(a * b),
                    _ => Expr::BinaryOp { left: Box::new(l), op, right: Box::new(r) },
                }
            }
            other => other,
        }
    }
}

pub struct FilterPushdown;
impl OptimizerRule for FilterPushdown {
    fn name(&self) -> &str { "FilterPushdown" }
    fn optimize(&self, plan: Plan) -> Plan {
        match plan {
            Plan::Project { columns, source } => {
                match *source {
                    Plan::Filter { predicate, source: inner } => {
                        Plan::Filter {
                            predicate,
                            source: Box::new(Plan::Project { columns, source: inner }),
                        }
                    }
                    other => Plan::Project { columns, source: Box::new(self.optimize(other)) },
                }
            }
            Plan::Filter { predicate, source } => {
                Plan::Filter { predicate, source: Box::new(self.optimize(*source)) }
            }
            other => other,
        }
    }
}

pub struct Optimizer { rules: Vec<Box<dyn OptimizerRule>> }
impl Optimizer {
    pub fn new() -> Self {
        Optimizer { rules: vec![Box::new(ConstantFolding), Box::new(FilterPushdown)] }
    }
    pub fn optimize(&self, mut plan: Plan) -> Plan {
        for rule in &self.rules { plan = rule.optimize(plan); }
        plan
    }
}

fn main() {
    println!("=== Chapter 9: Optimizer — Solution ===");
    let plan = Plan::Filter {
        predicate: Expr::BinaryOp { left: Box::new(Expr::Number(1.0)), op: "+".into(), right: Box::new(Expr::Number(2.0)) },
        source: Box::new(Plan::Scan { table: "t".into() }),
    };
    println!("Before: {plan:?}");
    let optimized = Optimizer::new().optimize(plan);
    println!("After:  {optimized:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding() {
        let plan = Plan::Filter {
            predicate: Expr::BinaryOp { left: Box::new(Expr::Number(1.0)), op: "+".into(), right: Box::new(Expr::Number(2.0)) },
            source: Box::new(Plan::Scan { table: "t".into() }),
        };
        match ConstantFolding.optimize(plan) {
            Plan::Filter { predicate: Expr::Number(n), .. } => assert_eq!(n, 3.0),
            _ => panic!(),
        }
    }

    #[test]
    fn test_true_filter_removal() {
        let plan = Plan::Filter { predicate: Expr::Bool(true), source: Box::new(Plan::Scan { table: "t".into() }) };
        match ConstantFolding.optimize(plan) { Plan::Scan { .. } => {} _ => panic!() }
    }

    #[test]
    fn test_optimizer_applies_rules() {
        let plan = Plan::Filter { predicate: Expr::Bool(true), source: Box::new(Plan::Scan { table: "t".into() }) };
        match Optimizer::new().optimize(plan) { Plan::Scan { .. } => {} _ => panic!() }
    }
}
