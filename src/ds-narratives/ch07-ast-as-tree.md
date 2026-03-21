# AST as a Tree — "SELECT is a tree, not a sentence"

Look at the expression `1 + 2 * 3`. It looks linear -- five symbols in a row. But ask anyone what it equals, and they say 7, not 9. That means `*` binds tighter than `+`. The expression has hidden structure: `1 + (2 * 3)`. The multiplication happens first, deep inside, and the addition happens at the top.

An Abstract Syntax Tree (AST) makes this hidden structure explicit. Instead of a flat string, you get a tree where `+` is the root, `1` is the left child, and `*` is the right child with its own children `2` and `3`. The deeper a node sits in the tree, the earlier it evaluates. The tree IS the meaning.

SQL queries have the same hidden structure. `SELECT name FROM users WHERE age > 30` is not five words in sequence. It is a tree: a select node with a projection list, a source table, and a filter condition. The filter itself is a tree: `>` with `age` on the left and `30` on the right. Understanding this structure is essential before a database can execute anything.

Let's build expression trees from scratch, evaluate them, print them, and transform them.

---

## The Naive Way

The simplest approach: evaluate expressions with string parsing and immediate computation.

```rust
fn main() {
    // Evaluate "1 + 2 * 3" by parsing left to right
    let expr = "1 + 2 * 3";
    let tokens: Vec<&str> = expr.split_whitespace().collect();

    let mut result: f64 = tokens[0].parse().unwrap();
    let mut i = 1;
    while i < tokens.len() {
        let op = tokens[i];
        let operand: f64 = tokens[i + 1].parse().unwrap();
        match op {
            "+" => result += operand,
            "-" => result -= operand,
            "*" => result *= operand,
            "/" => result /= operand,
            _ => panic!("unknown op"),
        }
        i += 2;
    }

    println!("{} = {} (WRONG! Should be 7)", expr, result);
    // Output: 1 + 2 * 3 = 9 (WRONG! Should be 7)
    // Left-to-right evaluation ignores operator precedence.
}
```

This gives 9 because it evaluates left to right: `(1 + 2) * 3`. To get 7, you need to evaluate `2 * 3` first. But how does the evaluator know to do that? It cannot -- not from a flat sequence. It needs a tree.

---

## The Insight

Think of a family tree. The person at the top is the last to be "created" in the evaluation sense -- they are the final result. Their children were created first. Their grandchildren even earlier. To evaluate a family tree, you start at the leaves (the earliest ancestors) and work your way up.

An expression tree works identically. The leaves are literal values (`1`, `2`, `3`). The internal nodes are operations (`+`, `*`). To evaluate, you recursively evaluate the children first, then apply the operation. The tree structure enforces the correct order automatically:

```text
      +
     / \
    1   *
       / \
      2   3
```

Evaluating this tree: go left, get `1`. Go right, recursively evaluate `*`: go left, get `2`; go right, get `3`; multiply to get `6`. Now apply `+`: `1 + 6 = 7`. The tree forced multiplication to happen before addition because `*` is deeper.

This is exactly what a database parser must build from every SQL expression. `WHERE price * quantity > 1000` becomes a tree with `>` at the root, `*` on the left subtree, and `1000` on the right.

---

## The Build

### The Expression Type

We model the AST as a recursive enum. Each variant is either a leaf (a literal value or a column reference) or an internal node (an operation with children):

```rust
#[derive(Debug, Clone)]
enum Expr {
    // Leaves
    Integer(i64),
    Float(f64),
    Str(String),
    Boolean(bool),
    Column(String),

    // Internal nodes -- each has child expressions
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnOp,
        operand: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum BinOp {
    Add, Sub, Mul, Div,
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    And, Or,
}

#[derive(Debug, Clone)]
enum UnOp {
    Negate,
    Not,
}
```

The `Box<Expr>` is essential. Without it, `Expr` would be infinitely sized -- a `BinaryOp` contains two `Expr` values, which could each be `BinaryOp` values, and so on. `Box` puts the children on the heap, giving each `Expr` a fixed size (a pointer instead of an inline value).

### Building Trees by Hand

Before we build a parser, let's construct trees manually to understand the structure:

```rust
fn make_example() -> Expr {
    // Build the tree for: 1 + 2 * 3
    //       +
    //      / \
    //     1   *
    //        / \
    //       2   3
    Expr::BinaryOp {
        op: BinOp::Add,
        left: Box::new(Expr::Integer(1)),
        right: Box::new(Expr::BinaryOp {
            op: BinOp::Mul,
            left: Box::new(Expr::Integer(2)),
            right: Box::new(Expr::Integer(3)),
        }),
    }
}
```

