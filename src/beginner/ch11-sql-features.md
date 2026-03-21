# Chapter 11: SQL Features -- Joins, Aggregations, GROUP BY

Your executor can scan tables, filter rows, and project columns. That covers a surprising amount of SQL. But it misses the operations that make relational databases *relational*: combining data from multiple tables, and summarizing groups of rows. Without joins, every query reads from a single table -- you cannot answer "which users placed which orders?" Without aggregations, you cannot answer "how many users are over 30?" Without ORDER BY, results come back in whatever order they were stored.

This chapter extends the executor with four new operators. Each one is another struct implementing the `Executor` trait, composing with the operators you already have. By the end, your database will handle queries that combine data from multiple tables, group rows, compute totals, and sort results.

By the end of this chapter, you will have:

- A `NestedLoopJoinExecutor` that combines rows from two tables -- O(n * m)
- A `HashJoinExecutor` that uses a hash table for faster joins -- O(n + m)
- An `AggregateExecutor` with GROUP BY and five aggregation functions (COUNT, SUM, AVG, MIN, MAX)
- A `SortExecutor` that collects all rows and sorts them with a custom comparator
- A deep understanding of HashMap, the Entry API, Vec sorting, and the Ord trait

---

## Spotlight: Collections & Algorithms

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **collections and algorithms** -- the standard library data structures that power joins, aggregations, and sorting.

### HashMap: finding things fast

A `HashMap` stores key-value pairs and lets you look up a value by its key in O(1) average time. Think of a phone book: given a name (key), you can quickly find the phone number (value).

```rust
use std::collections::HashMap;

fn main() {
    let mut ages: HashMap<String, i32> = HashMap::new();

    // Insert key-value pairs
    ages.insert("Alice".to_string(), 30);
    ages.insert("Bob".to_string(), 25);
    ages.insert("Carol".to_string(), 35);

    // Look up a value by key
    if let Some(age) = ages.get("Alice") {
        println!("Alice is {} years old", age);  // "Alice is 30 years old"
    }

    // Check if a key exists
    println!("{}", ages.contains_key("Dave"));  // false
}
```

For our database, HashMap is critical in two places:

1. **Hash joins**: Build a HashMap from one table's join column. For each row in the other table, look up the matching rows in O(1) instead of scanning the entire first table.

2. **GROUP BY**: Use a HashMap where the key is the group (e.g., department name) and the value is the accumulated result (count, sum, etc.).

### The Entry API: insert or update in one step

When processing GROUP BY, each row either starts a new group or updates an existing one. The naive approach is awkward:

```rust,ignore
// Without Entry API -- clunky
if let Some(count) = map.get_mut("engineering") {
    *count += 1;
} else {
    map.insert("engineering".to_string(), 1);
}
```

The Entry API does this in one step:

```rust
use std::collections::HashMap;

fn main() {
    let mut word_count: HashMap<String, i32> = HashMap::new();
    let words = vec!["hello", "world", "hello", "rust", "hello"];

    for word in words {
        // entry() returns an Entry -- either Occupied or Vacant
        // or_insert(0) sets the value to 0 if the key is new
        // The * dereferences the mutable reference to add 1
        *word_count.entry(word.to_string()).or_insert(0) += 1;
    }

    println!("{:?}", word_count);
    // {"hello": 3, "world": 1, "rust": 1}
}
```

Let us break down `*word_count.entry(word).or_insert(0) += 1`:

1. `word_count.entry(word)` -- look up the key. Returns an `Entry` enum.
2. `.or_insert(0)` -- if the key is not in the map, insert it with value 0. Either way, return a mutable reference to the value.
3. `*... += 1` -- dereference the mutable reference and add 1.

This pattern is essential for aggregation. When processing GROUP BY, each row either starts a new group (Vacant entry) or updates an existing one (Occupied entry).

> **What just happened?**
>
> The Entry API solves the "get or insert" problem in one step. Without it, you need to look up the key twice -- once to check if it exists, and once to insert or update. The Entry API does one lookup and gives you a mutable reference to work with. Think of it like checking in to a hotel: if your room is ready (Occupied), you go there directly. If not (Vacant), the front desk assigns you one and then you go there.

### Vec and sorting with custom comparators

`Vec::sort_by` lets you sort using any comparison logic you want:

```rust
fn main() {
    let mut people = vec![
        ("Alice", 30),
        ("Bob", 25),
        ("Carol", 35),
    ];

    // Sort by age (ascending)
    people.sort_by(|a, b| a.1.cmp(&b.1));
    println!("{:?}", people);
    // [("Bob", 25), ("Alice", 30), ("Carol", 35)]

    // Sort by age (descending)
    people.sort_by(|a, b| b.1.cmp(&a.1));
    println!("{:?}", people);
    // [("Carol", 35), ("Alice", 30), ("Bob", 25)]
}
```

The closure `|a, b| a.1.cmp(&b.1)` takes two elements and returns an `Ordering`:
- `Ordering::Less` -- `a` should come before `b`
- `Ordering::Equal` -- `a` and `b` are equivalent
- `Ordering::Greater` -- `a` should come after `b`

The `.cmp()` method is from the `Ord` trait. For descending order, swap `a` and `b`: `b.1.cmp(&a.1)`.

### Ord and PartialOrd: making types sortable

To sort values, Rust needs to know how to compare them. This is done through two traits:

- **`PartialOrd`** -- types that can sometimes be compared (floating-point numbers cannot always be compared because `NaN != NaN`)
- **`Ord`** -- types that can always be compared (integers, strings, etc.)

