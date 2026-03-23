## Exercise 1: Implement NestedLoopJoinExecutor

**Goal:** Build a join operator that combines rows from two tables using nested loops — the simplest join algorithm.

### Step 1: Understand the join

A JOIN combines rows from two tables based on a condition. For example:

```sql
SELECT users.name, orders.item
FROM users
JOIN orders ON users.id = orders.user_id;
```

The nested loop join is the brute-force approach: for each row in the left table, scan the entire right table and output combined rows where the condition is true. If the left table has `n` rows and the right has `m` rows, this examines `n * m` row pairs.

```
For each row L in left:
    For each row R in right:
        If join_condition(L, R):
            Emit combined row (L + R)
```

### Step 2: Design the executor

The tricky part of a nested-loop join in the Volcano model is maintaining state across `next()` calls. We need to remember where we are in the double loop:

```rust
// src/executor.rs (continued)

/// Joins two executors using a nested loop.
///
/// For each row in the left (outer) executor, iterates through all rows
/// in the right (inner) executor, emitting combined rows where the
/// join predicate evaluates to true.
///
/// This is O(n * m) — the simplest but slowest join algorithm.
pub struct NestedLoopJoinExecutor {
    /// The outer (left) executor.
    left: Box<dyn Executor>,
    /// The inner (right) rows — materialized because we re-scan for each left row.
    right_rows: Vec<Row>,
    /// Column names for the right side.
    right_columns: Vec<String>,
    /// The join predicate (e.g., users.id = orders.user_id).
    predicate: Expression,
    /// Combined column names: left columns + right columns.
    combined_columns: Vec<String>,
    /// The current left row we are joining against.
    current_left: Option<Row>,
    /// Current position in the right_rows for the current left row.
    right_position: usize,
}

impl NestedLoopJoinExecutor {
    pub fn new(
        left: Box<dyn Executor>,
        mut right: Box<dyn Executor>,
        predicate: Expression,
    ) -> Result<Self, ExecutorError> {
        // Materialize the right side — we re-scan it for every left row.
        let right_columns = right.columns().to_vec();
        let mut right_rows = Vec::new();
        while let Some(row) = right.next()? {
            right_rows.push(row);
        }

        // Build combined column names: left.col1, left.col2, right.col1, ...
        let combined_columns: Vec<String> = left.columns().iter()
            .chain(right_columns.iter())
            .cloned()
            .collect();

        Ok(NestedLoopJoinExecutor {
            left,
            right_rows,
            right_columns,
            predicate,
            combined_columns,
            current_left: None,
            right_position: 0,
        })
    }
}

impl Executor for NestedLoopJoinExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        loop {
            // If we do not have a current left row, pull one
            if self.current_left.is_none() {
                match self.left.next()? {
                    None => return Ok(None), // left exhausted — join complete
                    Some(row) => {
                        self.current_left = Some(row);
                        self.right_position = 0;
                    }
                }
            }

            let left_row = self.current_left.as_ref().unwrap();

            // Scan through right rows from our current position
            while self.right_position < self.right_rows.len() {
                let right_row = &self.right_rows[self.right_position];
                self.right_position += 1;

                // Combine left + right into a single row
                let combined = Row::new(
                    left_row.values.iter()
                        .chain(right_row.values.iter())
                        .cloned()
                        .collect()
                );

                // Evaluate the join predicate against the combined row
                let result = evaluate(
                    &self.predicate,
                    &combined,
                    &self.combined_columns,
                )?;

                match result {
                    Value::Boolean(true) => return Ok(Some(combined)),
                    Value::Boolean(false) | Value::Null => continue,
                    other => return Err(ExecutorError::TypeError(
                        format!("JOIN predicate must be boolean, got {:?}", other)
                    )),
                }
            }

            // Right side exhausted for this left row — move to next left row
            self.current_left = None;
        }
    }

    fn columns(&self) -> &[String] {
        &self.combined_columns
    }
}
```

### Step 3: Understand the state machine

The `next()` method on a nested-loop join is a state machine with two levels:

1. **Outer level:** Pull a row from the left executor. If exhausted, the join is done.
2. **Inner level:** For the current left row, scan through right rows starting from `right_position`. If a match is found, return it. If the right side is exhausted, go back to step 1.

The `current_left` and `right_position` fields maintain state between `next()` calls. Each call resumes exactly where the previous call left off. This is how the Volcano model turns a nested loop into an iterator — the double loop is "unrolled" across multiple `next()` calls.

### Step 4: Test the join