### Recursive Evaluation

The evaluator walks the tree recursively. Leaves return their value directly. Internal nodes evaluate their children first, then apply the operation:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Integer(i64),
    Float(f64),
    Str(String),
    Boolean(bool),
    Null,
}

fn eval(expr: &Expr) -> Value {
    match expr {
        Expr::Integer(n) => Value::Integer(*n),
        Expr::Float(f) => Value::Float(*f),
        Expr::Str(s) => Value::Str(s.clone()),
        Expr::Boolean(b) => Value::Boolean(*b),
        Expr::Column(_) => Value::Null, // would look up in a row

        Expr::UnaryOp { op, operand } => {
            let val = eval(operand);
            match (op, val) {
                (UnOp::Negate, Value::Integer(n)) => Value::Integer(-n),
                (UnOp::Negate, Value::Float(f)) => Value::Float(-f),
                (UnOp::Not, Value::Boolean(b)) => Value::Boolean(!b),
                _ => Value::Null,
            }
        }

        Expr::BinaryOp { op, left, right } => {
            let l = eval(left);
            let r = eval(right);
            match (op, l, r) {
                // Arithmetic on integers
                (BinOp::Add, Value::Integer(a), Value::Integer(b)) => Value::Integer(a + b),
                (BinOp::Sub, Value::Integer(a), Value::Integer(b)) => Value::Integer(a - b),
                (BinOp::Mul, Value::Integer(a), Value::Integer(b)) => Value::Integer(a * b),
                (BinOp::Div, Value::Integer(a), Value::Integer(b)) => {
                    if b == 0 { Value::Null } else { Value::Integer(a / b) }
                }
                // Comparisons on integers
                (BinOp::Eq, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a == b),
                (BinOp::NotEq, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a != b),
                (BinOp::Lt, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a < b),
                (BinOp::LtEq, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a <= b),
                (BinOp::Gt, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a > b),
                (BinOp::GtEq, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a >= b),
                // Boolean logic
                (BinOp::And, Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a && b),
                (BinOp::Or, Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a || b),
                // String concatenation
                (BinOp::Add, Value::Str(a), Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
                _ => Value::Null,
            }
        }

        Expr::FunctionCall { name, args } => {
            let evaluated: Vec<Value> = args.iter().map(|a| eval(a)).collect();
            match name.to_uppercase().as_str() {
                "ABS" => match &evaluated[0] {
                    Value::Integer(n) => Value::Integer(n.abs()),
                    _ => Value::Null,
                },
                _ => Value::Null,
            }
        }
    }
}
```

### Pretty-Printing

To debug ASTs, we need to visualize them. A recursive pretty-printer with indentation:

```rust
fn pretty_print(expr: &Expr, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::Integer(n) => format!("{}Int({})", pad, n),
        Expr::Float(f) => format!("{}Float({})", pad, f),
        Expr::Str(s) => format!("{}Str(\"{}\")", pad, s),
        Expr::Boolean(b) => format!("{}Bool({})", pad, b),
        Expr::Column(c) => format!("{}Col({})", pad, c),

        Expr::BinaryOp { op, left, right } => {
            format!(
                "{}{:?}\n{}\n{}",
                pad, op,
                pretty_print(left, indent + 1),
                pretty_print(right, indent + 1),
            )
        }

        Expr::UnaryOp { op, operand } => {
            format!(
                "{}{:?}\n{}",
                pad, op,
                pretty_print(operand, indent + 1),
            )
        }

        Expr::FunctionCall { name, args } => {
            let mut result = format!("{}Call({})", pad, name);
            for arg in args {
                result.push_str(&format!("\n{}", pretty_print(arg, indent + 1)));
            }
            result
        }
    }
}
```

### Tree Transformations: Constant Folding

The real power of ASTs is that you can rewrite them. **Constant folding** replaces subtrees that contain only constants with their computed result. The expression `2 * 3 + x` becomes `6 + x` -- the multiplication happens at compile time instead of being repeated for every row.

```rust
fn constant_fold(expr: Expr) -> Expr {
    match expr {
        Expr::BinaryOp { op, left, right } => {
            let left = constant_fold(*left);
            let right = constant_fold(*right);

            // If both sides are constants, compute the result
            if let (Expr::Integer(a), Expr::Integer(b)) = (&left, &right) {
                let result = match op {
                    BinOp::Add => Some(a + b),
                    BinOp::Sub => Some(a - b),
                    BinOp::Mul => Some(a * b),
                    BinOp::Div => if *b != 0 { Some(a / b) } else { None },
                    _ => None,
                };
                if let Some(val) = result {
                    return Expr::Integer(val);
                }
            }

            // Can't fold -- return with folded children
            Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            }
        }

        Expr::UnaryOp { op, operand } => {
            let operand = constant_fold(*operand);
            if let (UnOp::Negate, Expr::Integer(n)) = (&op, &operand) {
                return Expr::Integer(-n);
            }
            Expr::UnaryOp { op, operand: Box::new(operand) }
        }

        // Leaves and function calls pass through unchanged
        other => other,
    }
}
```

Notice how the transformation works bottom-up: fold the children first, then check if the current node can be folded. This is the natural recursion pattern for tree rewriting. `2 * 3 + 4 * 5` folds to `6 + 20`, then to `26`, in a single pass.

---

## The Payoff

Here is the full, runnable implementation:

```rust
#[derive(Debug, Clone, PartialEq)]
enum BinOp { Add, Sub, Mul, Div, Eq, NotEq, Lt, LtEq, Gt, GtEq, And, Or }
#[derive(Debug, Clone)] enum UnOp { Negate, Not }

