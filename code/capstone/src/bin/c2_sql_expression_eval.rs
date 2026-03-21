/// Capstone Challenge 2: SQL Expression Evaluator
/// Evaluate arithmetic and comparison expressions represented as a tree.

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(f64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalResult {
    Number(f64),
    Boolean(bool),
    Error(String),
}

/// Evaluate an expression tree.
pub fn eval(expr: &Expr) -> EvalResult {
    // TODO: Recursively evaluate the expression tree
    // - Literal → Number
    // - Add/Sub/Mul/Div → evaluate both sides, perform arithmetic
    //   (return Error on division by zero)
    // - Eq/Lt/Gt → evaluate both sides, compare (both must be Number)
    // - And/Or → evaluate both sides, logical ops (both must be Boolean)
    // - Not → evaluate inner, negate (must be Boolean)
    todo!("Implement eval")
}

fn main() {
    println!("Capstone Challenge 2: SQL Expression Evaluator");
    println!("Run `cargo test --bin c2-exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        // (2 + 3) * 4 = 20
        let expr = Expr::Mul(
            Box::new(Expr::Add(Box::new(Expr::Literal(2.0)), Box::new(Expr::Literal(3.0)))),
            Box::new(Expr::Literal(4.0)),
        );
        assert_eq!(eval(&expr), EvalResult::Number(20.0));
    }

    #[test]
    fn test_comparison() {
        let expr = Expr::Lt(Box::new(Expr::Literal(3.0)), Box::new(Expr::Literal(5.0)));
        assert_eq!(eval(&expr), EvalResult::Boolean(true));
    }

    #[test]
    fn test_logical() {
        // true AND (NOT false) = true
        let expr = Expr::And(
            Box::new(Expr::Gt(Box::new(Expr::Literal(5.0)), Box::new(Expr::Literal(3.0)))),
            Box::new(Expr::Not(Box::new(Expr::Lt(Box::new(Expr::Literal(5.0)), Box::new(Expr::Literal(3.0)))))),
        );
        assert_eq!(eval(&expr), EvalResult::Boolean(true));
    }

    #[test]
    fn test_div_by_zero() {
        let expr = Expr::Div(Box::new(Expr::Literal(1.0)), Box::new(Expr::Literal(0.0)));
        assert!(matches!(eval(&expr), EvalResult::Error(_)));
    }
}
