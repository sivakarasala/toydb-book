# Chapter 19: Coding Challenges

Chapters 1 through 18 wove roughly eighteen DSA patterns into the fabric of toydb — BTreeMap for ordered key-value storage, hash maps for in-memory indexes, append-only logs for persistence, tree traversal for AST evaluation, state machines for Raft consensus, iterators for the Volcano executor, and many more. You learned those patterns the best way: by needing them to build a database.

This chapter introduces eight patterns that did not appear organically in the codebase but show up constantly in coding interviews — framed in the database domain. You will see B-tree range queries, expression evaluation, transaction scheduling, and distributed counters. The data structures are the same ones you have been building for eighteen chapters.

For each problem you get: a concrete problem statement, worked examples, a brute-force solution with analysis, an optimized solution with full Rust code, complexity analysis for both, a section connecting the pattern to our database, and interview tips.

The eight patterns:

| # | Problem | Pattern |
|---|---------|---------|
| 1 | KV Range Query | B-Tree Traversal |
| 2 | SQL Expression Evaluator | Tree / Stack |
| 3 | Query Plan Builder | Tree / Dynamic Programming |
| 4 | Transaction Scheduler | Topological Sort |
| 5 | Raft Log Compaction | Sliding Window |
| 6 | Index Scan Optimizer | Greedy / Cost-Based |
| 7 | Deadlock Detector | Cycle Detection |
| 8 | Distributed Counter | CRDTs |

Work through them in order or jump to whichever pattern you want to drill. Each problem stands alone.

---

## Problem 1: KV Range Query

### Problem Statement

You have a key-value store backed by a sorted tree structure (like `BTreeMap`). Implement a `range_query(start, end)` function that returns all key-value pairs where `start <= key < end`, in sorted order. The tree is represented as a simple binary search tree (BST) for this problem.

### Examples

```
Tree:
        10
       /  \
      5    15
     / \   / \
    3   7 12  20

range_query(5, 15) → [(5, "e"), (7, "g"), (10, "j"), (12, "l")]
range_query(1, 6)  → [(3, "c"), (5, "e")]
range_query(20, 25) → [(20, "t")]
range_query(8, 9)  → []
```

### Constraints

- The tree has up to 100,000 nodes
- Keys are unique integers
- The range is half-open: `[start, end)`
- Return results in sorted order

### Brute Force

Do an in-order traversal of the entire tree, collecting all nodes, then filter by range.

```rust,ignore
use std::collections::BTreeMap;

fn range_query_brute(
    tree: &BTreeMap<i64, String>,
    start: i64,
    end: i64,
) -> Vec<(i64, String)> {
    tree.iter()
        .filter(|(&k, _)| k >= start && k < end)
        .map(|(&k, v)| (k, v.clone()))
        .collect()
}
```

**Time:** O(n) — visits every node regardless of the range size.
**Space:** O(n) — collects all nodes before filtering.

This is technically correct with `BTreeMap` since `iter()` is already sorted, but the point is we visit every node. On a hand-built BST, a naive in-order traversal has the same cost.

### Optimized Solution

Use the BST structure to skip entire subtrees. If the current node's key is less than `start`, skip the left subtree entirely. If the key is greater than or equal to `end`, skip the right subtree. This is the core insight behind B-tree range scans in real databases.

```rust
#[derive(Debug)]
struct BstNode {
    key: i64,
    value: String,
    left: Option<Box<BstNode>>,
    right: Option<Box<BstNode>>,
}

impl BstNode {
    fn new(key: i64, value: &str) -> Self {
        BstNode {
            key,
            value: value.to_string(),
            left: None,
            right: None,
        }
    }

    fn insert(&mut self, key: i64, value: &str) {
        if key < self.key {
            match &mut self.left {
                Some(left) => left.insert(key, value),
                None => self.left = Some(Box::new(BstNode::new(key, value))),
            }
        } else if key > self.key {
            match &mut self.right {
                Some(right) => right.insert(key, value),
                None => self.right = Some(Box::new(BstNode::new(key, value))),
            }
        } else {
            self.value = value.to_string(); // update existing key
        }
    }
}

fn range_query(
    node: &Option<Box<BstNode>>,
    start: i64,
    end: i64,
    result: &mut Vec<(i64, String)>,
) {
    let Some(n) = node else { return };

    // If key >= start, the left subtree might have valid keys
    if n.key >= start {
        range_query(&n.left, start, end, result);
    }

    // Include this node if it is in range
    if n.key >= start && n.key < end {
        result.push((n.key, n.value.clone()));
    }

    // If key < end, the right subtree might have valid keys
    if n.key < end {
        range_query(&n.right, start, end, result);
    }
}

fn main() {
    let mut root = BstNode::new(10, "j");
    root.insert(5, "e");
    root.insert(15, "o");
    root.insert(3, "c");
    root.insert(7, "g");
    root.insert(12, "l");
    root.insert(20, "t");

    let mut results = Vec::new();
    range_query(&Some(Box::new(root)), 5, 15, &mut results);
    println!("{:?}", results);
    // [(5, "e"), (7, "g"), (10, "j"), (12, "l")]
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| Brute force | O(n) | O(n) |
| BST range scan | O(log n + k) | O(log n + k) |

Where `n` is the total number of nodes and `k` is the number of results. The optimized solution visits O(log n) nodes to reach the start of the range, then visits O(k) nodes within the range. The space is O(log n) for the recursion stack plus O(k) for the results.

### Connection to Our Database

In Chapter 2, we chose `BTreeMap` over `HashMap` for `MemoryStorage` specifically because it supports ordered iteration. The `scan()` method in the `Storage` trait performs exactly this operation — a range scan over sorted keys. Rust's `BTreeMap::range()` method does the optimized version internally:

```rust,ignore
// Our MemoryStorage.scan() uses BTreeMap's built-in range support
fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
    let results = self.data.range(prefix.to_string()..)
        .take_while(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(results)
}
```

The `BTreeMap::range()` method does exactly what our BST range query does — it navigates to the starting key in O(log n) and then iterates forward. Understanding the algorithm behind it helps you reason about performance when your database serves range queries like `SELECT * FROM users WHERE id BETWEEN 100 AND 200`.

### Interview Tips

- Start by asking whether the data structure is a BST, a balanced BST, or a B-tree. The algorithm is the same, but the analysis differs (balanced trees guarantee O(log n) traversal to the start).
- Mention that `BTreeMap::range()` in Rust and `TreeMap.subMap()` in Java do this natively — interviewers want to know you recognize standard library solutions.
- If asked about disk-based range queries, explain that B-trees store multiple keys per node to minimize disk seeks. The in-memory BST version is the same algorithm with a branching factor of 2 instead of hundreds.
- If asked about concurrent range queries, mention that MVCC (Chapter 5) allows readers to scan without blocking writers — each reader sees a consistent snapshot of the tree.

### Variation: Prefix Range Scan

A common variation is scanning by key prefix rather than numeric range. This is exactly what our `Storage::scan()` method does:

```rust
fn prefix_scan(
    node: &Option<Box<BstNode>>,
    prefix: &str,
    result: &mut Vec<(String, String)>,
) where
{
    let Some(n) = node else { return };

    // If the current key could have the prefix, check left subtree
    if n.value.as_str() >= prefix {
        // For a string BST, we'd use the key field
        // This is simplified for illustration
    }

    // Check if current key starts with prefix
    let key_str = format!("{}", n.key);
    if key_str.starts_with(prefix) {
        result.push((key_str, n.value.clone()));
    }

    // Continue searching right subtree if keys could still match
    // (string ordering means prefix matches are contiguous)
}
```

The key insight for prefix scans: in a sorted structure, all keys with the same prefix are contiguous. Once you find the first matching key, you can iterate forward until you find a non-matching key and stop. This is O(log n + k) — identical to the numeric range query.

---

## Problem 2: SQL Expression Evaluator

### Problem Statement

Given a parsed expression tree (like the one our SQL parser produces), evaluate it against a row of values. The expression tree supports literals, column references, and binary operations (arithmetic and comparison).

### Examples

```
Expression: (column[1] + 10) > column[0]

Row: [100, 50]

Evaluation:
  column[1] = 50
  50 + 10 = 60
  column[0] = 100
  60 > 100 = false

Result: Value::Boolean(false)
```

```
Expression: column[0] * column[1] + column[2]

Row: [3, 4, 5]

Evaluation: 3 * 4 + 5 = 17