For our `Value` enum, we need to define comparison:

```rust,ignore
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            (Value::Null, _) => Some(std::cmp::Ordering::Less),  // NULL sorts first
            (_, Value::Null) => Some(std::cmp::Ordering::Greater),
            _ => None,  // Different types cannot be compared
        }
    }
}
```

The `partial_cmp` returns `Option<Ordering>` because some comparisons are not possible (e.g., comparing an Integer to a String). When it returns `None`, the comparison is undefined.

We put NULLs first (before all other values). This is how PostgreSQL handles NULLs in ascending sorts.

> **What just happened?**
>
> `PartialOrd` tells Rust how to compare two values of our `Value` type. Integers are compared as numbers, strings alphabetically, booleans as false < true, and NULLs always sort first. Mixed types (like comparing an Integer to a String) return `None`, meaning they cannot be compared.

---

## Exercise 1: Implement NestedLoopJoinExecutor

**Goal:** Build a join operator that combines rows from two tables by checking every pair. This is the simplest join algorithm.

### Step 1: Understand what a join does

A JOIN combines rows from two tables based on a condition. Think of it like matching students to their test scores using the student ID. You have one table of students and another table of scores. The join matches each student with their corresponding score.

```sql
SELECT users.name, orders.item
FROM users
JOIN orders ON users.id = orders.user_id;
```

For each user, we find all orders where the user's ID matches the order's user_id. If Alice has ID 1 and there are two orders with user_id 1, Alice appears twice in the output (once for each order).

The nested loop join is the brute-force approach:

```
For each row in the left table (users):
    For each row in the right table (orders):
        If the join condition is true (users.id = orders.user_id):
            Output the combined row (all columns from both tables)
```

If the left table has `n` rows and the right has `m` rows, this checks `n * m` pairs. Simple but potentially slow for large tables.

### Step 2: Design the state machine

The tricky part is maintaining state across `next()` calls. In the Volcano model, we cannot use a simple nested for loop because `next()` must return one row at a time and resume where it left off.

We need to track:
- Which left row we are currently joining
- How far through the right table we have gotten for that left row

```rust
// src/executor.rs (continued)

/// Joins two executors using nested loops.
///
/// For each row in the left (outer) executor, scans all rows
/// in the right (inner) executor. Emits combined rows where
/// the join predicate is true.
///
/// The right side is materialized (loaded into memory) because
/// we need to re-scan it for each left row.
pub struct NestedLoopJoinExecutor {
    /// The outer (left) executor.
    left: Box<dyn Executor>,
    /// All rows from the right side (loaded into memory).
    right_rows: Vec<Row>,
    /// Column names for the right side.
    right_columns: Vec<String>,
    /// The join condition (e.g., users.id = orders.user_id).
    predicate: Expression,
    /// Column names for the combined output: left cols + right cols.
    combined_columns: Vec<String>,
    /// The current left row we are joining against.
    current_left: Option<Row>,
    /// Our position in right_rows for the current left row.
    right_position: usize,
}
```

Why do we store `right_rows` as a `Vec<Row>` instead of keeping the right executor? Because for each left row, we need to scan the entire right side from the beginning. An executor can only go forward -- there is no "rewind." So we materialize (load into memory) the right side once and re-scan our in-memory copy for each left row.

### Step 3: Build the constructor

```rust
impl NestedLoopJoinExecutor {
    pub fn new(
        left: Box<dyn Executor>,
        mut right: Box<dyn Executor>,
        predicate: Expression,
    ) -> Result<Self, ExecutorError> {
        // Materialize the right side -- pull all rows into memory.
        let right_columns = right.columns().to_vec();
        let mut right_rows = Vec::new();
        while let Some(row) = right.next()? {
            right_rows.push(row);
        }

        // Build combined column names: left columns, then right columns
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
```

The `while let Some(row) = right.next()?` pattern is a convenient way to pull all rows from an executor. It keeps calling `next()` until it gets `None`.

The `.chain()` method connects two iterators end-to-end. `left_cols.iter().chain(right_cols.iter())` produces all left column names followed by all right column names.

### Step 4: Implement the Executor trait

```rust
impl Executor for NestedLoopJoinExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        loop {
            // If we do not have a current left row, pull one
            if self.current_left.is_none() {
                match self.left.next()? {
                    None => return Ok(None),  // Left exhausted -- join is done
                    Some(row) => {
                        self.current_left = Some(row);
                        self.right_position = 0;  // Start from beginning of right
                    }
                }
            }

            // We have a left row -- scan through right rows
            let left_row = self.current_left.as_ref().unwrap();

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

                // Evaluate the join predicate
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

            // Right side exhausted for this left row.
            // Move to the next left row.
            self.current_left = None;
        }
    }

    fn columns(&self) -> &[String] {
        &self.combined_columns
    }
}
```

This is a state machine disguised as a loop. Let us trace through it:

1. First call: `current_left` is `None`. Pull a row from the left executor (say Alice). Set `right_position` to 0.
2. Check right_rows[0] against Alice. If the predicate matches, return the combined row. If not, increment `right_position` and try the next right row.
3. When all right rows are checked for Alice, set `current_left` to `None`. The outer loop pulls the next left row (Bob).
4. Reset `right_position` to 0. Check all right rows against Bob.
5. Continue until the left executor is exhausted.