```rust
#[cfg(test)]
mod tests {
    // ... (previous tests) ...

    fn orders_storage() -> Storage {
        let mut storage = Storage::new();

        storage.create_table("users", vec![
            "id".to_string(), "name".to_string(),
        ]);
        storage.insert_row("users", Row::new(vec![
            Value::Integer(1), Value::String("Alice".to_string()),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(2), Value::String("Bob".to_string()),
        ])).unwrap();
        storage.insert_row("users", Row::new(vec![
            Value::Integer(3), Value::String("Carol".to_string()),
        ])).unwrap();

        storage.create_table("orders", vec![
            "order_id".to_string(),
            "user_id".to_string(),
            "item".to_string(),
        ]);
        storage.insert_row("orders", Row::new(vec![
            Value::Integer(101),
            Value::Integer(1),
            Value::String("Widget".to_string()),
        ])).unwrap();
        storage.insert_row("orders", Row::new(vec![
            Value::Integer(102),
            Value::Integer(2),
            Value::String("Gadget".to_string()),
        ])).unwrap();
        storage.insert_row("orders", Row::new(vec![
            Value::Integer(103),
            Value::Integer(1),
            Value::String("Doohickey".to_string()),
        ])).unwrap();

        storage
    }

    #[test]
    fn test_nested_loop_join() {
        let storage = orders_storage();

        let left = ScanExecutor::new(&storage, "users").unwrap();
        let right = ScanExecutor::new(&storage, "orders").unwrap();

        // JOIN ON users.id = orders.user_id
        let predicate = Expression::BinaryOp {
            left: Box::new(Expression::ColumnRef("id".to_string())),
            op: BinaryOperator::Equal,
            right: Box::new(Expression::ColumnRef("user_id".to_string())),
        };

        let mut join = NestedLoopJoinExecutor::new(
            Box::new(left),
            Box::new(right),
            predicate,
        ).unwrap();

        // Alice has 2 orders, Bob has 1, Carol has 0
        let r1 = join.next().unwrap().unwrap();
        assert_eq!(r1.values[1], Value::String("Alice".to_string()));
        assert_eq!(r1.values[4], Value::String("Widget".to_string()));

        let r2 = join.next().unwrap().unwrap();
        assert_eq!(r2.values[1], Value::String("Alice".to_string()));
        assert_eq!(r2.values[4], Value::String("Doohickey".to_string()));

        let r3 = join.next().unwrap().unwrap();
        assert_eq!(r3.values[1], Value::String("Bob".to_string()));
        assert_eq!(r3.values[4], Value::String("Gadget".to_string()));

        // Carol has no matching orders — she does not appear
        assert_eq!(join.next().unwrap(), None);
    }
}
```

```
Expected output:
$ cargo test test_nested_loop_join
running 1 test
test executor::tests::test_nested_loop_join ... ok
test result: ok. 1 passed; 0 failed
```

<details>
<summary>Hint: If the join produces duplicate or missing rows</summary>

The most common bug is not resetting `right_position` to 0 when advancing to the next left row. Each left row must scan the entire right side from the beginning. Check that `self.right_position = 0` is set when `self.current_left` is replaced with a new row.

Another common issue is column name collisions. If both tables have an "id" column, `ColumnRef("id")` will match the first one found in the combined columns list. For production code, you would qualify column names with table prefixes (`users.id`, `orders.user_id`). For now, ensure your test tables use distinct column names.

</details>

---

## Exercise 2: Implement HashJoinExecutor

**Goal:** Build a hash join — the same result as nested loop, but O(n+m) instead of O(n*m).

### Step 1: Understand hash joins

The nested loop join examines every pair of rows. For tables with 10,000 rows each, that is 100 million comparisons. A hash join reduces this to ~20,000 operations:

1. **Build phase:** Read all rows from the smaller table. For each row, compute a hash of the join key and store the row in a hash table.
2. **Probe phase:** Read rows from the larger table one at a time. For each row, compute a hash of the join key and look up matching rows in the hash table.

```
Build phase (right table):
  orders row 1 (user_id=1): hash(1) -> bucket A -> store row
  orders row 2 (user_id=2): hash(2) -> bucket B -> store row
  orders row 3 (user_id=1): hash(1) -> bucket A -> store row

Probe phase (left table):
  users row 1 (id=1): hash(1) -> bucket A -> found 2 matches -> emit 2 rows
  users row 2 (id=2): hash(2) -> bucket B -> found 1 match -> emit 1 row
  users row 3 (id=3): hash(3) -> no bucket -> skip
```

### Step 2: Implement the hash join

