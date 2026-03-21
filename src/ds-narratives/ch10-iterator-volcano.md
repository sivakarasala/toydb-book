# Iterator Pattern: Volcano Model — "Pull one row at a time"

You have a table with 1 million rows. The query says `SELECT name FROM users WHERE age > 30`. The naive approach: load all 1 million rows into a `Vec`, filter it down to the matching rows, then extract the `name` column into another `Vec`. At peak, you are holding the entire table plus the filtered copy in memory. If each row is 200 bytes, that is 200 MB for the full table, plus however much the filtered set takes.

The Volcano model does it differently. Instead of loading everything, it sets up a pipeline: the Scan operator sits at the bottom, the Filter sits above it, and the Project sits at the top. When the executor asks the Project for a row, Project asks Filter, which asks Scan. Scan reads one row from disk, hands it to Filter. Filter checks `age > 30`. If it passes, Filter hands it to Project, which extracts `name` and returns it. If the row fails the filter, Filter asks Scan for the next one. At no point is more than one row in memory.

One row at a time. A million rows flow through the pipeline, but the pipeline only holds one. Let's build it.

---

## The Naive Way

Load everything, filter everything, project everything:

```rust
fn main() {
    // Simulate a table with 1 million rows
    let table: Vec<Vec<String>> = (0..1_000_000)
        .map(|i| vec![
            format!("user_{}", i),           // name
            format!("{}", 18 + (i % 60)),     // age
            format!("user_{}@mail.com", i),   // email
        ])
        .collect();

    println!("Table size: {} rows, ~{} MB in memory",
        table.len(),
        table.len() * 3 * 20 / 1_000_000  // rough estimate
    );

    // Step 1: Filter (creates a NEW vector)
    let filtered: Vec<&Vec<String>> = table.iter()
        .filter(|row| row[1].parse::<i32>().unwrap() > 30)
        .collect();

    println!("After filter: {} rows (still holding original table!)", filtered.len());

    // Step 2: Project (creates ANOTHER vector)
    let names: Vec<&str> = filtered.iter()
        .map(|row| row[0].as_str())
        .collect();

    println!("After project: {} names", names.len());
    println!("Peak memory: full table + filtered refs + projected names");
    // We held 3 data structures simultaneously
}
```

Three data structures in memory at peak: the full table, the filtered references, and the projected names. For large tables, this is wasteful. For tables that do not fit in memory, it is impossible.

---

## The Insight

Picture a factory assembly line. Raw materials enter at one end. The first station shapes them. The second station paints them. The third station inspects them. Each station processes one item at a time: take an item from the upstream station, process it, pass it to the downstream station, then wait for the next one.

No station ever stockpiles. The painting station does not say "bring me all the shaped items." It says "give me the next shaped item." This **pull-based** model means the assembly line works even if you have a million items -- each station only holds one item at a time.