> **What just happened?**
>
> The nested loop join is a double loop "unrolled" across multiple `next()` calls. The `current_left` and `right_position` fields are the loop variables, preserved between calls. Each call to `next()` resumes exactly where the previous call left off. This is how the Volcano model turns imperative loops into a pull-based iterator.

### Step 5: Test the join

```rust
#[cfg(test)]
mod join_tests {
    use super::*;

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

        // Alice (id=1) matches order 101 (Widget) and 103 (Doohickey)
        let r1 = join.next().unwrap().unwrap();
        assert_eq!(r1.values[1], Value::String("Alice".to_string()));
        assert_eq!(r1.values[4], Value::String("Widget".to_string()));

        let r2 = join.next().unwrap().unwrap();
        assert_eq!(r2.values[1], Value::String("Alice".to_string()));
        assert_eq!(r2.values[4], Value::String("Doohickey".to_string()));

        // Bob (id=2) matches order 102 (Gadget)
        let r3 = join.next().unwrap().unwrap();
        assert_eq!(r3.values[1], Value::String("Bob".to_string()));
        assert_eq!(r3.values[4], Value::String("Gadget".to_string()));

        // Carol (id=3) has no matching orders -- she does not appear
        assert_eq!(join.next().unwrap(), None);
    }
}
```

```
$ cargo test test_nested_loop_join
running 1 test
test executor::join_tests::test_nested_loop_join ... ok

test result: ok. 1 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Forgetting to reset `right_position`**: When you move to the next left row, you must set `right_position` back to 0. Otherwise, the second left row starts scanning right rows from where the first left row stopped.
>
> 2. **Column name collisions**: If both tables have an "id" column, `ColumnRef("id")` matches the first one found. For production code, you would qualify names with table prefixes. For now, use distinct column names in tests.

<details>
<summary>Hint: If the join produces wrong results</summary>

Print the combined columns to see the full schema: `println!("{:?}", join.columns())`. This shows you the order of columns in the combined row. If the output is `["id", "name", "order_id", "user_id", "item"]`, then index 0 is "id", index 1 is "name", index 2 is "order_id", etc. Make sure your test assertions use the correct indices.

</details>

---

## Exercise 2: Implement HashJoinExecutor

**Goal:** Build a faster join algorithm using a hash table. Instead of checking every pair (O(n * m)), build a HashMap from one table and look up matches in O(1).

### Step 1: Understand hash joins

The nested loop join checks every pair of rows. If the left table has 1,000 rows and the right has 1,000, that is 1,000,000 comparisons. A hash join reduces this dramatically:

1. **Build phase**: Read all rows from the smaller table. For each row, compute the join key (e.g., `user_id`) and store the row in a HashMap keyed by that value.
2. **Probe phase**: Read rows from the larger table one at a time. For each row, compute the join key, look it up in the HashMap. If found, output the combined row.

This is O(n + m) instead of O(n * m). For 1,000 rows on each side, that is 2,000 operations instead of 1,000,000.

Think of it like looking up someone's phone number. The nested loop approach is reading every entry in the phone book for each lookup. The hash join approach is building an index (HashMap) first, then doing instant lookups.

### Step 2: Define the struct

```rust
// src/executor.rs (continued)

/// Joins two executors using a hash table.
///
/// Build phase: reads all rows from the right executor and stores
/// them in a HashMap keyed by the join column value.
///
/// Probe phase: for each row from the left executor, looks up
/// matching right rows in the HashMap.
///
/// This is O(n + m) instead of O(n * m) for nested loops.
pub struct HashJoinExecutor {
    /// The left (probe) executor.
    left: Box<dyn Executor>,
    /// Right rows grouped by join key value.
    hash_table: HashMap<String, Vec<Row>>,
    /// Column names for the right side.
    right_columns: Vec<String>,
    /// The left column to join on.
    left_key: String,
    /// Combined column names.
    combined_columns: Vec<String>,
    /// Current left row being joined.
    current_left: Option<Row>,
    /// Matching right rows for the current left row.
    current_matches: Vec<Row>,
    /// Position within current_matches.
    match_position: usize,
}
```

### Step 3: Build the constructor

```rust
impl HashJoinExecutor {
    pub fn new(
        left: Box<dyn Executor>,
        mut right: Box<dyn Executor>,
        left_key: String,
        right_key: String,
    ) -> Result<Self, ExecutorError> {
        let right_columns = right.columns().to_vec();

        // Find the right key column index
        let right_key_index = right_columns.iter()
            .position(|c| c == &right_key)
            .ok_or_else(|| ExecutorError::ColumnNotFound(right_key.clone()))?;

        // Build phase: read all right rows into a HashMap
        let mut hash_table: HashMap<String, Vec<Row>> = HashMap::new();
        while let Some(row) = right.next()? {
            // Use the join key value as the HashMap key.
            // We convert to String for simplicity (a production DB
            // would use a proper hash of the Value).
            let key = format!("{}", row.values[right_key_index]);
            hash_table.entry(key).or_insert_with(Vec::new).push(row);
        }

        // Build combined columns
        let combined_columns: Vec<String> = left.columns().iter()
            .chain(right_columns.iter())
            .cloned()
            .collect();

        Ok(HashJoinExecutor {
            left,
            hash_table,
            right_columns,
            left_key,
            combined_columns,
            current_left: None,
            current_matches: Vec::new(),
            match_position: 0,
        })
    }
}
```

The build phase uses `entry().or_insert_with(Vec::new).push(row)`:

1. `entry(key)` -- look up the key in the HashMap
2. `.or_insert_with(Vec::new)` -- if the key does not exist, create a new empty Vec
3. `.push(row)` -- add the row to the Vec

This groups right rows by their join key value. If multiple orders have the same `user_id`, they are all stored in the same Vec.

### Step 4: Implement the Executor trait

```rust
impl Executor for HashJoinExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        loop {
            // If we have matches to return, return the next one
            if self.match_position < self.current_matches.len() {
                let left_row = self.current_left.as_ref().unwrap();
                let right_row = &self.current_matches[self.match_position];
                self.match_position += 1;

                let combined = Row::new(
                    left_row.values.iter()
                        .chain(right_row.values.iter())
                        .cloned()
                        .collect()
                );

                return Ok(Some(combined));
            }