```rust
// src/executor.rs (continued)

/// Joins two executors using a hash table.
///
/// Build phase: materializes the right (build) side into a HashMap
/// keyed by the join column value.
/// Probe phase: for each left (probe) row, looks up matching right rows.
///
/// This is O(n + m) — much faster than nested loop for large tables.
pub struct HashJoinExecutor {
    /// The probe (left) executor — rows are pulled one at a time.
    probe: Box<dyn Executor>,
    /// Hash table: join_key_value -> list of matching right rows.
    build_table: HashMap<HashableValue, Vec<Row>>,
    /// Column names for the right (build) side.
    build_columns: Vec<String>,
    /// The column name to join on (left side).
    probe_key: String,
    /// The column name to join on (right side).
    build_key: String,
    /// Combined column names.
    combined_columns: Vec<String>,
    /// Current probe row being joined.
    current_probe: Option<Row>,
    /// Matching build rows for the current probe row.
    current_matches: Vec<Row>,
    /// Position in current_matches.
    match_position: usize,
}

/// A wrapper around Value that implements Hash + Eq for use as HashMap keys.
/// Floats are handled by converting to bits representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashableValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(u64), // f64 bits for hash equality
    String(String),
}

impl std::hash::Hash for HashableValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            HashableValue::Null => {}
            HashableValue::Boolean(b) => b.hash(state),
            HashableValue::Integer(i) => i.hash(state),
            HashableValue::Float(bits) => bits.hash(state),
            HashableValue::String(s) => s.hash(state),
        }
    }
}

impl HashableValue {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => HashableValue::Null,
            Value::Boolean(b) => HashableValue::Boolean(*b),
            Value::Integer(i) => HashableValue::Integer(*i),
            Value::Float(f) => HashableValue::Float(f.to_bits()),
            Value::String(s) => HashableValue::String(s.clone()),
        }
    }
}

impl HashJoinExecutor {
    pub fn new(
        probe: Box<dyn Executor>,
        mut build: Box<dyn Executor>,
        probe_key: String,
        build_key: String,
    ) -> Result<Self, ExecutorError> {
        let build_columns = build.columns().to_vec();

        // Find the build key column index
        let build_key_index = build_columns.iter()
            .position(|c| c == &build_key)
            .ok_or_else(|| ExecutorError::ColumnNotFound(build_key.clone()))?;

        // Build phase: read all rows from the build side into a hash table
        let mut build_table: HashMap<HashableValue, Vec<Row>> = HashMap::new();
        while let Some(row) = build.next()? {
            let key = HashableValue::from_value(&row.values[build_key_index]);
            build_table.entry(key).or_insert_with(Vec::new).push(row);
        }

        let combined_columns: Vec<String> = probe.columns().iter()
            .chain(build_columns.iter())
            .cloned()
            .collect();

        Ok(HashJoinExecutor {
            probe,
            build_table,
            build_columns,
            probe_key,
            build_key,
            combined_columns,
            current_probe: None,
            current_matches: Vec::new(),
            match_position: 0,
        })
    }
}

impl Executor for HashJoinExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        loop {
            // If we have remaining matches for the current probe row, yield one
            if self.match_position < self.current_matches.len() {
                let probe_row = self.current_probe.as_ref().unwrap();
                let build_row = &self.current_matches[self.match_position];
                self.match_position += 1;

                let combined = Row::new(
                    probe_row.values.iter()
                        .chain(build_row.values.iter())
                        .cloned()
                        .collect()
                );

                return Ok(Some(combined));
            }

            // Pull the next probe row
            match self.probe.next()? {
                None => return Ok(None),
                Some(probe_row) => {
                    // Find the probe key column index
                    let probe_columns = self.probe.columns();
                    let probe_key_index = probe_columns.iter()
                        .position(|c| c == &self.probe_key)
                        .ok_or_else(|| {
                            ExecutorError::ColumnNotFound(self.probe_key.clone())
                        })?;

                    // Look up matching build rows
                    let key = HashableValue::from_value(
                        &probe_row.values[probe_key_index]
                    );

                    self.current_matches = self.build_table
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    self.match_position = 0;
                    self.current_probe = Some(probe_row);
                }
            }
        }
    }

    fn columns(&self) -> &[String] {
        &self.combined_columns
    }
}
```

### Step 3: Understand the HashableValue wrapper

Rust's `HashMap` requires keys to implement `Hash + Eq`. Our `Value` enum contains `f64`, which does NOT implement `Hash` because floating-point equality is problematic (NaN != NaN, -0.0 == +0.0).

The `HashableValue` wrapper solves this by converting `f64` to its bit representation (`u64`). Two floats are equal in the hash table if and only if they have identical bit patterns. This means NaN != NaN (different bit patterns for signaling and quiet NaN) and -0.0 != +0.0 (different sign bits). For database joins, this is acceptable — you rarely join on floating-point columns, and when you do, exact bit equality is the safest behavior.

### Step 4: Test the hash join