#[derive(Debug, Clone)]
enum Expr {
    Integer(i64), Float(f64), Str(String), Boolean(bool), Column(String),
    BinaryOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnOp, operand: Box<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
enum Value { Integer(i64), Float(f64), Str(String), Boolean(bool), Null }

fn eval(expr: &Expr) -> Value {
    match expr {
        Expr::Integer(n) => Value::Integer(*n),
        Expr::Float(f) => Value::Float(*f),
        Expr::Str(s) => Value::Str(s.clone()),
        Expr::Boolean(b) => Value::Boolean(*b),
        Expr::Column(_) => Value::Null,
        Expr::UnaryOp { op, operand } => {
            match (op, eval(operand)) {
                (UnOp::Negate, Value::Integer(n)) => Value::Integer(-n),
                (UnOp::Not, Value::Boolean(b)) => Value::Boolean(!b),
                _ => Value::Null,
            }
        }
        Expr::BinaryOp { op, left, right } => {
            match (op, eval(left), eval(right)) {
                (BinOp::Add, Value::Integer(a), Value::Integer(b)) => Value::Integer(a + b),
                (BinOp::Sub, Value::Integer(a), Value::Integer(b)) => Value::Integer(a - b),
                (BinOp::Mul, Value::Integer(a), Value::Integer(b)) => Value::Integer(a * b),
                (BinOp::Div, Value::Integer(a), Value::Integer(b)) if b != 0 => Value::Integer(a / b),
                (BinOp::Gt, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a > b),
                (BinOp::Lt, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a < b),
                (BinOp::Eq, Value::Integer(a), Value::Integer(b)) => Value::Boolean(a == b),
                (BinOp::And, Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a && b),
                (BinOp::Or, Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a || b),
                (BinOp::Add, Value::Str(a), Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
                _ => Value::Null,
            }
        }
        Expr::FunctionCall { name, args } => {
            let vals: Vec<Value> = args.iter().map(|a| eval(a)).collect();
            match name.to_uppercase().as_str() {
                "ABS" => match &vals[0] { Value::Integer(n) => Value::Integer(n.abs()), _ => Value::Null },
                _ => Value::Null,
            }
        }
    }
}

fn pretty(expr: &Expr, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::Integer(n) => format!("{}Int({})", pad, n),
        Expr::Float(f) => format!("{}Float({})", pad, f),
        Expr::Str(s) => format!("{}Str(\"{}\")", pad, s),
        Expr::Boolean(b) => format!("{}Bool({})", pad, b),
        Expr::Column(c) => format!("{}Col({})", pad, c),
        Expr::BinaryOp { op, left, right } =>
            format!("{}{:?}\n{}\n{}", pad, op, pretty(left, indent+1), pretty(right, indent+1)),
        Expr::UnaryOp { op, operand } =>
            format!("{}{:?}\n{}", pad, op, pretty(operand, indent+1)),
        Expr::FunctionCall { name, args } => {
            let mut s = format!("{}Call({})", pad, name);
            for a in args { s.push_str(&format!("\n{}", pretty(a, indent+1))); }
            s
        }
    }
}

fn fold(expr: Expr) -> Expr {
    match expr {
        Expr::BinaryOp { op, left, right } => {
            let l = fold(*left); let r = fold(*right);
            if let (Expr::Integer(a), Expr::Integer(b)) = (&l, &r) {
                let res = match op {
                    BinOp::Add => Some(a + b), BinOp::Sub => Some(a - b),
                    BinOp::Mul => Some(a * b),
                    BinOp::Div => if *b != 0 { Some(a / b) } else { None },
                    _ => None,
                };
                if let Some(v) = res { return Expr::Integer(v); }
            }
            Expr::BinaryOp { op, left: Box::new(l), right: Box::new(r) }
        }
        Expr::UnaryOp { op, operand } => {
            let o = fold(*operand);
            if let (UnOp::Negate, Expr::Integer(n)) = (&op, &o) { return Expr::Integer(-n); }
            Expr::UnaryOp { op, operand: Box::new(o) }
        }
        other => other,
    }
}

fn main() {
    // 1. Build and evaluate: 1 + 2 * 3
    let expr = Expr::BinaryOp {
        op: BinOp::Add,
        left: Box::new(Expr::Integer(1)),
        right: Box::new(Expr::BinaryOp {
            op: BinOp::Mul,
            left: Box::new(Expr::Integer(2)),
            right: Box::new(Expr::Integer(3)),
        }),
    };

    println!("=== Expression: 1 + 2 * 3 ===");
    println!("{}", pretty(&expr, 0));
    println!("Result: {:?}\n", eval(&expr));

    // 2. SQL-like: WHERE age > 30 AND name = 'Alice'
    let where_expr = Expr::BinaryOp {
        op: BinOp::And,
        left: Box::new(Expr::BinaryOp {
            op: BinOp::Gt,
            left: Box::new(Expr::Column("age".into())),
            right: Box::new(Expr::Integer(30)),
        }),
        right: Box::new(Expr::BinaryOp {
            op: BinOp::Eq,
            left: Box::new(Expr::Column("name".into())),
            right: Box::new(Expr::Str("Alice".into())),
        }),
    };

    println!("=== WHERE age > 30 AND name = 'Alice' ===");
    println!("{}\n", pretty(&where_expr, 0));

    // 3. Constant folding: 2 * 3 + 4 * 5 + x
    let complex = Expr::BinaryOp {
        op: BinOp::Add,
        left: Box::new(Expr::BinaryOp {
            op: BinOp::Add,
            left: Box::new(Expr::BinaryOp {
                op: BinOp::Mul,
                left: Box::new(Expr::Integer(2)),
                right: Box::new(Expr::Integer(3)),
            }),
            right: Box::new(Expr::BinaryOp {
                op: BinOp::Mul,
                left: Box::new(Expr::Integer(4)),
                right: Box::new(Expr::Integer(5)),
            }),
        }),
        right: Box::new(Expr::Column("x".into())),
    };

    println!("=== Before constant folding: 2*3 + 4*5 + x ===");
    println!("{}\n", pretty(&complex, 0));

    let folded = fold(complex);
    println!("=== After constant folding ===");
    println!("{}\n", pretty(&folded, 0));
    // The constant parts (2*3=6, 4*5=20, 6+20=26) are folded.
    // Result: 26 + x

    // 4. Function call: ABS(-42)
    let func = Expr::FunctionCall {
        name: "ABS".into(),
        args: vec![Expr::UnaryOp {
            op: UnOp::Negate,
            operand: Box::new(Expr::Integer(42)),
        }],
    };
    println!("=== ABS(-42) ===");
    println!("{}", pretty(&func, 0));
    println!("Result: {:?}", eval(&func));
}
```

The tree structure enforces evaluation order. Constant folding eliminates unnecessary computation at plan time. Pretty-printing makes debugging possible. These are the three things a database does with every AST it builds.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Build tree (n nodes) | O(n) | O(n) | One allocation per node |
| Evaluate tree | O(n) | O(d) stack | d = tree depth (recursion depth) |
| Pretty-print | O(n) | O(n * d) | String building at each level |
| Constant folding | O(n) | O(n) | Single bottom-up pass |
| Tree depth (balanced) | -- | O(log n) | Operators with two children |
| Tree depth (worst case) | -- | O(n) | Chained unary ops or left-associative |

The key insight: tree depth matters for stack usage. A deeply nested expression like `1 + 2 + 3 + ... + 1000` creates a left-leaning tree of depth 999. Each recursive `eval` call adds a stack frame. For very deep trees, you might need to convert to an iterative evaluator -- but for typical SQL expressions (depth < 50), recursion is fine.

---

## Where This Shows Up in Our Database

In Chapter 7, we build the SQL parser that converts token streams into ASTs:

```rust,ignore
// After lexing "SELECT name FROM users WHERE age > 30",
// the parser builds:
//
// SelectStatement {
//     columns: [Column("name")],
//     from: Table("users"),
//     where_clause: Some(BinaryOp {
//         op: Gt,
//         left: Column("age"),
//         right: Integer(30),
//     }),
// }
```

ASTs are the universal intermediate representation in database systems:
- **Query planners** convert ASTs into execution plans (scan, filter, join nodes)
- **Query optimizers** rewrite ASTs for better performance (predicate pushdown, join reordering)
- **Type checkers** walk ASTs to verify that operations are valid (you cannot add a string to an integer)
- **Code generators** in JIT-compiled databases convert ASTs to machine code

Every SQL database, from SQLite to PostgreSQL to Snowflake, builds an AST from every query. The tree is the truth.

---

## Try It Yourself

### Exercise 1: Tree Size and Depth

Implement two functions: `tree_size(expr) -> usize` that counts the total number of nodes, and `tree_depth(expr) -> usize` that computes the maximum depth (leaves have depth 1). Test with the expression `(1 + 2) * (3 + 4)` and verify: size = 7 (3 operators + 4 literals), depth = 3.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Expr {
    Integer(i64),
    Column(String),
    BinaryOp { op: String, left: Box<Expr>, right: Box<Expr> },
}

fn tree_size(expr: &Expr) -> usize {
    match expr {
        Expr::Integer(_) | Expr::Column(_) => 1,
        Expr::BinaryOp { left, right, .. } => {
            1 + tree_size(left) + tree_size(right)
        }
    }
}

fn tree_depth(expr: &Expr) -> usize {
    match expr {
        Expr::Integer(_) | Expr::Column(_) => 1,
        Expr::BinaryOp { left, right, .. } => {
            1 + tree_depth(left).max(tree_depth(right))
        }
    }
}

fn main() {
    // (1 + 2) * (3 + 4)
    let expr = Expr::BinaryOp {
        op: "*".into(),
        left: Box::new(Expr::BinaryOp {
            op: "+".into(),
            left: Box::new(Expr::Integer(1)),
            right: Box::new(Expr::Integer(2)),
        }),
        right: Box::new(Expr::BinaryOp {
            op: "+".into(),
            left: Box::new(Expr::Integer(3)),
            right: Box::new(Expr::Integer(4)),
        }),
    };

    println!("Size: {} (expected 7)", tree_size(&expr));
    println!("Depth: {} (expected 3)", tree_depth(&expr));

    // Skewed tree: 1 + 2 + 3 + 4 (left-associative)
    let skewed = Expr::BinaryOp {
        op: "+".into(),
        left: Box::new(Expr::BinaryOp {
            op: "+".into(),
            left: Box::new(Expr::BinaryOp {
                op: "+".into(),
                left: Box::new(Expr::Integer(1)),
                right: Box::new(Expr::Integer(2)),
            }),
            right: Box::new(Expr::Integer(3)),
        }),
        right: Box::new(Expr::Integer(4)),
    };

    println!("Skewed size: {} (expected 7)", tree_size(&skewed));
    println!("Skewed depth: {} (expected 4)", tree_depth(&skewed));
}
```

</details>

### Exercise 2: Expression Infix Printer

Write a function `to_infix(expr) -> String` that converts an AST back to a human-readable infix string with minimal parentheses. For example, `Add(1, Mul(2, 3))` should produce `"1 + 2 * 3"` (no parens needed because `*` binds tighter), but `Mul(Add(1, 2), 3)` should produce `"(1 + 2) * 3"` (parens needed).

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Op { Add, Sub, Mul, Div }

#[derive(Debug, Clone)]
enum Expr {
    Int(i64),
    BinOp { op: Op, left: Box<Expr>, right: Box<Expr> },
}

fn precedence(op: &Op) -> u8 {
    match op {
        Op::Add | Op::Sub => 1,
        Op::Mul | Op::Div => 2,
    }
}

fn op_str(op: &Op) -> &str {
    match op { Op::Add => "+", Op::Sub => "-", Op::Mul => "*", Op::Div => "/" }
}

fn to_infix(expr: &Expr) -> String {
    match expr {
        Expr::Int(n) => n.to_string(),
        Expr::BinOp { op, left, right } => {
            let left_str = match left.as_ref() {
                Expr::BinOp { op: child_op, .. } if precedence(child_op) < precedence(op) => {
                    format!("({})", to_infix(left))
                }
                _ => to_infix(left),
            };

            let right_str = match right.as_ref() {
                Expr::BinOp { op: child_op, .. } if precedence(child_op) < precedence(op) => {
                    format!("({})", to_infix(right))
                }
                // Also parenthesize right child with same precedence for - and /
                // because they are left-associative: 1 - (2 - 3) != 1 - 2 - 3
                Expr::BinOp { op: child_op, .. }
                    if precedence(child_op) == precedence(op)
                    && matches!(op, Op::Sub | Op::Div) =>
                {
                    format!("({})", to_infix(right))
                }
                _ => to_infix(right),
            };

            format!("{} {} {}", left_str, op_str(op), right_str)
        }
    }
}

fn main() {
    // 1 + 2 * 3 (no parens needed)
    let e1 = Expr::BinOp {
        op: Op::Add,
        left: Box::new(Expr::Int(1)),
        right: Box::new(Expr::BinOp {
            op: Op::Mul,
            left: Box::new(Expr::Int(2)),
            right: Box::new(Expr::Int(3)),
        }),
    };
    println!("{}", to_infix(&e1)); // "1 + 2 * 3"

    // (1 + 2) * 3 (parens needed)
    let e2 = Expr::BinOp {
        op: Op::Mul,
        left: Box::new(Expr::BinOp {
            op: Op::Add,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(2)),
        }),
        right: Box::new(Expr::Int(3)),
    };
    println!("{}", to_infix(&e2)); // "(1 + 2) * 3"