            // No more matches for the current left row.
            // Pull the next left row.
            match self.left.next()? {
                None => return Ok(None),  // Left exhausted
                Some(left_row) => {
                    // Find the left key column index
                    let left_key_index = self.left.columns().iter()
                        .position(|c| c == &self.left_key)
                        .ok_or_else(|| {
                            ExecutorError::ColumnNotFound(self.left_key.clone())
                        })?;

                    // Look up the join key in the hash table
                    let key = format!("{}", left_row.values[left_key_index]);
                    self.current_matches = self.hash_table
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    self.match_position = 0;
                    self.current_left = Some(left_row);
                }
            }
        }
    }

    fn columns(&self) -> &[String] {
        &self.combined_columns
    }
}
```

The probe phase works like this:

1. Pull a row from the left executor
2. Get the join key value (e.g., Alice's id = 1)
3. Look up `"1"` in the hash table -- get a Vec of matching right rows
4. Return combined rows one at a time from `current_matches`
5. When `current_matches` is exhausted, pull the next left row

The `.unwrap_or_default()` call is important: if the key is not in the hash table (no matching right rows), it returns an empty Vec. The left row simply has no matches and is skipped.

> **What just happened?**
>
> The hash join eliminates the inner loop. Instead of scanning all right rows for each left row (n * m comparisons), we build a hash table in O(m) and then do O(1) lookups for each left row, totaling O(n + m). The tradeoff: we use extra memory to store the hash table. This is a classic space-time tradeoff in computer science.

### Step 5: Test the hash join

```rust
#[cfg(test)]
mod hash_join_tests {
    use super::*;

    #[test]
    fn test_hash_join() {
        let storage = orders_storage();

        let left = ScanExecutor::new(&storage, "users").unwrap();
        let right = ScanExecutor::new(&storage, "orders").unwrap();

        let mut join = HashJoinExecutor::new(
            Box::new(left),
            Box::new(right),
            "id".to_string(),
            "user_id".to_string(),
        ).unwrap();

        // Same results as nested loop join, same order
        let r1 = join.next().unwrap().unwrap();
        assert_eq!(r1.values[1], Value::String("Alice".to_string()));
        assert_eq!(r1.values[4], Value::String("Widget".to_string()));

        let r2 = join.next().unwrap().unwrap();
        assert_eq!(r2.values[1], Value::String("Alice".to_string()));
        assert_eq!(r2.values[4], Value::String("Doohickey".to_string()));

        let r3 = join.next().unwrap().unwrap();
        assert_eq!(r3.values[1], Value::String("Bob".to_string()));
        assert_eq!(r3.values[4], Value::String("Gadget".to_string()));

        // Carol has no orders
        assert_eq!(join.next().unwrap(), None);
    }

    #[test]
    fn test_hash_join_no_matches() {
        let mut storage = Storage::new();
        storage.create_table("a", vec!["id".to_string()]);
        storage.insert_row("a", Row::new(vec![Value::Integer(1)])).unwrap();

        storage.create_table("b", vec!["aid".to_string()]);
        storage.insert_row("b", Row::new(vec![Value::Integer(99)])).unwrap();

        let left = ScanExecutor::new(&storage, "a").unwrap();
        let right = ScanExecutor::new(&storage, "b").unwrap();

        let mut join = HashJoinExecutor::new(
            Box::new(left),
            Box::new(right),
            "id".to_string(),
            "aid".to_string(),
        ).unwrap();

        // No matches
        assert_eq!(join.next().unwrap(), None);
    }
}
```

```
$ cargo test hash_join_tests
running 2 tests
test executor::hash_join_tests::test_hash_join ... ok
test executor::hash_join_tests::test_hash_join_no_matches ... ok

test result: ok. 2 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Using the wrong key format**: We convert values to strings with `format!("{}", value)` for HashMap keys. This means `Integer(1)` and `Float(1.0)` produce different keys ("1" vs "1.00"). A production database would handle type coercion, but string keys work for learning.
>
> 2. **Forgetting to clone matches**: `self.hash_table.get(&key).cloned()` returns `Option<Vec<Row>>`. The `.cloned()` creates a copy of the Vec. Without it, you would have a borrow conflict because `self` is already mutably borrowed by `next()`.

---

## Exercise 3: Implement AggregateExecutor

**Goal:** Build an aggregation operator that handles GROUP BY with COUNT, SUM, AVG, MIN, and MAX.

### Step 1: Understand aggregation

Aggregation takes many rows and produces summary values. For example:

```sql
SELECT department, COUNT(*), AVG(salary)
FROM employees
GROUP BY department;
```

This:
1. Groups rows by the `department` column
2. For each group, counts the rows and computes the average salary
3. Produces one output row per group

Think of it like sorting students into classrooms (groups), then counting how many students are in each classroom.

### Step 2: Define aggregation types