```rust
#[cfg(test)]
mod tests {
    // ... (previous tests) ...

    #[test]
    fn test_hash_join() {
        let storage = orders_storage();

        let left = ScanExecutor::new(&storage, "users").unwrap();
        let right = ScanExecutor::new(&storage, "orders").unwrap();

        let mut join = HashJoinExecutor::new(
            Box::new(left),
            Box::new(right),
            "id".to_string(),       // probe key (left)
            "user_id".to_string(),  // build key (right)
        ).unwrap();

        // Same results as nested loop, possibly in different order
        // within each probe group
        let mut results = Vec::new();
        while let Some(row) = join.next().unwrap() {
            results.push(row);
        }

        assert_eq!(results.len(), 3); // Alice x2, Bob x1, Carol x0

        // Verify Alice's orders
        let alice_rows: Vec<&Row> = results.iter()
            .filter(|r| r.values[1] == Value::String("Alice".to_string()))
            .collect();
        assert_eq!(alice_rows.len(), 2);

        // Verify Bob's order
        let bob_rows: Vec<&Row> = results.iter()
            .filter(|r| r.values[1] == Value::String("Bob".to_string()))
            .collect();
        assert_eq!(bob_rows.len(), 1);

        // Carol has no orders — not in results (INNER JOIN)
        let carol_rows: Vec<&Row> = results.iter()
            .filter(|r| r.values[1] == Value::String("Carol".to_string()))
            .collect();
        assert_eq!(carol_rows.len(), 0);
    }

    #[test]
    fn test_hash_join_no_matches() {
        let mut storage = Storage::new();
        storage.create_table("left_t", vec!["id".to_string()]);
        storage.insert_row("left_t", Row::new(vec![Value::Integer(99)])).unwrap();

        storage.create_table("right_t", vec!["id".to_string()]);
        storage.insert_row("right_t", Row::new(vec![Value::Integer(1)])).unwrap();

        let left = ScanExecutor::new(&storage, "left_t").unwrap();
        let right = ScanExecutor::new(&storage, "right_t").unwrap();

        let mut join = HashJoinExecutor::new(
            Box::new(left),
            Box::new(right),
            "id".to_string(),
            "id".to_string(),
        ).unwrap();

        // No matching keys
        assert_eq!(join.next().unwrap(), None);
    }
}
```

```
Expected output:
$ cargo test test_hash_join
running 2 tests
test executor::tests::test_hash_join ... ok
test executor::tests::test_hash_join_no_matches ... ok
test result: ok. 2 passed; 0 failed
```

<details>
<summary>Hint: If the hash join produces wrong results</summary>

The most common bug is looking up the probe key in the wrong column list. The probe key must be looked up in `self.probe.columns()` (the left side), and the build key must be looked up in `self.build_columns` (the right side). If you look up `"id"` in the combined columns, you might get the wrong index because `id` appears in both tables.

Another issue: the build phase must happen in the constructor, not in `next()`. If you try to build lazily on the first `next()` call, you will have borrowed `self` mutably twice (once for `self.build.next()` and once for `self.build_table.insert()`).

</details>

---

## Exercise 3: Implement AggregateExecutor with GROUP BY

**Goal:** Build an aggregation operator that groups rows and computes COUNT, SUM, AVG, MIN, and MAX.

### Step 1: Define aggregation types

```rust
// src/executor.rs (continued)

/// An aggregation function applied to a column or expression.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

/// A single aggregation: function + the expression to aggregate.
#[derive(Debug, Clone)]
pub struct Aggregation {
    pub function: AggregateFunction,
    pub expression: Expression,
    /// The output column name (e.g., "COUNT(*)", "SUM(salary)")
    pub alias: String,
}

/// Accumulator for computing aggregations incrementally.
/// Each group has one Accumulator per aggregation.
#[derive(Debug, Clone)]
struct Accumulator {
    function: AggregateFunction,
    count: i64,
    sum: f64,
    min: Option<Value>,
    max: Option<Value>,
}

impl Accumulator {
    fn new(function: AggregateFunction) -> Self {
        Accumulator {
            function,
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
        }
    }

    fn accumulate(&mut self, value: &Value) {
        // Skip NULLs (SQL standard: aggregates ignore NULL values)
        if matches!(value, Value::Null) {
            return;
        }

        self.count += 1;

        match &self.function {
            AggregateFunction::Count => {
                // count is already incremented above
            }
            AggregateFunction::Sum | AggregateFunction::Avg => {
                match value {
                    Value::Integer(i) => self.sum += *i as f64,
                    Value::Float(f) => self.sum += f,
                    _ => {} // silently skip non-numeric values
                }
            }
            AggregateFunction::Min => {
                let should_replace = match &self.min {
                    None => true,
                    Some(current) => value_less_than(value, current),
                };
                if should_replace {
                    self.min = Some(value.clone());
                }
            }
            AggregateFunction::Max => {
                let should_replace = match &self.max {
                    None => true,
                    Some(current) => value_less_than(current, value),
                };
                if should_replace {
                    self.max = Some(value.clone());
                }
            }
        }
    }

    fn result(&self) -> Value {
        match &self.function {
            AggregateFunction::Count => Value::Integer(self.count),
            AggregateFunction::Sum => {
                if self.count == 0 {
                    Value::Null
                } else {
                    Value::Float(self.sum)
                }
            }
            AggregateFunction::Avg => {
                if self.count == 0 {
                    Value::Null
                } else {
                    Value::Float(self.sum / self.count as f64)
                }
            }
            AggregateFunction::Min => {
                self.min.clone().unwrap_or(Value::Null)
            }
            AggregateFunction::Max => {
                self.max.clone().unwrap_or(Value::Null)
            }
        }
    }
}

/// Compare two values, returning true if a < b.
fn value_less_than(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => a < b,
        (Value::Float(a), Value::Float(b)) => a < b,
        (Value::Integer(a), Value::Float(b)) => (*a as f64) < *b,
        (Value::Float(a), Value::Integer(b)) => *a < (*b as f64),
        (Value::String(a), Value::String(b)) => a < b,
        (Value::Boolean(a), Value::Boolean(b)) => !a & b, // false < true
        _ => false,
    }
}
```