Result: Value::Integer(17)
```

### Constraints

- Expressions can be nested to arbitrary depth
- Supported operations: `+`, `-`, `*`, `/`, `>`, `<`, `>=`, `<=`, `=`, `!=`, `AND`, `OR`
- Column indices are zero-based and guaranteed to be in bounds
- Division by zero returns an error
- Type mismatches (e.g., adding a string to an integer) return an error

### Brute Force

Flatten the expression tree to a string, parse it with an eval-like function. This is how scripting languages work but is unsafe and slow.

```rust,ignore
// Pseudocode — do NOT do this in a real system
fn evaluate_brute(expr: &Expression, row: &[Value]) -> Result<Value, String> {
    let expr_string = expr.to_sql_string(row); // "50 + 10 > 100"
    eval(&expr_string) // Parse and evaluate the string
}
```

**Time:** O(n^2) in the worst case — string construction is O(n) and parsing is O(n).
**Space:** O(n) for the intermediate string.

This approach is also a security risk (SQL injection through expression values) and loses type safety. We include it only to contrast with the proper solution.

### Optimized Solution

Recursively evaluate the tree. Each node evaluates its children first, then combines them. This is the same algorithm used in our query executor (Chapter 10).

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Str(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Str(s) => write!(f, "'{}'", s),
        }
    }
}

#[derive(Debug, Clone)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    Gte,
    Lte,
    Eq,
    Neq,
    And,
    Or,
}

#[derive(Debug, Clone)]
enum Expression {
    Literal(Value),
    Column(usize),
    BinaryOp {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Debug)]
enum EvalError {
    DivisionByZero,
    TypeMismatch { op: String, left: String, right: String },
    ColumnOutOfBounds { index: usize, row_len: usize },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::TypeMismatch { op, left, right } => {
                write!(f, "type mismatch: cannot apply {} to {} and {}", op, left, right)
            }
            EvalError::ColumnOutOfBounds { index, row_len } => {
                write!(f, "column {} out of bounds (row has {} columns)", index, row_len)
            }
        }
    }
}

fn evaluate(expr: &Expression, row: &[Value]) -> Result<Value, EvalError> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),

        Expression::Column(idx) => {
            if *idx >= row.len() {
                return Err(EvalError::ColumnOutOfBounds {
                    index: *idx,
                    row_len: row.len(),
                });
            }
            Ok(row[*idx].clone())
        }

        Expression::BinaryOp { op, left, right } => {
            let l = evaluate(left, row)?;
            let r = evaluate(right, row)?;
            apply_op(op, l, r)
        }
    }
}

fn apply_op(op: &BinaryOp, left: Value, right: Value) -> Result<Value, EvalError> {
    match (op, &left, &right) {
        // Arithmetic on integers
        (BinaryOp::Add, Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
        (BinaryOp::Sub, Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
        (BinaryOp::Mul, Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
        (BinaryOp::Div, Value::Integer(_), Value::Integer(0)) => Err(EvalError::DivisionByZero),
        (BinaryOp::Div, Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),

        // Arithmetic on floats
        (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (BinaryOp::Div, Value::Float(_), Value::Float(b)) if *b == 0.0 => {
            Err(EvalError::DivisionByZero)
        }
        (BinaryOp::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),

        // Comparison on integers
        (BinaryOp::Gt, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a > b)),
        (BinaryOp::Lt, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a < b)),
        (BinaryOp::Gte, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a >= b)),
        (BinaryOp::Lte, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a <= b)),
        (BinaryOp::Eq, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a == b)),
        (BinaryOp::Neq, Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a != b)),

        // Comparison on strings
        (BinaryOp::Eq, Value::Str(a), Value::Str(b)) => Ok(Value::Boolean(a == b)),
        (BinaryOp::Neq, Value::Str(a), Value::Str(b)) => Ok(Value::Boolean(a != b)),
        (BinaryOp::Gt, Value::Str(a), Value::Str(b)) => Ok(Value::Boolean(a > b)),
        (BinaryOp::Lt, Value::Str(a), Value::Str(b)) => Ok(Value::Boolean(a < b)),

        // Logical
        (BinaryOp::And, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a && *b)),
        (BinaryOp::Or, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(*a || *b)),

        // Type mismatch
        _ => Err(EvalError::TypeMismatch {
            op: format!("{:?}", op),
            left: format!("{}", left),
            right: format!("{}", right),
        }),
    }
}

fn main() {
    // Expression: (column[1] + 10) > column[0]
    let expr = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::BinaryOp {
            op: BinaryOp::Add,
            left: Box::new(Expression::Column(1)),
            right: Box::new(Expression::Literal(Value::Integer(10))),
        }),
        right: Box::new(Expression::Column(0)),
    };

    let row = vec![Value::Integer(100), Value::Integer(50)];
    let result = evaluate(&expr, &row);
    println!("{:?}", result); // Ok(Boolean(false)) — 60 > 100 is false

    let row2 = vec![Value::Integer(30), Value::Integer(50)];
    let result2 = evaluate(&expr, &row2);
    println!("{:?}", result2); // Ok(Boolean(true)) — 60 > 30 is true
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| String eval (brute) | O(n^2) | O(n) |
| Tree recursion | O(n) | O(d) |

Where `n` is the number of nodes in the expression tree and `d` is the depth. Each node is visited exactly once. The recursion stack is at most `d` deep, which is O(log n) for a balanced expression tree and O(n) in the degenerate case (a chain like `a + b + c + d + ...`).

### Connection to Our Database

This is literally the expression evaluator from Chapter 10. The `evaluate()` function in our query executor has the same structure — recursive descent over an `Expression` enum. The `FilterExecutor` calls this function for every row, passing the WHERE clause expression and the current row. The `ProjectExecutor` calls it for computed columns.

The key insight for interviews: an expression evaluator is a tree traversal. If someone asks you to "evaluate an arithmetic expression," recognize that parsing and evaluation are separate steps. Parsing builds the tree (Chapter 7). Evaluation walks the tree (this problem). Conflating the two leads to messy code.

### Interview Tips

- If given a string expression instead of a tree, say "I would first parse this into an AST using precedence climbing or the shunting-yard algorithm, then evaluate the tree." This shows you understand the separation of concerns.
- Mention short-circuit evaluation for `AND` and `OR` — if the left side of `AND` is false, skip the right side. Our implementation does not short-circuit (it evaluates both sides), but a production system should.
- If asked about performance for millions of rows, mention that databases compile expressions to bytecode or machine code (JIT) instead of interpreting the tree per row. Our tree walker is correct but not fast at scale.

### Variation: Stack-Based Evaluation

An alternative to tree recursion is converting the expression to reverse Polish notation (RPN) and evaluating with a stack. This is how some database engines (SQLite's VDBE) implement expression evaluation:

```rust,ignore
#[derive(Debug)]
enum RpnOp {
    PushLiteral(Value),
    PushColumn(usize),
    BinaryOp(BinaryOp),
}

fn to_rpn(expr: &Expression) -> Vec<RpnOp> {
    let mut ops = Vec::new();
    build_rpn(expr, &mut ops);
    ops
}

fn build_rpn(expr: &Expression, ops: &mut Vec<RpnOp>) {
    match expr {
        Expression::Literal(v) => ops.push(RpnOp::PushLiteral(v.clone())),
        Expression::Column(idx) => ops.push(RpnOp::PushColumn(*idx)),
        Expression::BinaryOp { op, left, right } => {
            build_rpn(left, ops);
            build_rpn(right, ops);
            ops.push(RpnOp::BinaryOp(op.clone()));
        }
    }
}

fn evaluate_rpn(ops: &[RpnOp], row: &[Value]) -> Result<Value, EvalError> {
    let mut stack: Vec<Value> = Vec::new();

    for op in ops {
        match op {
            RpnOp::PushLiteral(v) => stack.push(v.clone()),
            RpnOp::PushColumn(idx) => stack.push(row[*idx].clone()),
            RpnOp::BinaryOp(op) => {
                let right = stack.pop().unwrap();
                let left = stack.pop().unwrap();
                stack.push(apply_op(op, left, right)?);
            }
        }
    }

    Ok(stack.pop().unwrap())
}
```

The RPN approach has two advantages for databases:
1. **No recursion.** The stack depth is bounded by the expression depth, and the evaluation loop has no function call overhead.
2. **Compilable.** The `Vec<RpnOp>` is essentially bytecode. You can compile it once and evaluate it millions of times (once per row) without re-traversing the tree.

TiDB uses this approach: expressions are compiled to RPN bytecode before the executor starts, and the inner loop evaluates bytecode without tree traversal.

---

## Problem 3: Query Plan Builder

### Problem Statement

Given N tables with known sizes and join predicates between them, find the optimal join order that minimizes the estimated total cost. The cost of joining two tables of sizes `A` and `B` is `A * B` (a simplified model of nested loop join cost). You must join all N tables.

### Examples

```
Tables:
  users:    1000 rows
  orders:   5000 rows
  products: 200 rows