```rust
// src/executor.rs (continued)

/// The kind of aggregation to perform.
#[derive(Debug, Clone)]
pub enum AggregateFunction {
    /// Count the number of rows in the group.
    Count,
    /// Sum the values of an expression.
    Sum(Expression),
    /// Average the values of an expression.
    Avg(Expression),
    /// Find the minimum value of an expression.
    Min(Expression),
    /// Find the maximum value of an expression.
    Max(Expression),
}

/// Accumulator for tracking aggregation state.
///
/// As rows arrive, the accumulator updates its internal state.
/// When the group is complete, it produces the final result.
#[derive(Debug, Clone)]
struct Accumulator {
    count: i64,
    sum: f64,
    min: Option<Value>,
    max: Option<Value>,
}

impl Accumulator {
    fn new() -> Self {
        Accumulator {
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
        }
    }

    /// Update the accumulator with a new value.
    fn update(&mut self, value: &Value) {
        self.count += 1;

        // Update sum (for SUM and AVG)
        match value {
            Value::Integer(n) => self.sum += *n as f64,
            Value::Float(f) => self.sum += f,
            _ => {} // Non-numeric values: sum is not meaningful
        }

        // Update min
        match (&self.min, value) {
            (None, v) => self.min = Some(v.clone()),
            (Some(current), v) => {
                if let Some(std::cmp::Ordering::Greater) = partial_cmp_values(current, v) {
                    self.min = Some(v.clone());
                }
            }
        }

        // Update max
        match (&self.max, value) {
            (None, v) => self.max = Some(v.clone()),
            (Some(current), v) => {
                if let Some(std::cmp::Ordering::Less) = partial_cmp_values(current, v) {
                    self.max = Some(v.clone());
                }
            }
        }
    }

    /// Get the result for a specific aggregation function.
    fn result(&self, func: &AggregateFunction) -> Value {
        match func {
            AggregateFunction::Count => Value::Integer(self.count),
            AggregateFunction::Sum(_) => {
                if self.count == 0 { Value::Null } else { Value::Float(self.sum) }
            }
            AggregateFunction::Avg(_) => {
                if self.count == 0 {
                    Value::Null
                } else {
                    Value::Float(self.sum / self.count as f64)
                }
            }
            AggregateFunction::Min(_) => {
                self.min.clone().unwrap_or(Value::Null)
            }
            AggregateFunction::Max(_) => {
                self.max.clone().unwrap_or(Value::Null)
            }
        }
    }
}

/// Compare two Values for ordering.
fn partial_cmp_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x.partial_cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::String(x), Value::String(y)) => x.partial_cmp(y),
        (Value::Integer(x), Value::Float(y)) => (*x as f64).partial_cmp(y),
        (Value::Float(x), Value::Integer(y)) => x.partial_cmp(&(*y as f64)),
        _ => None,
    }
}
```

The Accumulator is like a tally counter. As each row arrives, it updates:
- `count` -- always increments by 1
- `sum` -- adds the numeric value
- `min` -- keeps the smallest value seen
- `max` -- keeps the largest value seen

When the group is complete, calling `result()` produces the final answer.

### Step 3: Build the AggregateExecutor

```rust
/// Aggregation executor with GROUP BY support.
///
/// Reads ALL rows from its source (eager, not lazy!),
/// groups them, computes aggregates, and then yields
/// one result row per group.
pub struct AggregateExecutor {
    /// Pre-computed result rows (aggregation requires reading all input first).
    result_rows: Vec<Row>,
    /// Position in result_rows.
    position: usize,
    /// Output column names.
    output_columns: Vec<String>,
}

impl AggregateExecutor {
    pub fn new(
        mut source: Box<dyn Executor>,
        group_by: Vec<String>,
        aggregates: Vec<(String, AggregateFunction)>,
    ) -> Result<Self, ExecutorError> {
        let source_columns = source.columns().to_vec();

        // Collect all rows from the source
        let mut all_rows = Vec::new();
        while let Some(row) = source.next()? {
            all_rows.push(row);
        }

        // Group rows by the group-by columns
        let mut groups: HashMap<String, (Vec<Value>, Vec<Accumulator>)> = HashMap::new();

        for row in &all_rows {
            // Build the group key from the group-by column values
            let group_key: String = group_by.iter()
                .map(|col| {
                    let idx = source_columns.iter()
                        .position(|c| c == col)
                        .unwrap_or(0);
                    format!("{}", row.values[idx])
                })
                .collect::<Vec<_>>()
                .join("|");

            // Get or create the group entry
            let entry = groups.entry(group_key).or_insert_with(|| {
                // Store the group-by values for the output row
                let group_values: Vec<Value> = group_by.iter()
                    .map(|col| {
                        let idx = source_columns.iter()
                            .position(|c| c == col)
                            .unwrap_or(0);
                        row.values[idx].clone()
                    })
                    .collect();

                // Create one accumulator per aggregate function
                let accumulators = aggregates.iter()
                    .map(|_| Accumulator::new())
                    .collect();

                (group_values, accumulators)
            });

            // Update each accumulator with the current row's value
            for (i, (_, func)) in aggregates.iter().enumerate() {
                let value = match func {
                    AggregateFunction::Count => Value::Integer(1),
                    AggregateFunction::Sum(expr)
                    | AggregateFunction::Avg(expr)
                    | AggregateFunction::Min(expr)
                    | AggregateFunction::Max(expr) => {
                        evaluate(expr, row, &source_columns)?
                    }
                };
                entry.1[i].update(&value);
            }
        }

        // Handle no groups (no GROUP BY -- aggregate the entire table)
        if group_by.is_empty() && groups.is_empty() {
            // Produce one row with default aggregates (COUNT=0, SUM=NULL, etc.)
            let group_key = String::new();
            let accumulators = aggregates.iter()
                .map(|_| Accumulator::new())
                .collect();
            groups.insert(group_key, (vec![], accumulators));
        }

        // Build result rows
        let mut result_rows = Vec::new();
        for (_, (group_values, accumulators)) in &groups {
            let mut values = group_values.clone();
            for (i, (_, func)) in aggregates.iter().enumerate() {
                values.push(accumulators[i].result(func));
            }
            result_rows.push(Row::new(values));
        }

        // Build output column names
        let mut output_columns: Vec<String> = group_by.clone();
        for (name, _) in &aggregates {
            output_columns.push(name.clone());
        }

        Ok(AggregateExecutor {
            result_rows,
            position: 0,
            output_columns,
        })
    }
}

impl Executor for AggregateExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        if self.position >= self.result_rows.len() {
            return Ok(None);
        }
        let row = self.result_rows[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.output_columns
    }
}
```