### Step 2: Build the AggregateExecutor

```rust
// src/executor.rs (continued)

/// Groups rows and computes aggregation functions.
///
/// This executor is NOT lazy — it must read ALL rows from the source
/// before it can produce any output. This is because GROUP BY requires
/// seeing every row to know which groups exist and to compute the
/// final aggregation values.
pub struct AggregateExecutor {
    /// Pre-computed result rows (group key values + aggregation results).
    results: Vec<Row>,
    /// Current position in results.
    position: usize,
    /// Output column names.
    output_columns: Vec<String>,
}

impl AggregateExecutor {
    pub fn new(
        mut source: Box<dyn Executor>,
        group_by: Vec<String>,
        aggregations: Vec<Aggregation>,
    ) -> Result<Self, ExecutorError> {
        let source_columns = source.columns().to_vec();

        // Find group-by column indices
        let group_indices: Vec<usize> = group_by.iter()
            .map(|name| {
                source_columns.iter()
                    .position(|c| c == name)
                    .ok_or_else(|| ExecutorError::ColumnNotFound(name.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Read all rows and group them
        // Key: group key values (as a Vec<HashableValue>)
        // Value: accumulators for each aggregation
        let mut groups: HashMap<Vec<HashableValue>, Vec<Accumulator>> = HashMap::new();

        // If no GROUP BY, everything is one group
        let has_groups = !group_by.is_empty();

        while let Some(row) = source.next()? {
            // Compute the group key
            let group_key: Vec<HashableValue> = if has_groups {
                group_indices.iter()
                    .map(|&i| HashableValue::from_value(&row.values[i]))
                    .collect()
            } else {
                vec![] // single group
            };

            // Get or create accumulators for this group
            let accumulators = groups.entry(group_key).or_insert_with(|| {
                aggregations.iter()
                    .map(|agg| Accumulator::new(agg.function.clone()))
                    .collect()
            });

            // Feed each aggregation expression's value into its accumulator
            for (i, agg) in aggregations.iter().enumerate() {
                let value = evaluate(&agg.expression, &row, &source_columns)?;
                accumulators[i].accumulate(&value);
            }
        }

        // Build output column names
        let mut output_columns: Vec<String> = group_by.clone();
        for agg in &aggregations {
            output_columns.push(agg.alias.clone());
        }

        // Convert groups into result rows
        let mut results = Vec::new();
        for (group_key, accumulators) in groups {
            let mut values = Vec::new();

            // Group key values
            for key_val in &group_key {
                values.push(match key_val {
                    HashableValue::Null => Value::Null,
                    HashableValue::Boolean(b) => Value::Boolean(*b),
                    HashableValue::Integer(i) => Value::Integer(*i),
                    HashableValue::Float(bits) => Value::Float(f64::from_bits(*bits)),
                    HashableValue::String(s) => Value::String(s.clone()),
                });
            }

            // Aggregation results
            for acc in &accumulators {
                values.push(acc.result());
            }

            results.push(Row::new(values));
        }

        Ok(AggregateExecutor {
            results,
            position: 0,
            output_columns,
        })
    }
}

impl Executor for AggregateExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        if self.position >= self.results.len() {
            return Ok(None);
        }
        let row = self.results[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.output_columns
    }
}
```

### Step 3: Understand why aggregation breaks laziness

Every executor so far has been lazy — it processes one row at a time without materializing the entire input. The `AggregateExecutor` is the first operator that cannot be lazy. Consider `SELECT department, AVG(salary) FROM employees GROUP BY department`:

- Until you have seen ALL employees, you do not know the final AVG for any department.
- The last employee processed might belong to any department, changing its average.

This is a fundamental property of aggregation: it requires a full pass over the data before producing any output. In database terminology, this is a **blocking operator** — it blocks the pipeline until all input is consumed.