Join predicates:
  users JOIN orders ON users.id = orders.user_id
  orders JOIN products ON orders.product_id = products.id

Join order 1: (users JOIN orders) JOIN products
  Cost: 1000*5000 + 5_000_000*200 = 5_000_000 + 1_000_000_000 = 1_005_000_000

Join order 2: (orders JOIN products) JOIN users
  Cost: 5000*200 + 1_000_000*1000 = 1_000_000 + 1_000_000_000 = 1_001_000_000

Join order 3: (users JOIN products) JOIN orders  (cross join, then join orders)
  Cost: 1000*200 + 200_000*5000 = 200_000 + 1_000_000_000 = 1_000_200_000

Optimal: order 3 (assuming result sizes are bounded by the smaller table)
```

Note: In practice, join result size estimation is complex. For this problem, assume the result of joining tables of sizes A and B is `min(A, B) * 10` (a selectivity-based simplification).

### Constraints

- 2 to 10 tables
- Table sizes are positive integers
- The result size of joining A and B is `min(A, B) * 10`
- You must join all tables into a single result
- Return the minimum cost and the join order

### Brute Force

Try every permutation of tables. For each permutation, compute the cost of joining tables left to right.

```rust,ignore
fn optimal_join_order_brute(tables: &[(&str, u64)]) -> (u64, Vec<String>) {
    let n = tables.len();
    let indices: Vec<usize> = (0..n).collect();
    let mut best_cost = u64::MAX;
    let mut best_order = Vec::new();

    // Generate all permutations
    for perm in permutations(&indices) {
        let mut cost = 0u64;
        let mut current_size = tables[perm[0]].1;

        for i in 1..n {
            let next_size = tables[perm[i]].1;
            cost += current_size * next_size;
            current_size = current_size.min(next_size) * 10; // result size estimate
        }

        if cost < best_cost {
            best_cost = cost;
            best_order = perm.iter().map(|&i| tables[i].0.to_string()).collect();
        }
    }

    (best_cost, best_order)
}

fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut result = Vec::new();
    for (i, &item) in items.iter().enumerate() {
        let rest: Vec<usize> = items.iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &v)| v)
            .collect();
        for mut perm in permutations(&rest) {
            perm.insert(0, item);
            result.push(perm);
        }
    }
    result
}
```

**Time:** O(n! * n) — n! permutations, each costing O(n) to evaluate.
**Space:** O(n!) for storing permutations.

For 10 tables, that is 3,628,800 permutations. Slow but feasible.

### Optimized Solution

Use dynamic programming over subsets (the "DP on bitmask" technique). For each subset of tables, compute the minimum cost of joining all tables in that subset. Build up from pairs to the full set.

```rust
use std::collections::HashMap;

fn optimal_join_order(tables: &[(&str, u64)]) -> (u64, Vec<String>) {
    let n = tables.len();

    // dp[mask] = (min_cost, result_size, last_joined_index)
    // mask is a bitmask representing which tables have been joined
    let mut dp: HashMap<u32, (u64, u64, Vec<usize>)> = HashMap::new();

    // Base case: each single table has cost 0
    for i in 0..n {
        let mask = 1u32 << i;
        dp.insert(mask, (0, tables[i].1, vec![i]));
    }

    // Fill subsets of increasing size
    for size in 2..=n {
        // Enumerate all subsets of the given size
        for mask in 0..(1u32 << n) {
            if mask.count_ones() as usize != size {
                continue;
            }

            let mut best_cost = u64::MAX;
            let mut best_size = 0u64;
            let mut best_order = Vec::new();

            // Try every way to split this subset into two non-empty parts:
            // a subset S and a single table t not in S
            for t in 0..n {
                let t_bit = 1u32 << t;
                if mask & t_bit == 0 {
                    continue; // t is not in this subset
                }

                let rest = mask ^ t_bit;
                if rest == 0 {
                    continue;
                }

                if let Some(&(rest_cost, rest_size, ref rest_order)) = dp.get(&rest) {
                    let join_cost = rest_cost + rest_size * tables[t].1;
                    let join_size = rest_size.min(tables[t].1) * 10;

                    if join_cost < best_cost {
                        best_cost = join_cost;
                        best_size = join_size;
                        let mut order = rest_order.clone();
                        order.push(t);
                        best_order = order;
                    }
                }
            }

            if best_cost < u64::MAX {
                dp.insert(mask, (best_cost, best_size, best_order));
            }
        }
    }

    let full_mask = (1u32 << n) - 1;
    let (cost, _, order) = dp.get(&full_mask).unwrap();
    let names: Vec<String> = order.iter().map(|&i| tables[i].0.to_string()).collect();
    (*cost, names)
}