This is an **eager** executor -- it reads all input rows before producing any output. This is necessary because you cannot compute an average until you have seen all the values. Unlike Scan, Filter, and Project (which are lazy), aggregation must materialize the entire input.

> **What just happened?**
>
> The AggregateExecutor reads all input rows, groups them by the GROUP BY columns, and computes aggregates for each group. The result is stored as a Vec of pre-computed rows. When `next()` is called, it simply walks through this pre-computed list. This is different from our other executors, which compute results on the fly. Aggregation requires seeing all data before it can produce any output.

### Step 4: Test aggregation

```rust
#[cfg(test)]
mod aggregate_tests {
    use super::*;

    fn employees_storage() -> Storage {
        let mut storage = Storage::new();
        storage.create_table("employees", vec![
            "name".to_string(),
            "department".to_string(),
            "salary".to_string(),
        ]);
        storage.insert_row("employees", Row::new(vec![
            Value::String("Alice".to_string()),
            Value::String("Engineering".to_string()),
            Value::Integer(90000),
        ])).unwrap();
        storage.insert_row("employees", Row::new(vec![
            Value::String("Bob".to_string()),
            Value::String("Engineering".to_string()),
            Value::Integer(85000),
        ])).unwrap();
        storage.insert_row("employees", Row::new(vec![
            Value::String("Carol".to_string()),
            Value::String("Sales".to_string()),
            Value::Integer(75000),
        ])).unwrap();
        storage.insert_row("employees", Row::new(vec![
            Value::String("Dave".to_string()),
            Value::String("Sales".to_string()),
            Value::Integer(70000),
        ])).unwrap();
        storage.insert_row("employees", Row::new(vec![
            Value::String("Eve".to_string()),
            Value::String("Engineering".to_string()),
            Value::Integer(95000),
        ])).unwrap();
        storage
    }

    #[test]
    fn test_count_all() {
        let storage = employees_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        // SELECT COUNT(*) FROM employees
        let mut agg = AggregateExecutor::new(
            Box::new(scan),
            vec![],  // no GROUP BY
            vec![("count".to_string(), AggregateFunction::Count)],
        ).unwrap();

        let row = agg.next().unwrap().unwrap();
        assert_eq!(row.values[0], Value::Integer(5));
        assert_eq!(agg.next().unwrap(), None);
    }

    #[test]
    fn test_group_by_count() {
        let storage = employees_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        // SELECT department, COUNT(*) FROM employees GROUP BY department
        let mut agg = AggregateExecutor::new(
            Box::new(scan),
            vec!["department".to_string()],
            vec![("count".to_string(), AggregateFunction::Count)],
        ).unwrap();

        // Collect all rows (order may vary because HashMap is unordered)
        let mut rows = Vec::new();
        while let Some(row) = agg.next().unwrap() {
            rows.push(row);
        }

        assert_eq!(rows.len(), 2);

        // Find the Engineering row
        let eng = rows.iter()
            .find(|r| r.values[0] == Value::String("Engineering".to_string()))
            .unwrap();
        assert_eq!(eng.values[1], Value::Integer(3));

        // Find the Sales row
        let sales = rows.iter()
            .find(|r| r.values[0] == Value::String("Sales".to_string()))
            .unwrap();
        assert_eq!(sales.values[1], Value::Integer(2));
    }

    #[test]
    fn test_group_by_avg() {
        let storage = employees_storage();
        let scan = ScanExecutor::new(&storage, "employees").unwrap();

        // SELECT department, AVG(salary) FROM employees GROUP BY department
        let mut agg = AggregateExecutor::new(
            Box::new(scan),
            vec!["department".to_string()],
            vec![(
                "avg_salary".to_string(),
                AggregateFunction::Avg(
                    Expression::ColumnRef("salary".to_string()),
                ),
            )],
        ).unwrap();

        let mut rows = Vec::new();
        while let Some(row) = agg.next().unwrap() {
            rows.push(row);
        }

        // Engineering avg: (90000 + 85000 + 95000) / 3 = 90000.0
        let eng = rows.iter()
            .find(|r| r.values[0] == Value::String("Engineering".to_string()))
            .unwrap();
        assert_eq!(eng.values[1], Value::Float(90000.0));

        // Sales avg: (75000 + 70000) / 2 = 72500.0
        let sales = rows.iter()
            .find(|r| r.values[0] == Value::String("Sales".to_string()))
            .unwrap();
        assert_eq!(sales.values[1], Value::Float(72500.0));
    }
}
```