The same is true for ORDER BY (you cannot output the smallest value until you have seen all values) and for hash joins during the build phase (you cannot probe until the hash table is complete).

### Step 4: Test the aggregation

```rust
#[cfg(test)]
mod tests {
    // ... (previous tests) ...

    fn employee_storage() -> Storage {
        let mut storage = Storage::new();
        storage.create_table("employees", vec![
            "name".to_string(),
            "department".to_string(),
            "salary".to_string(),
        ]);

        let employees = vec![
            ("Alice",   "Engineering", 120000),
            ("Bob",     "Engineering", 110000),
            ("Carol",   "Marketing",   90000),
            ("Dave",    "Engineering", 130000),
            ("Eve",     "Marketing",   95000),
            ("Frank",   "Sales",       80000),
        ];

        for (name, dept, salary) in employees {
            storage.insert_row("employees", Row::new(vec![
                Value::String(name.to_string()),
                Value::String(dept.to_string()),
                Value::Integer(salary),
            ])).unwrap();
        }

        storage
    }

    #[test]
    fn test_aggregate_group_by() {
        let storage = employee_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        let agg = AggregateExecutor::new(
            Box::new(scan),
            vec!["department".to_string()],
            vec![
                Aggregation {
                    function: AggregateFunction::Count,
                    expression: Expression::Literal(Value::Integer(1)),
                    alias: "count".to_string(),
                },
                Aggregation {
                    function: AggregateFunction::Avg,
                    expression: Expression::ColumnRef("salary".to_string()),
                    alias: "avg_salary".to_string(),
                },
            ],
        ).unwrap();

        let result = ResultSet::collect_from(Box::new(agg)).unwrap();

        // 3 departments
        assert_eq!(result.rows.len(), 3);

        // Find Engineering group
        let eng_row = result.rows.iter()
            .find(|r| r.values[0] == Value::String("Engineering".to_string()))
            .expect("Engineering group not found");

        assert_eq!(eng_row.values[1], Value::Integer(3)); // COUNT
        // AVG(120000, 110000, 130000) = 120000.0
        if let Value::Float(avg) = eng_row.values[2] {
            assert!((avg - 120000.0).abs() < 0.01);
        } else {
            panic!("Expected Float for AVG");
        }

        println!("{}", result.display());
    }

    #[test]
    fn test_aggregate_no_group_by() {
        let storage = employee_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        // SELECT COUNT(*), MIN(salary), MAX(salary) FROM employees
        let agg = AggregateExecutor::new(
            Box::new(scan),
            vec![], // no GROUP BY — all rows in one group
            vec![
                Aggregation {
                    function: AggregateFunction::Count,
                    expression: Expression::Literal(Value::Integer(1)),
                    alias: "count".to_string(),
                },
                Aggregation {
                    function: AggregateFunction::Min,
                    expression: Expression::ColumnRef("salary".to_string()),
                    alias: "min_salary".to_string(),
                },
                Aggregation {
                    function: AggregateFunction::Max,
                    expression: Expression::ColumnRef("salary".to_string()),
                    alias: "max_salary".to_string(),
                },
            ],
        ).unwrap();

        let result = ResultSet::collect_from(Box::new(agg)).unwrap();

        assert_eq!(result.rows.len(), 1); // one group (all rows)
        assert_eq!(result.rows[0].values[0], Value::Integer(6));    // COUNT
        assert_eq!(result.rows[0].values[1], Value::Integer(80000)); // MIN
        assert_eq!(result.rows[0].values[2], Value::Integer(130000)); // MAX
    }
}
```

```
Expected output:
$ cargo test test_aggregate -- --nocapture
running 2 tests
department  | count | avg_salary
------------+-------+-----------
Engineering | 3     | 120000.00
Marketing   | 2     | 92500.00
Sales       | 1     | 80000.00
(3 rows)

test executor::tests::test_aggregate_group_by ... ok
test executor::tests::test_aggregate_no_group_by ... ok
test result: ok. 2 passed; 0 failed
```

Note: the order of groups in the output is nondeterministic because `HashMap` does not guarantee iteration order. If you need deterministic output, use `BTreeMap` or add a `SortExecutor` after the aggregation.

<details>
<summary>Hint: If AVG returns the wrong value</summary>

Check that your `Accumulator` tracks `count` and `sum` separately. AVG = sum / count. A common bug is using `count` for both COUNT(*) and the divisor in AVG — but COUNT(*) counts all rows including NULLs, while AVG's count should only count non-NULL values. Our implementation handles this correctly because `accumulate()` skips NULLs before incrementing `count`.

Also verify that SUM accumulates as `f64` — integer overflow is a risk if you accumulate as `i64` for large salary values. Using `f64` avoids overflow at the cost of some precision for very large integers, which is acceptable for AVG.