fn main() {
    let tables = vec![
        ("users", 1000u64),
        ("orders", 5000),
        ("products", 200),
    ];

    let (cost, order) = optimal_join_order(&tables);
    println!("Optimal cost: {}", cost);
    println!("Join order: {:?}", order);
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| Brute force | O(n! * n) | O(n!) |
| DP on bitmask | O(2^n * n) | O(2^n) |

For 10 tables: brute force does ~36 million operations, DP does ~10,000. The DP approach is exponential but dramatically better than factorial. In practice, real query optimizers use heuristics (greedy join ordering) for more than ~10 tables, because even O(2^n) becomes expensive.

### Connection to Our Database

Chapter 9 (Query Optimizer) faced exactly this problem. When a SQL query joins multiple tables, the optimizer must decide the join order. Our optimizer used a simpler greedy approach — join the smallest tables first — because we rarely join more than 3-4 tables. But production databases like PostgreSQL use DP-based join ordering for up to ~12 tables, then fall back to a genetic algorithm (GEQO) for larger join counts.

The bitmask DP technique is the same one used in PostgreSQL's `join_search_one_level()` function. Understanding it helps you reason about why certain queries are slow to plan (the optimizer is exploring 2^n subsets) and why `SET join_collapse_limit = 1` can speed up planning at the cost of plan quality.

### Interview Tips

- Recognize that this is a variant of the Traveling Salesman Problem (TSP). TSP finds the minimum-cost Hamiltonian path; join ordering finds the minimum-cost tree.
- Start with the brute force permutation approach, then introduce the bitmask DP as the optimization. This shows progression.
- If the interviewer asks about more than 12 tables, mention that production systems use heuristics: greedy (join smallest first), simulated annealing, or genetic algorithms.
- The key insight interviewers want: "the number of possible join orders is factorial, but DP reduces it to exponential by reusing subproblem solutions."

### Variation: Greedy Join Ordering

In practice, most query optimizers use a greedy heuristic for join ordering when the number of tables exceeds the DP threshold (typically 10-12 tables). The greedy approach: always join the two smallest available tables next.

```rust,ignore
fn greedy_join_order(tables: &mut Vec<(&str, u64)>) -> (u64, Vec<String>) {
    let mut total_cost = 0u64;
    let mut order = Vec::new();

    while tables.len() > 1 {
        // Sort by size — smallest tables first
        tables.sort_by_key(|&(_, size)| size);

        // Join the two smallest
        let (name_a, size_a) = tables.remove(0);
        let (name_b, size_b) = tables.remove(0);

        let join_cost = size_a * size_b;
        let result_size = size_a.min(size_b) * 10;

        total_cost += join_cost;
        order.push(format!("{} JOIN {}", name_a, name_b));

        // Add the result as a new "table"
        let result_name = format!("({} JOIN {})", name_a, name_b);
        tables.push((Box::leak(result_name.into_boxed_str()), result_size));
    }

    (total_cost, order)
}
```

The greedy approach is O(n^2 log n) — far better than factorial or even exponential. It does not always find the optimal plan, but it finds a good plan quickly. PostgreSQL's GEQO (Genetic Query Optimizer) takes this further with genetic algorithms for 12+ table joins.

For interviews, knowing both the DP optimal and the greedy heuristic shows you understand the full solution space — not just the textbook answer.

---

## Problem 4: Transaction Scheduler

### Problem Statement

You have a list of transactions, each consisting of read and write operations on database keys. Two transactions conflict if one writes a key that the other reads or writes. Given a set of committed transactions, determine a serial execution order that is consistent with the observed behavior — or report that no such order exists (indicating a serializability violation).

This is the **serializability check**: given a set of concurrent transactions, can we find an equivalent serial schedule?

### Examples

```
Transaction T1: read(A), write(B)
Transaction T2: read(B), write(C)
Transaction T3: read(C), write(A)

Dependency graph:
  T1 → T2  (T1 writes B, T2 reads B)
  T2 → T3  (T2 writes C, T3 reads C)
  T3 → T1  (T3 writes A, T1 reads A)

This is a cycle: T1 → T2 → T3 → T1
No serial order exists — serializability violation!
```

```
Transaction T1: read(A), write(B)
Transaction T2: read(B), write(C)
Transaction T3: read(A), write(D)

Dependency graph:
  T1 → T2  (T1 writes B, T2 reads B)
  (No edge between T2 and T3 — they share no keys)
  (T1 and T3 both read A — no conflict for read-read)

Topological order: [T1, T3, T2] or [T1, T2, T3] or [T3, T1, T2]
Serializable!
```

### Constraints

- Up to 1000 transactions
- Each transaction has up to 100 read/write operations
- Keys are strings
- Output a valid serial order if one exists, or report a cycle

### Brute Force

Try every permutation of transactions and check if it is consistent with the dependency graph.

```rust,ignore
fn find_serial_order_brute(transactions: &[Transaction]) -> Option<Vec<usize>> {
    let graph = build_dependency_graph(transactions);
    let n = transactions.len();
    let indices: Vec<usize> = (0..n).collect();

    for perm in permutations(&indices) {
        if is_consistent(&perm, &graph) {
            return Some(perm);
        }
    }
    None
}

fn is_consistent(order: &[usize], graph: &[(usize, usize)]) -> bool {
    let position: HashMap<usize, usize> = order.iter()
        .enumerate()
        .map(|(pos, &txn)| (txn, pos))
        .collect();

    // Every edge (a -> b) means a must come before b
    graph.iter().all(|&(a, b)| position[&a] < position[&b])
}
```

**Time:** O(n! * E) where E is the number of edges.
**Space:** O(n!).

### Optimized Solution

Build a directed graph of transaction dependencies, then perform topological sort. If the topological sort succeeds, the result is a valid serial order. If it fails (cycle detected), no serial order exists.

```rust
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug)]
struct Transaction {
    id: usize,
    reads: Vec<String>,
    writes: Vec<String>,
}

fn build_dependency_graph(transactions: &[Transaction]) -> Vec<Vec<usize>> {
    let n = transactions.len();
    let mut graph = vec![Vec::new(); n];

    // For each pair of transactions, check for conflicts
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            let ti = &transactions[i];
            let tj = &transactions[j];

            // Check write-read conflict: Ti writes X, Tj reads X → Ti → Tj
            let ti_writes: HashSet<&str> = ti.writes.iter().map(|s| s.as_str()).collect();
            let tj_reads: HashSet<&str> = tj.reads.iter().map(|s| s.as_str()).collect();
            let tj_writes: HashSet<&str> = tj.writes.iter().map(|s| s.as_str()).collect();

            // Ti writes something Tj reads
            if ti_writes.intersection(&tj_reads).next().is_some() {
                graph[i].push(j);
                continue; // One edge per pair is enough
            }

            // Ti reads something Tj writes (read-write conflict)
            let ti_reads: HashSet<&str> = ti.reads.iter().map(|s| s.as_str()).collect();
            if ti_reads.intersection(&tj_writes).next().is_some() {
                graph[i].push(j);
                continue;
            }

            // Ti writes something Tj writes (write-write conflict)
            if ti_writes.intersection(&tj_writes).next().is_some() {
                graph[i].push(j);
            }
        }
    }

    graph
}

fn topological_sort(graph: &[Vec<usize>]) -> Option<Vec<usize>> {
    let n = graph.len();
    let mut in_degree = vec![0usize; n];

    for edges in graph {
        for &dest in edges {
            in_degree[dest] += 1;
        }
    }

    let mut queue: VecDeque<usize> = VecDeque::new();
    for i in 0..n {
        if in_degree[i] == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(n);

    while let Some(node) = queue.pop_front() {
        order.push(node);

        for &neighbor in &graph[node] {
            in_degree[neighbor] -= 1;
            if in_degree[neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if order.len() == n {
        Some(order) // All nodes processed — no cycle
    } else {
        None // Cycle detected — some nodes never reached in-degree 0
    }
}

fn schedule_transactions(transactions: &[Transaction]) -> Option<Vec<usize>> {
    let graph = build_dependency_graph(transactions);
    topological_sort(&graph)
}

fn main() {
    // Example with a valid serial order
    let transactions = vec![
        Transaction {
            id: 0,
            reads: vec!["A".into()],
            writes: vec!["B".into()],
        },
        Transaction {
            id: 1,
            reads: vec!["B".into()],
            writes: vec!["C".into()],
        },
        Transaction {
            id: 2,
            reads: vec!["A".into()],
            writes: vec!["D".into()],
        },
    ];

    match schedule_transactions(&transactions) {
        Some(order) => {
            println!("Serial order: {:?}", order);
            // Possible output: [0, 2, 1] or [0, 1, 2]
        }
        None => println!("Not serializable — cycle detected!"),
    }

    // Example with a cycle (not serializable)
    let cyclic = vec![
        Transaction {
            id: 0,
            reads: vec!["A".into()],
            writes: vec!["B".into()],
        },
        Transaction {
            id: 1,
            reads: vec!["B".into()],
            writes: vec!["C".into()],
        },
        Transaction {
            id: 2,
            reads: vec!["C".into()],
            writes: vec!["A".into()],
        },
    ];

    match schedule_transactions(&cyclic) {
        Some(order) => println!("Serial order: {:?}", order),
        None => println!("Not serializable — cycle detected!"),
        // Output: Not serializable — cycle detected!
    }
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| Brute force | O(n! * E) | O(n!) |
| Topological sort | O(n^2 * K + V + E) | O(V + E) |

Where `n` is the number of transactions, `K` is the average number of read/write operations per transaction, `V = n` is the number of vertices, and `E` is the number of edges (up to O(n^2)). Building the dependency graph is O(n^2 * K). The topological sort itself is O(V + E).

### Connection to Our Database

Chapter 5 (MVCC) implemented snapshot isolation, which avoids the need for serializability checking in most cases — each transaction sees a consistent snapshot and write-write conflicts are detected eagerly. But snapshot isolation is weaker than serializability: it allows write skew anomalies.

A database that wants full serializability (like PostgreSQL's `SERIALIZABLE` isolation level) must either:
1. Build and check the transaction dependency graph (Serializable Snapshot Isolation, or SSI), or
2. Use strict two-phase locking (2PL), which prevents cycles by construction

Our topological sort approach is the heart of SSI: build the dependency graph, check for cycles, abort one transaction in any cycle. PostgreSQL's SSI implementation uses a variant of this algorithm.

### Interview Tips

- Recognize that transaction scheduling reduces to topological sort on a dependency graph. State this clearly: "I model transactions as nodes and conflicts as directed edges, then topological sort gives a serial order."
- Know the three types of conflicts: write-read (WR), read-write (RW), write-write (WW). Read-read is not a conflict.
- If asked how to handle cycles in a real database, explain that you abort the youngest transaction in the cycle (the one with the highest transaction ID) and retry it.
- Mention that Kahn's algorithm (BFS-based topological sort) naturally detects cycles — if the queue empties before all nodes are processed, a cycle exists.

### Variation: Parallel Execution

An extension of the scheduling problem: given a valid serial order, identify which transactions can be executed in parallel. Two transactions can execute in parallel if they have no edge between them in the dependency graph.

This reduces to finding the **width** of the dependency DAG — the maximum number of independent transactions at any point. The width determines the maximum parallelism:

```rust,ignore
fn parallel_groups(order: &[usize], graph: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = graph.len();
    let mut in_degree = vec![0usize; n];
    for edges in graph {
        for &dest in edges {
            in_degree[dest] += 1;
        }
    }

    let mut groups = Vec::new();
    let mut remaining: Vec<bool> = vec![true; n];

    loop {
        // Find all nodes with in-degree 0 among remaining nodes
        let group: Vec<usize> = (0..n)
            .filter(|&i| remaining[i] && in_degree[i] == 0)
            .collect();

        if group.is_empty() {
            break;
        }

        // Remove these nodes and update in-degrees
        for &node in &group {
            remaining[node] = false;
            for &neighbor in &graph[node] {
                in_degree[neighbor] -= 1;
            }
        }

        groups.push(group);
    }

    groups
}
```

Each group can execute in parallel. This is how some database engines schedule independent transactions — they identify non-conflicting groups and execute them concurrently on different cores.

---

## Problem 5: Raft Log Compaction

### Problem Statement

A Raft cluster maintains a replicated log of commands. Over time, the log grows unboundedly. Log compaction replaces a prefix of the log with a snapshot: a compact representation of the state machine's state at a given index.

Given a log of entries and a snapshot interval `S`, implement a compaction function that:
1. Takes a snapshot of the state (the accumulated effect of all entries up to the compaction point)
2. Discards all log entries before the compaction point
3. Preserves all entries after the compaction point
4. Maintains the invariant that `last_applied <= commit_index <= last_log_index`

### Examples

```
Log: [("SET A 1", idx=1), ("SET B 2", idx=2), ("SET A 3", idx=3),
      ("DEL B", idx=4), ("SET C 5", idx=5)]
Snapshot interval: 3

After compaction at index 3:
  Snapshot: {A: 3, B: 2}  (state at index 3)
  Log: [("DEL B", idx=4), ("SET C 5", idx=5)]
  snapshot_index: 3
```

### Constraints

- Log entries are `(command, index)` pairs
- Commands are SET, DEL operations on string keys with integer values
- Snapshot interval S is a positive integer
- Compaction preserves all entries after the snapshot point
- The function should handle multiple rounds of compaction

### Brute Force

Replay all log entries from the beginning to compute the state at each potential snapshot point.

```rust,ignore
fn compact_brute(
    log: &[(String, u64)],
    snapshot_interval: u64,
) -> (HashMap<String, i64>, Vec<(String, u64)>) {
    let compact_at = (log.last().map(|(_, i)| *i).unwrap_or(0) / snapshot_interval)
        * snapshot_interval;

    if compact_at == 0 {
        return (HashMap::new(), log.to_vec());
    }

    // Replay from the beginning to build snapshot
    let mut state = HashMap::new();
    for (cmd, idx) in log {
        if *idx > compact_at {
            break;
        }
        apply_command(&mut state, cmd);
    }

    // Keep entries after snapshot
    let remaining: Vec<(String, u64)> = log.iter()
        .filter(|(_, idx)| *idx > compact_at)
        .cloned()
        .collect();

    (state, remaining)
}

fn apply_command(state: &mut HashMap<String, i64>, cmd: &str) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    match parts[0] {
        "SET" => { state.insert(parts[1].to_string(), parts[2].parse().unwrap()); }
        "DEL" => { state.remove(parts[1]); }
        _ => {}
    }
}
```

**Time:** O(n) per compaction where n is the total log length from the beginning.
**Space:** O(n) for the state.

The problem: each compaction replays from index 0. After K compactions, the total work is O(K * n).

### Optimized Solution

Maintain a running state machine that applies entries incrementally. When it is time to compact, the current state *is* the snapshot — no replay needed. Use a sliding window to track which entries have been compacted.

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct RaftLog {
    /// The log entries after the last snapshot
    entries: Vec<(String, u64)>,
    /// The accumulated state machine state
    state: HashMap<String, i64>,
    /// The index of the last compacted entry
    snapshot_index: u64,
    /// How many entries to accumulate before compacting
    snapshot_interval: u64,
    /// The last entry applied to the state machine
    last_applied: u64,
}

impl RaftLog {
    fn new(snapshot_interval: u64) -> Self {
        RaftLog {
            entries: Vec::new(),
            state: HashMap::new(),
            snapshot_index: 0,
            snapshot_interval,
            last_applied: 0,
        }
    }

    /// Append a new entry and apply it to the state machine
    fn append(&mut self, command: String) {
        let index = self.snapshot_index + self.entries.len() as u64 + 1;
        self.entries.push((command.clone(), index));
        self.apply_command(&command);
        self.last_applied = index;
    }

    fn apply_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        match parts.first() {
            Some(&"SET") if parts.len() == 3 => {
                if let Ok(val) = parts[2].parse::<i64>() {
                    self.state.insert(parts[1].to_string(), val);
                }
            }
            Some(&"DEL") if parts.len() == 2 => {
                self.state.remove(parts[1]);
            }
            _ => {}
        }
    }

    /// Check if compaction should occur, and compact if so
    fn maybe_compact(&mut self) -> Option<HashMap<String, i64>> {
        if self.entries.len() as u64 >= self.snapshot_interval {
            let compact_at = self.snapshot_index + self.snapshot_interval;

            // The state already reflects all entries up to last_applied,
            // so the snapshot is just a clone of the current state
            // BUT we need the state at compact_at specifically.
            // Since we apply entries eagerly, we need to track this carefully.

            // Remove entries up to the compaction point
            let entries_to_remove = (compact_at - self.snapshot_index) as usize;
            self.entries.drain(..entries_to_remove);
            self.snapshot_index = compact_at;

            // Return a copy of the snapshot (for persistence)
            Some(self.state.clone())
        } else {
            None
        }
    }

    /// Get the current snapshot state
    fn snapshot(&self) -> &HashMap<String, i64> {
        &self.state
    }

    /// Number of entries currently in the log (after last snapshot)
    fn log_length(&self) -> usize {
        self.entries.len()
    }
}

fn main() {
    let mut log = RaftLog::new(3);

    // Append 5 entries
    log.append("SET A 1".into());
    log.append("SET B 2".into());
    log.append("SET A 3".into());
    println!("Before compaction: {} entries", log.log_length()); // 3

    // Trigger compaction
    if let Some(snapshot) = log.maybe_compact() {
        println!("Snapshot at index {}: {:?}", log.snapshot_index, snapshot);
        // {A: 3, B: 2}
    }
    println!("After compaction: {} entries", log.log_length()); // 0

    // More entries
    log.append("DEL B".into());
    log.append("SET C 5".into());
    println!("Current state: {:?}", log.snapshot());
    // {A: 3, C: 5}
    println!("Log entries: {} (snapshot_index={})", log.log_length(), log.snapshot_index);
    // 2 entries, snapshot_index=3
}
```