```
$ cargo test aggregate_tests
running 3 tests
test executor::aggregate_tests::test_count_all ... ok
test executor::aggregate_tests::test_group_by_count ... ok
test executor::aggregate_tests::test_group_by_avg ... ok

test result: ok. 3 passed; 0 failed
```

> **Common Mistakes**
>
> 1. **Expecting ordered output**: HashMap does not guarantee order. The Engineering group might come before or after Sales. Use `.find()` in tests to locate specific groups instead of assuming order.
>
> 2. **Forgetting the no-group case**: `SELECT COUNT(*) FROM users` (no GROUP BY) should still produce one row. Without the special case for empty `group_by`, you would get zero rows.

---

## Exercise 4: Implement SortExecutor

**Goal:** Build a sort operator that reads all rows and returns them in sorted order.

### Step 1: Design the SortExecutor

Like aggregation, sorting is eager -- you must see all rows before you can produce any output in sorted order.

```rust
// src/executor.rs (continued)

/// Sort direction.
#[derive(Debug, Clone)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// A sort key: which expression to sort by, and in which direction.
#[derive(Debug, Clone)]
pub struct SortKey {
    pub expression: Expression,
    pub direction: SortDirection,
}

/// Sorts all rows from its source by the given sort keys.
///
/// This is an eager executor: it reads all input rows, sorts them,
/// and then yields them in sorted order.
pub struct SortExecutor {
    /// Pre-sorted rows.
    sorted_rows: Vec<Row>,
    /// Position in sorted_rows.
    position: usize,
    /// Column names (same as source -- sorting does not change schema).
    column_names: Vec<String>,
}

impl SortExecutor {
    pub fn new(
        mut source: Box<dyn Executor>,
        sort_keys: Vec<SortKey>,
    ) -> Result<Self, ExecutorError> {
        let column_names = source.columns().to_vec();

        // Read all rows from the source
        let mut rows = Vec::new();
        while let Some(row) = source.next()? {
            rows.push(row);
        }

        // Sort the rows
        // We need to handle potential errors from evaluate(),
        // so we sort with a closure that captures the column names.
        let cols = column_names.clone();
        rows.sort_by(|a, b| {
            for key in &sort_keys {
                // Evaluate the sort expression for both rows
                let a_val = evaluate(&key.expression, a, &cols)
                    .unwrap_or(Value::Null);
                let b_val = evaluate(&key.expression, b, &cols)
                    .unwrap_or(Value::Null);

                // Compare the values
                let ordering = match partial_cmp_values(&a_val, &b_val) {
                    Some(ord) => ord,
                    None => std::cmp::Ordering::Equal,
                };

                // Apply sort direction
                let ordering = match key.direction {
                    SortDirection::Ascending => ordering,
                    SortDirection::Descending => ordering.reverse(),
                };

                // If not equal, this key determines the order
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
                // If equal, try the next sort key
            }
            std::cmp::Ordering::Equal
        });

        Ok(SortExecutor {
            sorted_rows: rows,
            position: 0,
            column_names,
        })
    }
}

impl Executor for SortExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        if self.position >= self.sorted_rows.len() {
            return Ok(None);
        }
        let row = self.sorted_rows[self.position].clone();
        self.position += 1;
        Ok(Some(row))
    }

    fn columns(&self) -> &[String] {
        &self.column_names
    }
}
```

Key points about the sort:

1. **Multiple sort keys**: `ORDER BY department ASC, salary DESC` uses two keys. If two rows have the same department, the salary breaks the tie.

2. **`.sort_by(|a, b| ...)`**: Takes a closure that compares two rows. The closure returns `Ordering::Less`, `Equal`, or `Greater`.

3. **`.reverse()`**: Flips `Less` to `Greater` and vice versa. This is how we implement descending order.

4. **Error handling**: We use `unwrap_or(Value::Null)` inside the sort closure because `sort_by` expects the closure to be infallible (no `Result`). In a production database, you would validate sort expressions before sorting.

> **What just happened?**
>
> The SortExecutor reads all rows into memory, sorts them using `Vec::sort_by` with a custom comparator, and then yields them in order. The comparator evaluates sort expressions against each row and compares the results. Multiple sort keys are handled by trying each key in order -- the first non-equal comparison determines the row order.

### Step 2: Test the SortExecutor