</details>

---

## Exercise 4: Implement SortExecutor with ORDER BY

**Goal:** Build a sort operator that collects all rows from its source and sorts them by one or more expressions.

### Step 1: Define sort order

```rust
// src/executor.rs (continued)

/// Sort direction for ORDER BY.
#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// A single sort key: expression + direction.
#[derive(Debug, Clone)]
pub struct SortKey {
    pub expression: Expression,
    pub direction: SortDirection,
}
```

### Step 2: Implement SortExecutor

```rust
// src/executor.rs (continued)

/// Sorts all rows from the source by one or more sort keys.
///
/// Like AggregateExecutor, this is a blocking operator — it must
/// read ALL rows before it can produce any output.
pub struct SortExecutor {
    /// Sorted rows.
    rows: Vec<Row>,
    /// Current position.
    position: usize,
    /// Column names (unchanged from source).
    column_names: Vec<String>,
}

impl SortExecutor {
    pub fn new(
        mut source: Box<dyn Executor>,
        sort_keys: Vec<SortKey>,
    ) -> Result<Self, ExecutorError> {
        let column_names = source.columns().to_vec();

        // Collect all rows from the source
        let mut rows = Vec::new();
        while let Some(row) = source.next()? {
            rows.push(row);
        }

        // Sort using a custom comparator
        // We need to handle errors during expression evaluation,
        // but sort_by does not support Result. We pre-evaluate
        // the sort keys for each row and store them alongside.
        let mut keyed_rows: Vec<(Vec<Value>, Row)> = Vec::new();
        for row in rows {
            let mut keys = Vec::new();
            for sk in &sort_keys {
                let val = evaluate(&sk.expression, &row, &column_names)?;
                keys.push(val);
            }
            keyed_rows.push((keys, row));
        }

        keyed_rows.sort_by(|(keys_a, _), (keys_b, _)| {
            for (i, (a, b)) in keys_a.iter().zip(keys_b.iter()).enumerate() {
                let ordering = compare_values(a, b);
                if ordering == std::cmp::Ordering::Equal {
                    continue;
                }
                return match sort_keys[i].direction {
                    SortDirection::Ascending => ordering,
                    SortDirection::Descending => ordering.reverse(),
                };
            }
            std::cmp::Ordering::Equal
        });

        let sorted_rows: Vec<Row> = keyed_rows.into_iter()
            .map(|(_, row)| row)
            .collect();

        Ok(SortExecutor {
            rows: sorted_rows,
            position: 0,
            column_names,
        })
    }
}

/// Compare two Values for sorting. NULL sorts first (before all other values).
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        // NULLs sort first
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,

        // Same-type comparisons
        (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
        (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => a.cmp(b),

        // Cross-type numeric comparisons
        (Value::Integer(a), Value::Float(b)) => {
            (*a as f64).partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Float(a), Value::Integer(b)) => {
            a.partial_cmp(&(*b as f64)).unwrap_or(std::cmp::Ordering::Equal)
        }

        // Different types: use type discriminant for consistent ordering
        _ => std::mem::discriminant(a).hash_code()
            .cmp(&std::mem::discriminant(b).hash_code())
            .then(std::cmp::Ordering::Equal),
    }
}
```

Wait — `discriminant().hash_code()` does not exist. Let us simplify the catch-all case:

```rust
        // Different incomparable types: arbitrary but consistent ordering
        // by type, so sorting is at least stable.
        _ => {
            let type_rank = |v: &Value| -> u8 {
                match v {
                    Value::Null => 0,
                    Value::Boolean(_) => 1,
                    Value::Integer(_) => 2,
                    Value::Float(_) => 3,
                    Value::String(_) => 4,
                }
            };
            type_rank(a).cmp(&type_rank(b))
        }
```

Here is the complete, clean `compare_values`:

```rust
/// Compare two Values for sorting. NULL sorts first (before all other values).
/// Mixed types that are not numerically comparable are ordered by type rank.
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,

        (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
        (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => a.cmp(b),

        (Value::Integer(a), Value::Float(b)) => {
            (*a as f64).partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Float(a), Value::Integer(b)) => {
            a.partial_cmp(&(*b as f64)).unwrap_or(std::cmp::Ordering::Equal)
        }

        _ => {
            let rank = |v: &Value| -> u8 {
                match v {
                    Value::Null => 0,
                    Value::Boolean(_) => 1,
                    Value::Integer(_) => 2,
                    Value::Float(_) => 3,
                    Value::String(_) => 4,
                }
            };
            rank(a).cmp(&rank(b))
        }
    }
}

impl Executor for SortExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        if self.position >= self.rows.len() {
            return Ok(None);
        }
        let row = self.rows[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.column_names
    }
}
```

### Step 3: Understand the sort strategy