### Complexity Analysis

| Approach | Time per compaction | Total time for K compactions | Space |
|----------|-------------------|------------------------------|-------|
| Brute force (replay) | O(n) | O(K * n) | O(n) |
| Incremental (sliding window) | O(S) | O(n) total | O(S + M) |

Where `n` is the total number of entries ever appended, `S` is the snapshot interval, and `M` is the number of unique keys in the state. The incremental approach is amortized O(1) per entry — each entry is applied once and removed once.

### Connection to Our Database

Chapter 16 (Raft Durability & Recovery) dealt with exactly this problem. A Raft log that grows forever consumes unbounded disk space and makes recovery slow (you must replay the entire log). Compaction bounds the log size and speeds up recovery — a new node can load the snapshot and then replay only the entries after the snapshot.

The sliding window pattern appears whenever you need to maintain a summary of the last N items while discarding older ones. In Raft, the "window" is the uncompacted log; the "summary" is the snapshot. The same pattern appears in TCP sliding windows (acknowledgments discard sent segments), time-series databases (roll up old data points), and write-ahead logs (checkpoints allow truncation).

### Interview Tips

- Emphasize the amortized analysis: each entry is processed exactly twice (once when appended, once when compacted), so the total work is O(n) regardless of how many compactions occur.
- Mention the trade-off: frequent compaction reduces log size but increases snapshot I/O. Infrequent compaction reduces I/O but uses more memory and disk for the log.
- If asked about concurrent compaction, explain that Raft takes a snapshot asynchronously — the state machine continues applying entries while the snapshot is being written to disk. This requires copy-on-write semantics or a consistent read snapshot (exactly what MVCC provides).

---

## Problem 6: Index Scan Optimizer

### Problem Statement

A query engine must choose between two scan strategies for a `WHERE` clause:

1. **Full table scan:** Read every row and check the predicate. Cost = number of rows in the table.
2. **Index scan:** Use a B-tree index to find matching rows directly. Cost = number of matching rows * log(n) (for B-tree lookups) + a seek cost.

Given a table size, an estimated selectivity (fraction of rows matching the predicate), and whether an index exists, determine which scan strategy is cheaper.

Extend this to multiple predicates combined with AND: choose the best index (if any) for each predicate and decide the overall strategy.

### Examples

```
Table: users (100,000 rows)
Predicate: age > 25 (selectivity = 0.6)
Index on age: yes

Full scan cost: 100,000
Index scan cost: 100,000 * 0.6 * log2(100,000) + 1000 (seek overhead)
              = 60,000 * 17 + 1000 = 1,021,000

Decision: Full scan (cost 100,000) is cheaper!
```

```
Table: users (100,000 rows)
Predicate: id = 42 (selectivity = 0.00001)
Index on id: yes

Full scan cost: 100,000
Index scan cost: 100,000 * 0.00001 * log2(100,000) + 1000
              = 1 * 17 + 1000 = 1,017

Decision: Index scan (cost 1,017) is much cheaper!
```