```rust
#[cfg(test)]
mod sort_tests {
    use super::*;

    #[test]
    fn test_sort_ascending() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Sort by age ascending
        let mut sort = SortExecutor::new(
            Box::new(scan),
            vec![SortKey {
                expression: Expression::ColumnRef("age".to_string()),
                direction: SortDirection::Ascending,
            }],
        ).unwrap();

        let r1 = sort.next().unwrap().unwrap();
        assert_eq!(r1.values[2], Value::Integer(25));  // Bob

        let r2 = sort.next().unwrap().unwrap();
        assert_eq!(r2.values[2], Value::Integer(28));  // Dave

        let r3 = sort.next().unwrap().unwrap();
        assert_eq!(r3.values[2], Value::Integer(30));  // Alice

        let r4 = sort.next().unwrap().unwrap();
        assert_eq!(r4.values[2], Value::Integer(35));  // Carol

        assert_eq!(sort.next().unwrap(), None);
    }

    #[test]
    fn test_sort_descending() {
        let storage = sample_storage();
        let scan = ScanExecutor::new(&storage, "users").unwrap();

        // Sort by age descending
        let mut sort = SortExecutor::new(
            Box::new(scan),
            vec![SortKey {
                expression: Expression::ColumnRef("age".to_string()),
                direction: SortDirection::Descending,
            }],
        ).unwrap();

        let r1 = sort.next().unwrap().unwrap();
        assert_eq!(r1.values[2], Value::Integer(35));  // Carol

        let r2 = sort.next().unwrap().unwrap();
        assert_eq!(r2.values[2], Value::Integer(30));  // Alice

        let r3 = sort.next().unwrap().unwrap();
        assert_eq!(r3.values[2], Value::Integer(28));  // Dave

        let r4 = sort.next().unwrap().unwrap();
        assert_eq!(r4.values[2], Value::Integer(25));  // Bob

        assert_eq!(sort.next().unwrap(), None);
    }

    #[test]
    fn test_sort_empty() {
        let storage = Storage::new();
        storage.create_table("empty", vec!["id".to_string()]);

        let scan = ScanExecutor::new(&storage, "empty").unwrap();
        let mut sort = SortExecutor::new(
            Box::new(scan),
            vec![SortKey {
                expression: Expression::ColumnRef("id".to_string()),
                direction: SortDirection::Ascending,
            }],
        ).unwrap();

        assert_eq!(sort.next().unwrap(), None);
    }
}
```

```
$ cargo test sort_tests
running 3 tests
test executor::sort_tests::test_sort_ascending ... ok
test executor::sort_tests::test_sort_descending ... ok
test executor::sort_tests::test_sort_empty ... ok

test result: ok. 3 passed; 0 failed
```

---

## Exercise 5: Chaining Join + Aggregate + Sort (Challenge)

**Goal:** Execute a complex query that combines a join, aggregation, and sorting:

```sql
SELECT department, COUNT(*)
FROM users JOIN departments ON users.dept_id = departments.id
GROUP BY department
ORDER BY COUNT(*) DESC;
```

This is a stretch exercise. Build the executor chain step by step.

<details>
<summary>Hint 1: The executor chain</summary>

```
SortExecutor (by count, descending)
  AggregateExecutor (GROUP BY department, COUNT)
    ProjectExecutor (keep just the department column)
      HashJoinExecutor (users.dept_id = departments.id)
        ScanExecutor (users)
        ScanExecutor (departments)
```

Each executor wraps the one below it, forming a pipeline.

</details>

<details>
<summary>Hint 2: Building the chain in code</summary>

```rust
// 1. Scan both tables
let users_scan = ScanExecutor::new(&storage, "users").unwrap();
let depts_scan = ScanExecutor::new(&storage, "departments").unwrap();

// 2. Join them
let join = HashJoinExecutor::new(
    Box::new(users_scan),
    Box::new(depts_scan),
    "dept_id".to_string(),
    "id".to_string(),
).unwrap();

// 3. Aggregate
let agg = AggregateExecutor::new(
    Box::new(join),
    vec!["department".to_string()],
    vec![("count".to_string(), AggregateFunction::Count)],
).unwrap();

// 4. Sort
let mut sort = SortExecutor::new(
    Box::new(agg),
    vec![SortKey {
        expression: Expression::ColumnRef("count".to_string()),
        direction: SortDirection::Descending,
    }],
).unwrap();

// 5. Pull results
while let Some(row) = sort.next().unwrap() {
    println!("{}", row);
}
```

</details>

---

## What We Built

In this chapter, you extended the executor with four powerful operators:

1. **NestedLoopJoinExecutor** -- combines rows from two tables using a double loop (O(n * m))
2. **HashJoinExecutor** -- combines rows using a hash table for O(n + m) performance
3. **AggregateExecutor** -- groups rows and computes COUNT, SUM, AVG, MIN, MAX
4. **SortExecutor** -- sorts all rows by arbitrary expressions

The Rust concepts you learned:

- **HashMap** -- O(1) key-value lookup, essential for hash joins and grouping
- **Entry API** -- `entry().or_insert()` for ergonomic insert-or-update patterns
- **Vec::sort_by** -- sorting with custom comparators using closures
- **PartialOrd** -- defining how custom types are compared
- **Iterator::chain** -- combining two iterators end-to-end
- **State machines in iterators** -- maintaining loop state across `next()` calls (the join executor pattern)
- **Eager vs lazy execution** -- aggregation and sorting must see all data; scan, filter, and project can be lazy

Your database now handles a wide range of SQL: single-table queries, joins across tables, aggregations with grouping, and sorted output. In the next chapter, we make it accessible over the network.

---

## Key Takeaways

1. **Nested loop joins are simple but slow (O(n * m)).** Hash joins are fast (O(n + m)) but use more memory. The optimizer should choose the right algorithm based on table sizes.

2. **The Entry API eliminates the get-then-insert pattern.** For GROUP BY, `entry(key).or_insert_with(|| default).update(row)` handles both new groups and existing groups in one expression.

3. **Aggregation and sorting are eager operators.** Unlike Scan, Filter, and Project (which produce rows lazily), aggregation must read all input before producing any output. This has memory implications for large tables.

4. **Vec::sort_by with closures enables custom sort orders.** Multiple sort keys, ascending/descending, NULL handling -- all controlled by the comparator closure.

5. **Composition still works.** Every new operator implements the same `Executor` trait and can wrap any other executor. `Sort(Aggregate(Join(Scan, Scan)))` works just like `Project(Filter(Scan))`.