The Volcano model (named after Goetz Graefe's 1994 paper) works exactly like this. Each operator in the query plan implements one method: `next()`. When called, it returns the next row, or `None` if there are no more rows. Operators are composed: a Filter's `next()` calls its input's `next()` repeatedly until it finds a row that passes the predicate. A Project's `next()` calls its input's `next()` once and transforms the columns.

The interface is beautifully simple:

```text
trait Operator {
    fn next(&mut self) -> Option<Row>
}
```

That is it. Three operators (Scan, Filter, Project), one method each, and you can evaluate arbitrary query pipelines with constant memory.

---

## The Build

### The Row Type

A row is a vector of values. We keep it simple:

```rust
#[derive(Debug, Clone)]
enum Value {
    Integer(i64),
    Str(String),
    Boolean(bool),
    Null,
}

type Row = Vec<Value>;
```

### The Schema

Each operator needs to know column names so filters and projections can reference them:

```rust
#[derive(Debug, Clone)]
struct Schema {
    columns: Vec<String>,
}

impl Schema {
    fn new(columns: Vec<&str>) -> Self {
        Schema {
            columns: columns.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }
}
```

### The Operator Trait

Every operator implements `next()` and `schema()`:

```rust
trait Operator {
    fn next(&mut self) -> Option<Row>;
    fn schema(&self) -> &Schema;
}
```

### Scan: Read Rows from a Table

The Scan operator iterates over in-memory data, one row at a time:

```rust
struct Scan {
    data: Vec<Row>,
    pos: usize,
    schema: Schema,
}

impl Scan {
    fn new(schema: Schema, data: Vec<Row>) -> Self {
        Scan { data, pos: 0, schema }
    }
}

impl Operator for Scan {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.data.len() {
            let row = self.data[self.pos].clone();
            self.pos += 1;
            Some(row)
        } else {
            None
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}
```

In a real database, Scan would read from disk pages, not a `Vec`. But the interface is the same: `next()` returns one row at a time.

### Filter: Check a Predicate

The Filter operator asks its input for rows and only passes through those that match a predicate:

```rust
struct Filter {
    input: Box<dyn Operator>,
    predicate: Box<dyn Fn(&Row, &Schema) -> bool>,
    schema: Schema,
}

impl Filter {
    fn new(
        input: Box<dyn Operator>,
        predicate: Box<dyn Fn(&Row, &Schema) -> bool>,
    ) -> Self {
        let schema = input.schema().clone();
        Filter { input, predicate, schema }
    }
}

impl Operator for Filter {
    fn next(&mut self) -> Option<Row> {
        loop {
            let row = self.input.next()?;
            if (self.predicate)(&row, &self.schema) {
                return Some(row);
            }
            // Row didn't match -- ask for the next one.
            // This is where the magic happens: rejected rows
            // are dropped immediately, never stored.
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}
```

Notice the `loop`. The Filter might need to skip many rows before finding one that matches. Each skipped row is immediately dropped -- it never accumulates in memory. If 99% of rows fail the filter, 99% of data flows through and is discarded one row at a time.

### Project: Select Columns

The Project operator keeps only the specified columns from each row:

```rust
struct Project {
    input: Box<dyn Operator>,
    column_indices: Vec<usize>,
    schema: Schema,
}

impl Project {
    fn new(input: Box<dyn Operator>, columns: Vec<&str>) -> Self {
        let input_schema = input.schema().clone();
        let column_indices: Vec<usize> = columns.iter()
            .map(|name| input_schema.index_of(name)
                .unwrap_or_else(|| panic!("column '{}' not found", name)))
            .collect();

        let schema = Schema {
            columns: columns.iter().map(|s| s.to_string()).collect(),
        };

        Project { input, column_indices, schema }
    }
}

impl Operator for Project {
    fn next(&mut self) -> Option<Row> {
        let row = self.input.next()?;
        let projected: Row = self.column_indices.iter()
            .map(|&idx| row[idx].clone())
            .collect();
        Some(projected)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}
```

### Limit: Stop After N Rows

The Limit operator returns at most N rows, then stops:

```rust
struct Limit {
    input: Box<dyn Operator>,
    remaining: usize,
    schema: Schema,
}

impl Limit {
    fn new(input: Box<dyn Operator>, count: usize) -> Self {
        let schema = input.schema().clone();
        Limit { input, remaining: count, schema }
    }
}

impl Operator for Limit {
    fn next(&mut self) -> Option<Row> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        self.input.next()
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}
```

This is where the pull model really shines. A `LIMIT 10` query over a million-row table never touches more than 10 rows (plus however many the filter rejects). The Scan operator is never asked for row 11. In the push model (materialize everything), you would scan all million rows before picking the first 10.

---

## The Payoff

Here is the full, runnable implementation:

```rust
#[derive(Debug, Clone)]
enum Value { Integer(i64), Str(String), Boolean(bool), Null }

type Row = Vec<Value>;

#[derive(Debug, Clone)]
struct Schema { columns: Vec<String> }
impl Schema {
    fn new(cols: Vec<&str>) -> Self {
        Schema { columns: cols.iter().map(|s| s.to_string()).collect() }
    }
    fn index_of(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }
}

trait Operator {
    fn next(&mut self) -> Option<Row>;
    fn schema(&self) -> &Schema;
}

struct Scan { data: Vec<Row>, pos: usize, schema: Schema }
impl Scan {
    fn new(schema: Schema, data: Vec<Row>) -> Self { Scan { data, pos: 0, schema } }
}
impl Operator for Scan {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.data.len() {
            let r = self.data[self.pos].clone(); self.pos += 1; Some(r)
        } else { None }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct Filter {
    input: Box<dyn Operator>,
    predicate: Box<dyn Fn(&Row, &Schema) -> bool>,
    schema: Schema,
}
impl Filter {
    fn new(input: Box<dyn Operator>, pred: Box<dyn Fn(&Row, &Schema) -> bool>) -> Self {
        let s = input.schema().clone();
        Filter { input, predicate: pred, schema: s }
    }
}
impl Operator for Filter {
    fn next(&mut self) -> Option<Row> {
        loop {
            let row = self.input.next()?;
            if (self.predicate)(&row, &self.schema) { return Some(row); }
        }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct Project { input: Box<dyn Operator>, indices: Vec<usize>, schema: Schema }
impl Project {
    fn new(input: Box<dyn Operator>, cols: Vec<&str>) -> Self {
        let is = input.schema().clone();
        let indices: Vec<usize> = cols.iter()
            .map(|n| is.index_of(n).unwrap()).collect();
        let schema = Schema { columns: cols.iter().map(|s| s.to_string()).collect() };
        Project { input, indices, schema }
    }
}
impl Operator for Project {
    fn next(&mut self) -> Option<Row> {
        let row = self.input.next()?;
        Some(self.indices.iter().map(|&i| row[i].clone()).collect())
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct Limit { input: Box<dyn Operator>, remaining: usize, schema: Schema }
impl Limit {
    fn new(input: Box<dyn Operator>, count: usize) -> Self {
        let s = input.schema().clone();
        Limit { input, remaining: count, schema: s }
    }
}
impl Operator for Limit {
    fn next(&mut self) -> Option<Row> {
        if self.remaining == 0 { return None; }
        self.remaining -= 1;
        self.input.next()
    }
    fn schema(&self) -> &Schema { &self.schema }
}

fn format_value(v: &Value) -> String {
    match v {
        Value::Integer(n) => n.to_string(),
        Value::Str(s) => s.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "NULL".to_string(),
    }
}

fn main() {
    // Build a table: users(id, name, age, active)
    let schema = Schema::new(vec!["id", "name", "age", "active"]);
    let data: Vec<Row> = (0..1_000_000)
        .map(|i| vec![
            Value::Integer(i),
            Value::Str(format!("user_{}", i)),
            Value::Integer(18 + (i % 60)),
            Value::Boolean(i % 3 != 0),
        ])
        .collect();

    println!("Table: {} rows\n", data.len());

    // Query: SELECT name FROM users WHERE age > 50 AND active = true LIMIT 10
    let scan = Scan::new(schema, data);

    let filter = Filter::new(
        Box::new(scan),
        Box::new(|row: &Row, schema: &Schema| {
            let age_idx = schema.index_of("age").unwrap();
            let active_idx = schema.index_of("active").unwrap();
            let age_ok = match &row[age_idx] {
                Value::Integer(n) => *n > 50,
                _ => false,
            };
            let active_ok = match &row[active_idx] {
                Value::Boolean(b) => *b,
                _ => false,
            };
            age_ok && active_ok
        }),
    );

    let project = Project::new(Box::new(filter), vec!["name", "age"]);
    let mut limit = Limit::new(Box::new(project), 10);

    // Execute by pulling rows
    println!("Results (SELECT name, age FROM users WHERE age > 50 AND active LIMIT 10):");
    let mut count = 0;
    while let Some(row) = limit.next() {
        let cols: Vec<String> = row.iter().map(format_value).collect();
        println!("  {}", cols.join(" | "));
        count += 1;
    }
    println!("\nReturned {} rows from 1,000,000 row table", count);
    println!("Memory: only 1 row in pipeline at any time");

    // Demonstrate the pipeline structure
    println!("\n=== Pipeline Structure ===");
    println!("  Limit(10)");
    println!("    Project([name, age])");
    println!("      Filter(age > 50 AND active)");
    println!("        Scan(users: 1,000,000 rows)");
    println!("\nEach operator calls input.next() exactly once per output row.");
    println!("The Scan was never asked for more than ~60 rows total");
    println!("(enough to find 10 matching rows after filtering).");
}
```

One million rows in the table, but only about 60 rows were ever read from the scan -- just enough to find 10 that pass the filter. The remaining 999,940 rows were never touched. The Limit operator stopped pulling, and the entire pipeline stopped.

---

## Complexity Table

| Operation | Time per `next()` | Memory | Notes |
|-----------|-------------------|--------|-------|
| Scan | O(1) | O(1) per row | Returns one row, advances cursor |
| Filter | O(k) amortized | O(1) | k = rows skipped before a match |
| Project | O(c) | O(c) | c = number of projected columns |
| Limit | O(1) | O(1) | Decrements counter |
| Full pipeline (n rows, f filter rate) | O(n * f) | O(row_size) | Only processes n*f rows through filter |
| With LIMIT m | O(m / f) expected | O(row_size) | Stops after m output rows |

The critical insight: memory usage is **O(row_size)** regardless of table size. A table with 1 billion rows uses the same memory as a table with 100 rows. The pipeline only ever holds one row at a time. This is why databases can handle tables far larger than available RAM.

---

## Where This Shows Up in Our Database

In Chapter 10, we build the query executor using the Volcano model:

```rust,ignore
// The executor turns a plan tree into a pipeline of operators:
pub fn execute(plan: PlanNode) -> Box<dyn Operator> {
    match plan {
        PlanNode::Scan { table, .. } => Box::new(Scan::new(load_table(&table))),
        PlanNode::Filter { predicate, input } => {
            let input = execute(*input);
            Box::new(Filter::new(input, compile_predicate(predicate)))
        }
        PlanNode::Project { columns, input } => {
            let input = execute(*input);
            Box::new(Project::new(input, columns))
        }
        // ...
    }
}
```

The Volcano model is the standard execution model for row-oriented databases:
- **PostgreSQL** uses the Volcano/iterator model for all query execution
- **MySQL** uses a similar pull-based iterator model
- **SQLite** uses a virtual machine that operates row by row
- **Modern columnar databases** (DuckDB, Snowflake) use a variant called "vectorized execution" that pulls batches of rows instead of one at a time for better CPU cache utilization

The Volcano model trades CPU efficiency (function call overhead per row) for simplicity and memory efficiency. For disk-bound queries, the overhead is negligible. For CPU-bound analytical queries, vectorized execution (pulling 1,000 rows at a time instead of 1) provides better performance. But the pull-based composition pattern remains the same.

---

## Try It Yourself

### Exercise 1: Count Operator

Implement a `Count` operator that counts all rows from its input and returns a single row with one column: the count. `SELECT COUNT(*) FROM users WHERE age > 30` would pipeline as `Count -> Filter -> Scan`. The Count operator's `next()` should return `Some(vec![Value::Integer(count)])` on the first call and `None` on subsequent calls.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Value { Integer(i64), Str(String) }
type Row = Vec<Value>;

#[derive(Debug, Clone)]
struct Schema { columns: Vec<String> }
impl Schema {
    fn new(cols: Vec<&str>) -> Self {
        Schema { columns: cols.iter().map(|s| s.to_string()).collect() }
    }
}

trait Operator {
    fn next(&mut self) -> Option<Row>;
    fn schema(&self) -> &Schema;
}

struct Scan { data: Vec<Row>, pos: usize, schema: Schema }
impl Operator for Scan {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.data.len() {
            let r = self.data[self.pos].clone(); self.pos += 1; Some(r)
        } else { None }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct Filter {
    input: Box<dyn Operator>,
    pred: Box<dyn Fn(&Row) -> bool>,
    schema: Schema,
}
impl Operator for Filter {
    fn next(&mut self) -> Option<Row> {
        loop { let r = self.input.next()?; if (self.pred)(&r) { return Some(r); } }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct Count {
    input: Box<dyn Operator>,
    done: bool,
    schema: Schema,
}

impl Count {
    fn new(input: Box<dyn Operator>) -> Self {
        Count {
            input,
            done: false,
            schema: Schema::new(vec!["count"]),
        }
    }
}

impl Operator for Count {
    fn next(&mut self) -> Option<Row> {
        if self.done { return None; }
        self.done = true;

        // Drain the entire input, counting rows
        let mut count: i64 = 0;
        while self.input.next().is_some() {
            count += 1;
        }

        Some(vec![Value::Integer(count)])
    }

    fn schema(&self) -> &Schema { &self.schema }
}

fn main() {
    let schema = Schema::new(vec!["name", "age"]);
    let data: Vec<Row> = (0..100)
        .map(|i| vec![Value::Str(format!("user_{}", i)), Value::Integer(18 + i % 60)])
        .collect();

    let scan = Scan { data, pos: 0, schema: schema.clone() };
    let filter = Filter {
        input: Box::new(scan),
        pred: Box::new(|row: &Row| {
            matches!(&row[1], Value::Integer(n) if *n > 30)
        }),
        schema,
    };
    let mut count = Count::new(Box::new(filter));

    // First call returns the count
    let result = count.next().unwrap();
    println!("COUNT(*) WHERE age > 30: {:?}", result[0]);

    // Second call returns None
    assert!(count.next().is_none());
    println!("Second call: None (correct)");
}
```

</details>

### Exercise 2: HashAggregate Operator

Implement a `HashAggregate` operator that groups rows by a key column and computes SUM of a value column. For `SELECT city, SUM(salary) FROM employees GROUP BY city`, the operator accumulates sums in a HashMap, then emits one row per group.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
enum Value { Integer(i64), Str(String) }
type Row = Vec<Value>;

#[derive(Debug, Clone)]
struct Schema { columns: Vec<String> }
impl Schema {
    fn new(cols: Vec<&str>) -> Self {
        Schema { columns: cols.iter().map(|s| s.to_string()).collect() }
    }
    fn index_of(&self, name: &str) -> usize {
        self.columns.iter().position(|c| c == name).unwrap()
    }
}

trait Operator {
    fn next(&mut self) -> Option<Row>;
    fn schema(&self) -> &Schema;
}

struct Scan { data: Vec<Row>, pos: usize, schema: Schema }
impl Operator for Scan {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.data.len() {
            let r = self.data[self.pos].clone(); self.pos += 1; Some(r)
        } else { None }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

struct HashAggregate {
    results: Vec<Row>,
    pos: usize,
    schema: Schema,
}

impl HashAggregate {
    fn new(
        mut input: Box<dyn Operator>,
        group_col: &str,
        sum_col: &str,
    ) -> Self {
        let input_schema = input.schema().clone();
        let group_idx = input_schema.index_of(group_col);
        let sum_idx = input_schema.index_of(sum_col);

        // Build hash table of groups
        let mut groups: HashMap<String, i64> = HashMap::new();
        while let Some(row) = input.next() {
            let key = match &row[group_idx] {
                Value::Str(s) => s.clone(),
                Value::Integer(n) => n.to_string(),
            };
            let val = match &row[sum_idx] {
                Value::Integer(n) => *n,
                _ => 0,
            };
            *groups.entry(key).or_insert(0) += val;
        }

        // Convert to output rows
        let mut results: Vec<Row> = groups.into_iter()
            .map(|(key, sum)| vec![Value::Str(key), Value::Integer(sum)])
            .collect();
        results.sort_by(|a, b| {
            let ka = match &a[0] { Value::Str(s) => s.clone(), _ => String::new() };
            let kb = match &b[0] { Value::Str(s) => s.clone(), _ => String::new() };
            ka.cmp(&kb)
        });

        let schema = Schema::new(vec![group_col, &format!("SUM({})", sum_col)]);
        HashAggregate { results, pos: 0, schema }
    }
}

impl Operator for HashAggregate {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.results.len() {
            let r = self.results[self.pos].clone();
            self.pos += 1;
            Some(r)
        } else { None }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

fn main() {
    let schema = Schema::new(vec!["name", "city", "salary"]);
    let data = vec![
        vec![Value::Str("Alice".into()), Value::Str("NYC".into()), Value::Integer(80000)],
        vec![Value::Str("Bob".into()), Value::Str("SF".into()), Value::Integer(90000)],
        vec![Value::Str("Carol".into()), Value::Str("NYC".into()), Value::Integer(85000)],
        vec![Value::Str("Dave".into()), Value::Str("SF".into()), Value::Integer(95000)],
        vec![Value::Str("Eve".into()), Value::Str("NYC".into()), Value::Integer(70000)],
    ];

    let scan = Scan { data, pos: 0, schema };
    let mut agg = HashAggregate::new(Box::new(scan), "city", "salary");

    println!("SELECT city, SUM(salary) FROM employees GROUP BY city:");
    while let Some(row) = agg.next() {
        let city = match &row[0] { Value::Str(s) => s.as_str(), _ => "?" };
        let sum = match &row[1] { Value::Integer(n) => *n, _ => 0 };
        println!("  {} | ${}", city, sum);
    }
    // NYC: 235000, SF: 185000
}
```

</details>

### Exercise 3: NestedLoopJoin Operator

Implement a `NestedLoopJoin` operator that joins two inputs on a key column. For each row from the left input, scan the entire right input looking for matches. This is O(n*m) -- terrible for large tables, but simple and correct. Return joined rows with columns from both sides.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Value { Integer(i64), Str(String) }
type Row = Vec<Value>;

#[derive(Debug, Clone)]
struct Schema { columns: Vec<String> }
impl Schema {
    fn new(cols: Vec<&str>) -> Self {
        Schema { columns: cols.iter().map(|s| s.to_string()).collect() }
    }
    fn index_of(&self, name: &str) -> usize {
        self.columns.iter().position(|c| c == name).unwrap()
    }
}

trait Operator {
    fn next(&mut self) -> Option<Row>;
    fn schema(&self) -> &Schema;
}

struct Scan { data: Vec<Row>, pos: usize, schema: Schema }
impl Operator for Scan {
    fn next(&mut self) -> Option<Row> {
        if self.pos < self.data.len() {
            let r = self.data[self.pos].clone(); self.pos += 1; Some(r)
        } else { None }
    }
    fn schema(&self) -> &Schema { &self.schema }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        _ => false,
    }
}

struct NestedLoopJoin {
    current_left: Option<Row>,
    left: Box<dyn Operator>,
    right_data: Vec<Row>,
    right_pos: usize,
    left_key_idx: usize,
    right_key_idx: usize,
    schema: Schema,
}

impl NestedLoopJoin {
    fn new(
        mut left: Box<dyn Operator>,
        mut right: Box<dyn Operator>,
        left_key: &str,
        right_key: &str,
    ) -> Self {
        let left_schema = left.schema().clone();
        let right_schema = right.schema().clone();
        let left_key_idx = left_schema.index_of(left_key);
        let right_key_idx = right_schema.index_of(right_key);

        // Materialize right side (must scan it multiple times)
        let mut right_data = Vec::new();
        while let Some(row) = right.next() {
            right_data.push(row);
        }

        // Build joined schema
        let mut cols = left_schema.columns.clone();
        for (i, c) in right_schema.columns.iter().enumerate() {
            if i != right_key_idx { // skip duplicate key column
                cols.push(c.clone());
            }
        }
        let schema = Schema { columns: cols };

        let current_left = left.next();

        NestedLoopJoin {
            current_left,
            left,
            right_data,
            right_pos: 0,
            left_key_idx,
            right_key_idx,
            schema,
        }
    }
}

impl Operator for NestedLoopJoin {
    fn next(&mut self) -> Option<Row> {
        loop {
            let left_row = self.current_left.as_ref()?;

            while self.right_pos < self.right_data.len() {
                let right_row = &self.right_data[self.right_pos];
                self.right_pos += 1;

                if values_eq(&left_row[self.left_key_idx], &right_row[self.right_key_idx]) {
                    // Build joined row
                    let mut result = left_row.clone();
                    for (i, val) in right_row.iter().enumerate() {
                        if i != self.right_key_idx {
                            result.push(val.clone());
                        }
                    }
                    return Some(result);
                }
            }

            // Exhausted right side for current left row -- advance left
            self.current_left = self.left.next();
            self.right_pos = 0;
        }
    }

    fn schema(&self) -> &Schema { &self.schema }
}

fn fmt(v: &Value) -> String {
    match v { Value::Integer(n) => n.to_string(), Value::Str(s) => s.clone() }
}

fn main() {
    let users_schema = Schema::new(vec!["user_id", "name"]);
    let users = vec![
        vec![Value::Integer(1), Value::Str("Alice".into())],
        vec![Value::Integer(2), Value::Str("Bob".into())],
        vec![Value::Integer(3), Value::Str("Carol".into())],
    ];

    let orders_schema = Schema::new(vec!["order_id", "user_id", "amount"]);
    let orders = vec![
        vec![Value::Integer(101), Value::Integer(1), Value::Integer(50)],
        vec![Value::Integer(102), Value::Integer(2), Value::Integer(30)],
        vec![Value::Integer(103), Value::Integer(1), Value::Integer(75)],
        vec![Value::Integer(104), Value::Integer(3), Value::Integer(20)],
    ];

    let left = Scan { data: users, pos: 0, schema: users_schema };
    let right = Scan { data: orders, pos: 0, schema: orders_schema };

    let mut join = NestedLoopJoin::new(
        Box::new(left), Box::new(right),
        "user_id", "user_id",
    );

    println!("SELECT * FROM users JOIN orders ON users.user_id = orders.user_id:");
    println!("  {:?}", join.schema().columns);
    while let Some(row) = join.next() {
        let vals: Vec<String> = row.iter().map(fmt).collect();
        println!("  {}", vals.join(" | "));
    }
}
```

</details>

---

## Recap

The Volcano model turns a query plan into a pipeline of operators, each with one method: `next()`. Data flows from bottom to top, one row at a time. Filter skips non-matching rows without storing them. Project narrows rows without buffering the originals. Limit stops the entire pipeline after N rows. The result: constant memory regardless of table size, natural composition of arbitrary operator trees, and early termination for queries that need only a few rows. It is the execution model that makes databases possible.