### Constraints

- Table sizes range from 100 to 10,000,000
- Selectivity is a float between 0.0 and 1.0
- An index may or may not exist for each predicate
- Multiple predicates are combined with AND
- Seek overhead is a constant (e.g., 1000)
- Return the chosen strategy and its estimated cost

### Brute Force

For each predicate, compute both costs and pick the minimum. For multiple AND predicates, try every combination of strategies.

```rust,ignore
fn choose_scan_brute(
    table_size: u64,
    predicates: &[(f64, bool)], // (selectivity, has_index)
) -> (String, f64) {
    let n = predicates.len();
    let mut best_cost = f64::MAX;
    let mut best_strategy = String::new();

    // Try every combination: 2^n possibilities
    for mask in 0..(1u64 << n) {
        let mut cost = table_size as f64; // start with full scan
        let mut strategy_parts = Vec::new();

        for i in 0..n {
            if mask & (1 << i) != 0 && predicates[i].1 {
                // Use index for predicate i
                let idx_cost = (table_size as f64) * predicates[i].0
                    * (table_size as f64).log2() + 1000.0;
                cost = cost.min(idx_cost);
                strategy_parts.push(format!("index[{}]", i));
            } else {
                strategy_parts.push(format!("scan[{}]", i));
            }
        }

        if cost < best_cost {
            best_cost = cost;
            best_strategy = strategy_parts.join(" + ");
        }
    }

    (best_strategy, best_cost)
}
```

**Time:** O(2^n) for n predicates.
**Space:** O(n).

### Optimized Solution

Use a greedy approach: for each predicate independently, compute the cost of using an index versus filtering during a scan. The cheapest initial access method determines the scan strategy. Remaining predicates are applied as filters on the scan output.

```rust
fn choose_scan_strategy(
    table_size: u64,
    predicates: &[(f64, bool)], // (selectivity, has_index)
    seek_overhead: f64,
) -> ScanDecision {
    let full_scan_cost = table_size as f64;

    // For each predicate with an index, compute the index scan cost
    let mut best_index: Option<(usize, f64)> = None;

    for (i, &(selectivity, has_index)) in predicates.iter().enumerate() {
        if !has_index {
            continue;
        }

        let matching_rows = (table_size as f64) * selectivity;
        let btree_depth = (table_size as f64).log2();
        let index_cost = matching_rows * btree_depth + seek_overhead;

        match &best_index {
            None => best_index = Some((i, index_cost)),
            Some((_, best_cost)) if index_cost < *best_cost => {
                best_index = Some((i, index_cost));
            }
            _ => {}
        }
    }

    match best_index {
        Some((idx, index_cost)) if index_cost < full_scan_cost => {
            // Apply remaining predicates as filters on the index scan output
            let mut remaining_selectivity = 1.0;
            for (i, &(sel, _)) in predicates.iter().enumerate() {
                if i != idx {
                    remaining_selectivity *= sel;
                }
            }
            let total_rows_after_filter =
                (table_size as f64) * predicates[idx].0 * remaining_selectivity;

            ScanDecision {
                strategy: Strategy::IndexScan { predicate_index: idx },
                estimated_cost: index_cost,
                estimated_rows: total_rows_after_filter as u64,
            }
        }
        _ => {
            // Full scan with all predicates as filters
            let combined_selectivity: f64 = predicates.iter()
                .map(|(sel, _)| sel)
                .product();
            let estimated_rows = (table_size as f64 * combined_selectivity) as u64;

            ScanDecision {
                strategy: Strategy::FullScan,
                estimated_cost: full_scan_cost,
                estimated_rows,
            }
        }
    }
}

#[derive(Debug)]
enum Strategy {
    FullScan,
    IndexScan { predicate_index: usize },
}

#[derive(Debug)]
struct ScanDecision {
    strategy: Strategy,
    estimated_cost: f64,
    estimated_rows: u64,
}

fn main() {
    // High selectivity (many rows match) — full scan wins
    let decision = choose_scan_strategy(
        100_000,
        &[(0.6, true)], // 60% of rows match, index exists
        1000.0,
    );
    println!("High selectivity: {:?}", decision);
    // FullScan — scanning 100K rows is cheaper than 60K index lookups

    // Low selectivity (few rows match) — index scan wins
    let decision = choose_scan_strategy(
        100_000,
        &[(0.00001, true)], // 1 row matches, index exists
        1000.0,
    );
    println!("Low selectivity: {:?}", decision);
    // IndexScan — 1 lookup + seek overhead beats scanning 100K rows

    // Multiple predicates
    let decision = choose_scan_strategy(
        1_000_000,
        &[
            (0.1, true),   // 10% match, index available
            (0.5, false),  // 50% match, no index
            (0.01, true),  // 1% match, index available
        ],
        1000.0,
    );
    println!("Multiple predicates: {:?}", decision);
    // IndexScan on predicate 2 (1% selectivity) — fewest rows
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| Brute force | O(2^n) | O(n) |
| Greedy | O(n) | O(1) |

Where `n` is the number of predicates. The greedy approach examines each predicate once and picks the best index. This is optimal for the independent-predicate case (AND combinations where index choice does not affect other predicates' costs).

### Connection to Our Database

Chapter 9 (Query Optimizer) made exactly this decision. When the optimizer sees `WHERE id = 42 AND status = 'active'`, it must decide:
- Full table scan with both filters applied to each row
- Index scan on `id` (if an index exists), then filter by `status`
- Index scan on `status` (if an index exists), then filter by `id`

The optimizer picks the strategy with the lowest estimated cost. The selectivity estimates come from table statistics — histograms of value distributions, distinct value counts, and null fractions. Real databases maintain these statistics using `ANALYZE` (PostgreSQL) or `ANALYZE TABLE` (MySQL).

The crossover point — where index scan becomes cheaper than full scan — is typically around 5-15% selectivity. This is a useful heuristic to mention in interviews: "If more than ~10% of rows match, a full scan is usually cheaper because sequential I/O is much faster than random I/O."

### Interview Tips

- The key insight: index scans do random I/O (seek to each matching row), while full scans do sequential I/O. Sequential I/O is 10-100x faster on spinning disks and 2-5x faster on SSDs. This is why full scans win for low-selectivity predicates.
- If asked about covering indexes (indexes that include all needed columns), explain that they eliminate the random I/O entirely — the index contains all the data, so no table lookup is needed.
- Mention that real optimizers use histograms for selectivity estimation, not just a single float. A histogram captures the distribution of values, handling skewed data (e.g., 99% of users are in the US, 1% in other countries).

---

## Problem 7: Deadlock Detector

### Problem Statement

In a database that uses lock-based concurrency control, transactions may wait for each other. Transaction T1 holds lock A and waits for lock B, while T2 holds lock B and waits for lock A. Neither can proceed — this is a deadlock.

Given a set of "waits-for" edges (T_i waits for T_j), detect whether a deadlock exists. If one exists, identify the cycle.

### Examples

```
Waits-for edges:
  T1 → T2  (T1 is waiting for a lock held by T2)
  T2 → T3  (T2 is waiting for a lock held by T3)
  T3 → T1  (T3 is waiting for a lock held by T1)

Cycle detected: T1 → T2 → T3 → T1
Deadlock!
```

```
Waits-for edges:
  T1 → T2
  T2 → T3
  T4 → T3

No cycle — T3 does not wait for anyone.
No deadlock.
```

### Constraints

- Up to 10,000 transactions
- Each transaction waits for at most one other transaction (simple wait-for graph)
- Return the cycle if one exists, or None if no deadlock

### Brute Force

For each node, perform a DFS and check if we revisit a node on the current path.

```rust,ignore
fn detect_deadlock_brute(
    edges: &[(usize, usize)],
    num_transactions: usize,
) -> Option<Vec<usize>> {
    let mut graph = vec![Vec::new(); num_transactions];
    for &(from, to) in edges {
        graph[from].push(to);
    }

    // Try starting DFS from every node
    for start in 0..num_transactions {
        let mut visited = vec![false; num_transactions];
        let mut path = Vec::new();
        if let Some(cycle) = dfs_find_cycle(&graph, start, &mut visited, &mut path) {
            return Some(cycle);
        }
    }
    None
}