The `SortExecutor` uses a **decorate-sort-undecorate** pattern (also known as the Schwartzian transform):

1. **Decorate:** For each row, pre-evaluate all sort key expressions and store them alongside the row as `(Vec<Value>, Row)`.
2. **Sort:** Sort the decorated pairs by comparing the pre-evaluated keys.
3. **Undecorate:** Extract the rows, discarding the keys.

Why not evaluate the sort expression inside the comparator? Because `evaluate()` returns `Result`, and `sort_by`'s closure must return `Ordering` — there is no way to propagate errors. By pre-evaluating, we detect errors before sorting begins.

This pattern also avoids redundant computation. If sorting requires evaluating `salary * 12` for 10,000 rows, and the sort algorithm makes ~130,000 comparisons (for n * log(n)), evaluating in the comparator would compute `salary * 12` 260,000 times (once for each side of each comparison). Pre-evaluating does it 10,000 times.

### Step 4: Test the sort

```rust
#[cfg(test)]
mod tests {
    // ... (previous tests) ...

    #[test]
    fn test_sort_ascending() {
        let storage = sample_storage(); // users: Alice(30), Bob(25), Carol(35), Dave(28)
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        let sort = SortExecutor::new(
            Box::new(scan),
            vec![SortKey {
                expression: Expression::ColumnRef("age".to_string()),
                direction: SortDirection::Ascending,
            }],
        ).unwrap();

        let result = ResultSet::collect_from(Box::new(sort)).unwrap();
        let ages: Vec<&Value> = result.rows.iter()
            .map(|r| &r.values[2])
            .collect();

        assert_eq!(ages, vec![
            &Value::Integer(25),  // Bob
            &Value::Integer(28),  // Dave
            &Value::Integer(30),  // Alice
            &Value::Integer(35),  // Carol
        ]);
    }

    #[test]
    fn test_sort_descending() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        let sort = SortExecutor::new(
            Box::new(scan),
            vec![SortKey {
                expression: Expression::ColumnRef("name".to_string()),
                direction: SortDirection::Descending,
            }],
        ).unwrap();

        let result = ResultSet::collect_from(Box::new(sort)).unwrap();
        let names: Vec<&Value> = result.rows.iter()
            .map(|r| &r.values[1])
            .collect();

        assert_eq!(names, vec![
            &Value::String("Dave".to_string()),
            &Value::String("Carol".to_string()),
            &Value::String("Bob".to_string()),
            &Value::String("Alice".to_string()),
        ]);
    }

    #[test]
    fn test_complex_pipeline() {
        // SELECT department, COUNT(*) as count
        // FROM employees
        // GROUP BY department
        // ORDER BY count DESC

        let storage = employee_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        let agg = AggregateExecutor::new(
            Box::new(scan),
            vec!["department".to_string()],
            vec![Aggregation {
                function: AggregateFunction::Count,
                expression: Expression::Literal(Value::Integer(1)),
                alias: "count".to_string(),
            }],
        ).unwrap();

        let sort = SortExecutor::new(
            Box::new(agg),
            vec![SortKey {
                expression: Expression::ColumnRef("count".to_string()),
                direction: SortDirection::Descending,
            }],
        ).unwrap();

        let result = ResultSet::collect_from(Box::new(sort)).unwrap();
        println!("{}", result.display());

        // Engineering (3) > Marketing (2) > Sales (1)
        assert_eq!(result.rows[0].values[0], Value::String("Engineering".to_string()));
        assert_eq!(result.rows[0].values[1], Value::Integer(3));
        assert_eq!(result.rows[1].values[0], Value::String("Marketing".to_string()));
        assert_eq!(result.rows[1].values[1], Value::Integer(2));
        assert_eq!(result.rows[2].values[0], Value::String("Sales".to_string()));
        assert_eq!(result.rows[2].values[1], Value::Integer(1));
    }
}
```

```
Expected output:
$ cargo test test_complex_pipeline -- --nocapture
running 1 test
department  | count
------------+------
Engineering | 3
Marketing   | 2
Sales       | 1
(3 rows)

test executor::tests::test_complex_pipeline ... ok

$ cargo test test_sort
running 2 tests
test executor::tests::test_sort_ascending ... ok
test executor::tests::test_sort_descending ... ok
test result: ok. 2 passed; 0 failed
```

<details>
<summary>Hint: If the sort order is reversed</summary>

Check that `SortDirection::Descending` uses `ordering.reverse()`, not that you are swapping `a` and `b` in the comparison. Swapping `a` and `b` works for a single sort key, but breaks for multiple sort keys (the first key should be descending, the second ascending — swapping would reverse both).

The pattern `ordering.reverse()` flips the `Ordering` enum: `Less` becomes `Greater` and vice versa. `Equal` stays `Equal`. This correctly applies the direction to each key independently.

</details>

---
