/// Capstone Challenge 2: SQL Expression Evaluator — SOLUTION

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(f64), Add(Box<Expr>, Box<Expr>), Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>), Div(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>), Lt(Box<Expr>, Box<Expr>), Gt(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>), Or(Box<Expr>, Box<Expr>), Not(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalResult { Number(f64), Boolean(bool), Error(String) }

pub fn eval(expr: &Expr) -> EvalResult {
    match expr {
        Expr::Literal(n) => EvalResult::Number(*n),
        Expr::Add(a, b) => num_op(a, b, |x, y| x + y),
        Expr::Sub(a, b) => num_op(a, b, |x, y| x - y),
        Expr::Mul(a, b) => num_op(a, b, |x, y| x * y),
        Expr::Div(a, b) => {
            match (eval(a), eval(b)) {
                (EvalResult::Number(x), EvalResult::Number(y)) => {
                    if y == 0.0 { EvalResult::Error("Division by zero".into()) }
                    else { EvalResult::Number(x / y) }
                }
                _ => EvalResult::Error("Type error".into()),
            }
        }
        Expr::Eq(a, b) => cmp_op(a, b, |x, y| (x - y).abs() < f64::EPSILON),
        Expr::Lt(a, b) => cmp_op(a, b, |x, y| x < y),
        Expr::Gt(a, b) => cmp_op(a, b, |x, y| x > y),
        Expr::And(a, b) => bool_op(a, b, |x, y| x && y),
        Expr::Or(a, b) => bool_op(a, b, |x, y| x || y),
        Expr::Not(a) => match eval(a) {
            EvalResult::Boolean(v) => EvalResult::Boolean(!v),
            _ => EvalResult::Error("NOT requires boolean".into()),
        },
    }
}

fn num_op(a: &Expr, b: &Expr, op: impl Fn(f64, f64) -> f64) -> EvalResult {
    match (eval(a), eval(b)) {
        (EvalResult::Number(x), EvalResult::Number(y)) => EvalResult::Number(op(x, y)),
        _ => EvalResult::Error("Type error".into()),
    }
}

fn cmp_op(a: &Expr, b: &Expr, op: impl Fn(f64, f64) -> bool) -> EvalResult {
    match (eval(a), eval(b)) {
        (EvalResult::Number(x), EvalResult::Number(y)) => EvalResult::Boolean(op(x, y)),
        _ => EvalResult::Error("Type error".into()),
    }
}

fn bool_op(a: &Expr, b: &Expr, op: impl Fn(bool, bool) -> bool) -> EvalResult {
    match (eval(a), eval(b)) {
        (EvalResult::Boolean(x), EvalResult::Boolean(y)) => EvalResult::Boolean(op(x, y)),
        _ => EvalResult::Error("Type error".into()),
    }
}

fn main() {
    println!("Capstone Challenge 2: Expression Eval — Solution");
    let e = Expr::Add(Box::new(Expr::Literal(2.0)), Box::new(Expr::Literal(3.0)));
    println!("2 + 3 = {:?}", eval(&e));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_arith() {
        let e = Expr::Mul(Box::new(Expr::Add(Box::new(Expr::Literal(2.0)), Box::new(Expr::Literal(3.0)))), Box::new(Expr::Literal(4.0)));
        assert_eq!(eval(&e), EvalResult::Number(20.0));
    }
    #[test] fn test_cmp() { assert_eq!(eval(&Expr::Lt(Box::new(Expr::Literal(3.0)), Box::new(Expr::Literal(5.0)))), EvalResult::Boolean(true)); }
    #[test] fn test_div0() { assert!(matches!(eval(&Expr::Div(Box::new(Expr::Literal(1.0)), Box::new(Expr::Literal(0.0)))), EvalResult::Error(_))); }
}