fn dfs_find_cycle(
    graph: &[Vec<usize>],
    node: usize,
    visited: &mut [bool],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    if let Some(pos) = path.iter().position(|&n| n == node) {
        // Found a cycle — extract it
        return Some(path[pos..].to_vec());
    }
    if visited[node] {
        return None;
    }
    visited[node] = true;
    path.push(node);

    for &neighbor in &graph[node] {
        if let Some(cycle) = dfs_find_cycle(graph, neighbor, visited, path) {
            return Some(cycle);
        }
    }

    path.pop();
    None
}
```

**Time:** O(V * (V + E)) — DFS from every vertex.
**Space:** O(V) for the visited array and path.

### Optimized Solution

Use a single DFS with three-color marking (white/gray/black). White nodes are unvisited, gray nodes are on the current DFS path, black nodes are fully explored. A back edge (an edge to a gray node) indicates a cycle.

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Color {
    White, // Unvisited
    Gray,  // On the current DFS path
    Black, // Fully explored, no cycle through this node
}

fn detect_deadlock(
    edges: &[(usize, usize)],
    num_transactions: usize,
) -> Option<Vec<usize>> {
    // Build adjacency list
    let mut graph: Vec<Vec<usize>> = vec![Vec::new(); num_transactions];
    for &(from, to) in edges {
        graph[from].push(to);
    }

    let mut color = vec![Color::White; num_transactions];
    let mut parent: HashMap<usize, usize> = HashMap::new();

    for start in 0..num_transactions {
        if color[start] != Color::White {
            continue;
        }
        if let Some(cycle) = dfs_cycle(&graph, start, &mut color, &mut parent) {
            return Some(cycle);
        }
    }
    None
}

fn dfs_cycle(
    graph: &[Vec<usize>],
    node: usize,
    color: &mut [Color],
    parent: &mut HashMap<usize, usize>,
) -> Option<Vec<usize>> {
    color[node] = Color::Gray;

    for &neighbor in &graph[node] {
        if color[neighbor] == Color::Gray {
            // Back edge found — reconstruct the cycle
            let mut cycle = vec![neighbor];
            let mut current = node;
            while current != neighbor {
                cycle.push(current);
                current = *parent.get(&current).unwrap();
            }
            cycle.push(neighbor); // complete the cycle
            cycle.reverse();
            return Some(cycle);
        }

        if color[neighbor] == Color::White {
            parent.insert(neighbor, node);
            if let Some(cycle) = dfs_cycle(graph, neighbor, color, parent) {
                return Some(cycle);
            }
        }
    }

    color[node] = Color::Black;
    None
}

fn main() {
    // Deadlock: T0 → T1 → T2 → T0
    let edges = vec![(0, 1), (1, 2), (2, 0)];
    match detect_deadlock(&edges, 3) {
        Some(cycle) => println!("Deadlock detected: {:?}", cycle),
        None => println!("No deadlock"),
    }
    // Output: Deadlock detected: [0, 1, 2, 0]

    // No deadlock
    let edges = vec![(0, 1), (1, 2), (3, 2)];
    match detect_deadlock(&edges, 4) {
        Some(cycle) => println!("Deadlock detected: {:?}", cycle),
        None => println!("No deadlock"),
    }
    // Output: No deadlock

    // Self-deadlock (transaction waits for itself — degenerate case)
    let edges = vec![(0, 0)];
    match detect_deadlock(&edges, 1) {
        Some(cycle) => println!("Deadlock detected: {:?}", cycle),
        None => println!("No deadlock"),
    }
    // Output: Deadlock detected: [0, 0]
}
```

### Complexity Analysis

| Approach | Time | Space |
|----------|------|-------|
| Brute force (DFS per node) | O(V * (V + E)) | O(V) |
| Three-color DFS | O(V + E) | O(V) |

The three-color approach visits each vertex and edge exactly once. Black nodes are never revisited, so the total work across all DFS calls is O(V + E).

### Connection to Our Database

Our MVCC implementation in Chapter 5 uses optimistic concurrency — transactions do not acquire locks, so deadlocks are impossible by design. But many production databases (MySQL InnoDB, Oracle) use pessimistic locking and must detect deadlocks.

PostgreSQL's deadlock detector runs periodically (every `deadlock_timeout` milliseconds, default 1 second) and builds a waits-for graph from the lock manager. If it finds a cycle, it aborts the newest transaction in the cycle. The three-color DFS is exactly the algorithm PostgreSQL uses internally.

The design choice between MVCC (no deadlocks, possible write conflicts) and locking (possible deadlocks, no conflicts) is a fundamental trade-off. Our database chose MVCC. Understanding both approaches — and the cycle detection algorithm that makes locking viable — rounds out your knowledge.

### Interview Tips

- The three-color terminology is standard in algorithm textbooks (CLRS). Use it: "I mark nodes as white (unvisited), gray (in progress), and black (complete). A back edge to a gray node indicates a cycle."
- If asked how to *resolve* a deadlock (not just detect it), the standard approach is to abort the transaction that would cause the least wasted work — typically the youngest transaction (highest ID) or the one that has done the fewest writes.
- For the follow-up "how would you prevent deadlocks?" mention two approaches: (1) wait-die / wound-wait schemes that use transaction timestamps to break ties, and (2) timeout-based detection that simply aborts transactions that wait too long.

---

## Problem 8: Distributed Counter

### Problem Statement

Design a counter that works across multiple servers without coordination. Each server can increment its local counter. When you query the total count, it should return the correct total across all servers — eventually.

This is a CRDT (Conflict-free Replicated Data Type) problem. Implement a G-Counter (grow-only counter) that:
1. Supports `increment(node_id)` on any server
2. Supports `value()` that returns the total across all servers
3. Supports `merge(other)` to reconcile two replicas after a network partition
4. Never loses increments, even during concurrent operations and network splits

### Examples

```
Server A increments 3 times: A=3
Server B increments 5 times: B=5
Server C increments 2 times: C=2

Each server knows only its own count until merge.

After A merges with B: A sees {A:3, B:5}, total=8
After A merges with C: A sees {A:3, B:5, C:2}, total=10
After B merges with C: B sees {A:0, B:5, C:2}, total=7  (B hasn't seen A's updates yet)
After B merges with A: B sees {A:3, B:5, C:2}, total=10
```

### Constraints

- Up to 100 servers
- Increments are always positive (grow-only)
- Merge must be commutative (A merge B = B merge A), associative, and idempotent (merging twice has no extra effect)
- The value after all merges have propagated must be the true total

### Brute Force

Use a centralized counter with a lock. Every increment sends an RPC to a single "counter server."

```rust,ignore
use std::sync::Mutex;

struct CentralizedCounter {
    value: Mutex<u64>,
}

impl CentralizedCounter {
    fn increment(&self) {
        let mut val = self.value.lock().unwrap();
        *val += 1;
    }

    fn value(&self) -> u64 {
        *self.value.lock().unwrap()
    }
}
```

**Time:** O(1) per operation.
**Space:** O(1).

This is correct but requires network coordination for every increment. If the central server is down, no increments can happen. This is the antithesis of a distributed system.

### Optimized Solution

Use a G-Counter: each server maintains a map from `node_id -> count`. Incrementing only touches the local entry. Merging takes the element-wise maximum of two maps. This is conflict-free because the max operation is commutative, associative, and idempotent.

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct GCounter {
    /// Each entry tracks how many increments this node has observed for that node_id
    counts: HashMap<String, u64>,
    /// This node's identifier
    node_id: String,
}

impl GCounter {
    fn new(node_id: &str) -> Self {
        GCounter {
            counts: HashMap::new(),
            node_id: node_id.to_string(),
        }
    }

    /// Increment the counter on this node
    fn increment(&mut self) {
        let entry = self.counts.entry(self.node_id.clone()).or_insert(0);
        *entry += 1;
    }

    /// Increment by a specific amount
    fn increment_by(&mut self, amount: u64) {
        let entry = self.counts.entry(self.node_id.clone()).or_insert(0);
        *entry += amount;
    }