    // 1 - (2 - 3) (parens needed -- subtraction is left-associative)
    let e3 = Expr::BinOp {
        op: Op::Sub,
        left: Box::new(Expr::Int(1)),
        right: Box::new(Expr::BinOp {
            op: Op::Sub,
            left: Box::new(Expr::Int(2)),
            right: Box::new(Expr::Int(3)),
        }),
    };
    println!("{}", to_infix(&e3)); // "1 - (2 - 3)"
}
```

</details>

### Exercise 3: Collect Column References

Write a function `collect_columns(expr) -> Vec<String>` that extracts all column names referenced in an expression. This is useful for a query planner that needs to know which columns a filter touches. Test with `WHERE age > 30 AND name = 'Alice' OR city = 'NYC'` and verify it returns `["age", "name", "city"]`.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Expr {
    Integer(i64),
    Str(String),
    Column(String),
    BinaryOp { op: String, left: Box<Expr>, right: Box<Expr> },
}

fn collect_columns(expr: &Expr) -> Vec<String> {
    let mut result = Vec::new();
    collect_columns_inner(expr, &mut result);
    result
}

fn collect_columns_inner(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Column(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_columns_inner(left, out);
            collect_columns_inner(right, out);
        }
        _ => {} // literals have no column references
    }
}

fn main() {
    // WHERE age > 30 AND name = 'Alice' OR city = 'NYC'
    let expr = Expr::BinaryOp {
        op: "OR".into(),
        left: Box::new(Expr::BinaryOp {
            op: "AND".into(),
            left: Box::new(Expr::BinaryOp {
                op: ">".into(),
                left: Box::new(Expr::Column("age".into())),
                right: Box::new(Expr::Integer(30)),
            }),
            right: Box::new(Expr::BinaryOp {
                op: "=".into(),
                left: Box::new(Expr::Column("name".into())),
                right: Box::new(Expr::Str("Alice".into())),
            }),
        }),
        right: Box::new(Expr::BinaryOp {
            op: "=".into(),
            left: Box::new(Expr::Column("city".into())),
            right: Box::new(Expr::Str("NYC".into())),
        }),
    };

    let cols = collect_columns(&expr);
    println!("Columns: {:?}", cols);
    // ["age", "name", "city"]

    assert_eq!(cols, vec!["age", "name", "city"]);
    println!("Test passed!");
}
```

</details>

---

## Recap

An AST turns a flat string into a structured tree that captures meaning. Leaves are values, internal nodes are operations, and depth encodes evaluation order. Once you have a tree, you can evaluate it recursively, print it for debugging, and transform it for optimization. Every SQL query your database receives becomes a tree before anything else happens. The tree is not just a representation -- it is the canonical form of computation.