    /// Get the total value across all nodes
    fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Merge another counter into this one.
    /// Takes the element-wise maximum — this is the CRDT magic.
    fn merge(&mut self, other: &GCounter) {
        for (node_id, &count) in &other.counts {
            let entry = self.counts.entry(node_id.clone()).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

/// A PN-Counter supports both increment and decrement.
/// It uses two G-Counters: one for increments, one for decrements.
#[derive(Debug, Clone)]
struct PNCounter {
    increments: GCounter,
    decrements: GCounter,
}

impl PNCounter {
    fn new(node_id: &str) -> Self {
        PNCounter {
            increments: GCounter::new(node_id),
            decrements: GCounter::new(node_id),
        }
    }

    fn increment(&mut self) {
        self.increments.increment();
    }

    fn decrement(&mut self) {
        self.decrements.increment();
    }

    fn value(&self) -> i64 {
        self.increments.value() as i64 - self.decrements.value() as i64
    }

    fn merge(&mut self, other: &PNCounter) {
        self.increments.merge(&other.increments);
        self.decrements.merge(&other.decrements);
    }
}

fn main() {
    // G-Counter example
    let mut counter_a = GCounter::new("A");
    let mut counter_b = GCounter::new("B");
    let mut counter_c = GCounter::new("C");

    // Each server increments independently
    counter_a.increment_by(3);
    counter_b.increment_by(5);
    counter_c.increment_by(2);

    println!("A sees: {} (only its own)", counter_a.value()); // 3
    println!("B sees: {} (only its own)", counter_b.value()); // 5

    // A merges with B
    counter_a.merge(&counter_b);
    println!("A after merge with B: {}", counter_a.value()); // 8

    // A merges with C
    counter_a.merge(&counter_c);
    println!("A after merge with C: {}", counter_a.value()); // 10

    // B merges with A (gets everything)
    counter_b.merge(&counter_a);
    println!("B after merge with A: {}", counter_b.value()); // 10

    // Idempotent: merging again changes nothing
    counter_b.merge(&counter_a);
    println!("B after second merge: {}", counter_b.value()); // still 10

    println!();

    // PN-Counter example
    let mut pn_a = PNCounter::new("A");
    let mut pn_b = PNCounter::new("B");

    pn_a.increment(); // +1
    pn_a.increment(); // +2
    pn_b.increment(); // +1
    pn_b.decrement(); // 0 on B

    println!("PN A: {}", pn_a.value()); // 2
    println!("PN B: {}", pn_b.value()); // 0

    pn_a.merge(&pn_b);
    println!("PN A after merge: {}", pn_a.value()); // 2 (A:+2, B:+1-1)
}
```

### Complexity Analysis

| Approach | Increment | Value | Merge | Space |
|----------|-----------|-------|-------|-------|
| Centralized | O(1) + network | O(1) + network | N/A | O(1) |
| G-Counter | O(1) | O(N) | O(N) | O(N) |

Where N is the number of nodes. The G-Counter trades space (O(N) per counter per node) for availability — every node can increment locally without any coordination.

### Connection to Our Database

Our database uses Raft for consensus — strong consistency, linearizable reads and writes. CRDTs offer the opposite trade-off: eventual consistency with zero coordination. Understanding both extremes helps you design systems that choose the right consistency model for each piece of data.

In a distributed database like CockroachDB, strongly consistent operations (bank transfers) use Raft. Eventually consistent operations (page view counters, analytics) could use CRDTs. The database we built in Chapters 14-17 is the Raft side of this spectrum. This problem shows the CRDT side.

The merge operation's mathematical properties — commutativity (`A merge B = B merge A`), associativity (`(A merge B) merge C = A merge (B merge C)`), and idempotency (`A merge A = A`) — are what make CRDTs work without coordination. These are the same properties that make Raft's log replication work: committed entries are never changed, and applying the same entry twice is a no-op.

### Interview Tips

- Start by explaining the CAP theorem: you cannot have Consistency, Availability, and Partition tolerance simultaneously. CRDTs choose Availability + Partition tolerance (AP), while Raft chooses Consistency + Partition tolerance (CP).
- The key insight interviewers want: "The merge function must be commutative, associative, and idempotent. Element-wise max satisfies all three properties. Element-wise addition does not (it is not idempotent — merging twice doubles the count)."
- If asked about more complex CRDTs, mention: LWW-Register (last-writer-wins), OR-Set (observed-remove set), and CRDT Maps. Each uses a different conflict resolution strategy.
- Mention that DynamoDB, Riak, and Redis CRDT modules use these exact data structures in production.

---

## Summary

| Problem | Pattern | Key Insight | Complexity |
|---------|---------|-------------|------------|
| KV Range Query | B-Tree Traversal | Skip subtrees outside the range | O(log n + k) |
| Expression Evaluator | Tree Recursion | Evaluate children first, then combine | O(n) |
| Query Plan Builder | Bitmask DP | Reuse subproblem solutions for subset combinations | O(2^n * n) |
| Transaction Scheduler | Topological Sort | Conflicts are directed edges, serial order is a topological sort | O(V + E) |
| Raft Log Compaction | Sliding Window | Maintain running state, discard prefix | Amortized O(1) per entry |
| Index Scan Optimizer | Greedy / Cost-Based | Compare sequential vs random I/O costs | O(n) |
| Deadlock Detector | Three-Color DFS | Back edge to gray node means cycle | O(V + E) |
| Distributed Counter | CRDTs | Element-wise max is commutative, associative, idempotent | O(N) per merge |

Each problem connects directly to a component of the database you built. The B-tree traversal is your storage engine's scan. The expression evaluator is your query executor. The topological sort is serializability checking. The sliding window is Raft log compaction. The cycle detector is deadlock detection. The CRDT is an alternative to Raft's strong consistency model.

These are not just interview problems — they are the algorithms that run inside every database you will ever use.

---

## Additional Practice

Each problem above has natural extensions. Here are follow-up challenges you can attempt on your own:

### Extensions by Problem

**Problem 1 (KV Range Query):**
- Implement a range scan that returns an iterator instead of a Vec. This mirrors the design improvement discussed in Chapter 18.5 (Design Reflection).
- Add a `LIMIT` parameter that stops the scan after N results. How does this change the complexity?
- Implement a reverse range scan (iterate from `end` to `start` in descending order).

**Problem 2 (Expression Evaluator):**
- Add NULL handling. SQL's three-valued logic (true, false, NULL) requires special cases: `NULL AND false = false`, `NULL AND true = NULL`, `NULL OR true = true`, `NULL OR false = NULL`.
- Add aggregate function support: `SUM`, `COUNT`, `AVG`, `MIN`, `MAX`. These accumulate state across multiple rows.
- Implement constant folding: if both operands of a binary operation are literals, compute the result at plan time instead of execution time.

**Problem 3 (Query Plan Builder):**
- Add join predicate selectivity: instead of using `min(A, B) * 10` for result sizes, estimate based on the join predicate (equality join on a primary key produces at most `min(A, B)` rows).
- Implement the greedy heuristic (always join the two smallest available tables) and compare its results to the DP optimal. How often does greedy find the optimal plan?

**Problem 4 (Transaction Scheduler):**
- Extend to handle read-write conflicts at the column level instead of the row level. Two transactions that write different columns of the same row do not conflict.
- Implement SSI (Serializable Snapshot Isolation): detect write skew anomalies by tracking read sets.

**Problem 5 (Raft Log Compaction):**
- Implement InstallSnapshot RPC: when a follower is too far behind to catch up via AppendEntries, send the entire snapshot. This requires serializing the state machine state.
- Add a compaction trigger based on log size (in bytes) rather than entry count.

**Problem 6 (Index Scan Optimizer):**
- Add composite index support: an index on `(a, b)` can satisfy `WHERE a = 1 AND b = 2` but also `WHERE a = 1` (using just the prefix).
- Implement an index-only scan: if the index contains all columns needed by the query, skip the table lookup entirely.

**Problem 7 (Deadlock Detector):**
- Implement the wait-die protocol: older transactions wait for younger ones; younger transactions die (abort) when they would wait for older ones. This prevents deadlocks by construction.
- Extend to waits-for graphs with multiple lock types (shared, exclusive). Two transactions holding shared locks on the same key do not conflict.

**Problem 8 (Distributed Counter):**
- Implement a PN-Counter that supports both increment and decrement (included in the solution above). Verify that it converges correctly.
- Implement an OR-Set (Observed-Remove Set) — a CRDT set where elements can be added and removed without conflicts.
- Implement a LWW-Register (Last-Writer-Wins Register) using vector clocks for causal ordering.

### Cross-Problem Connections

Several problems share underlying structures:

| Shared Pattern | Problems |
|---------------|----------|
| Tree traversal | 1 (BST range scan), 2 (expression tree), 3 (plan tree) |
| Graph algorithms | 4 (topological sort), 7 (cycle detection) |
| Sliding window / amortized | 5 (log compaction), 6 (scan optimization) |
| Conflict resolution | 4 (transaction scheduling), 7 (deadlock detection), 8 (CRDTs) |

Understanding these connections helps you recognize patterns in unfamiliar problems. When you see a problem involving "ordering things with constraints," think topological sort. When you see "detect contradictions in a dependency graph," think cycle detection. When you see "merge data from multiple sources without coordination," think CRDTs.

The database domain naturally produces problems from every major algorithm category — trees, graphs, dynamic programming, greedy algorithms, and distributed data structures. This is why database internals are such fertile ground for interview preparation: they motivate real problems with real trade-offs, not artificial puzzle-box exercises.
