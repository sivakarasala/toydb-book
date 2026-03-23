## Rust Gym

Time for reps. These drills focus on testing and benchmarking — the spotlight concepts for this chapter.

### Drill 1: Property Tests for SQL Parser (Medium)

Write property tests that verify the SQL parser handles all valid integer literals correctly.

```rust
// Simulated parser for demonstration
fn parse_integer(input: &str) -> Result<i64, String> {
    input.trim().parse::<i64>()
        .map_err(|e| format!("invalid integer '{}': {}", input, e))
}

fn parse_select_integer(sql: &str) -> Result<i64, String> {
    let sql = sql.trim();
    if !sql.to_uppercase().starts_with("SELECT ") {
        return Err("expected SELECT".to_string());
    }
    let expr = &sql[7..].trim();
    parse_integer(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Write property tests that verify:
    // 1. Any i64 can be parsed as a SELECT expression
    // 2. Parsing preserves the value exactly
    // 3. Invalid inputs return Err (never panic)
    // 4. Whitespace around the number is tolerated

    #[test]
    fn test_basic_integers() {
        assert_eq!(parse_select_integer("SELECT 42").unwrap(), 42);
        assert_eq!(parse_select_integer("SELECT -1").unwrap(), -1);
        assert_eq!(parse_select_integer("SELECT 0").unwrap(), 0);
    }
}

fn main() {
    assert_eq!(parse_select_integer("SELECT 42").unwrap(), 42);
    assert_eq!(parse_select_integer("SELECT -1").unwrap(), -1);
    assert_eq!(parse_select_integer("SELECT 0").unwrap(), 0);
    assert!(parse_select_integer("SELECT abc").is_err());
    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
fn parse_integer(input: &str) -> Result<i64, String> {
    input.trim().parse::<i64>()
        .map_err(|e| format!("invalid integer '{}': {}", input, e))
}

fn parse_select_integer(sql: &str) -> Result<i64, String> {
    let sql = sql.trim();
    if !sql.to_uppercase().starts_with("SELECT ") {
        return Err("expected SELECT".to_string());
    }
    let expr = &sql[7..].trim();
    parse_integer(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_integers() {
        assert_eq!(parse_select_integer("SELECT 42").unwrap(), 42);
        assert_eq!(parse_select_integer("SELECT -1").unwrap(), -1);
        assert_eq!(parse_select_integer("SELECT 0").unwrap(), 0);
    }

    #[test]
    fn test_all_i64_boundaries() {
        assert_eq!(
            parse_select_integer(&format!("SELECT {}", i64::MAX)).unwrap(),
            i64::MAX,
        );
        assert_eq!(
            parse_select_integer(&format!("SELECT {}", i64::MIN)).unwrap(),
            i64::MIN,
        );
    }

    #[test]
    fn test_property_roundtrip() {
        // Test a range of values
        for n in -1000..=1000 {
            let sql = format!("SELECT {}", n);
            let result = parse_select_integer(&sql).unwrap();
            assert_eq!(result, n, "Failed for n={}", n);
        }
    }

    #[test]
    fn test_property_whitespace_tolerance() {
        for n in [0, 1, -1, 42, -999, i64::MAX, i64::MIN] {
            let padded = format!("SELECT   {}  ", n);
            let result = parse_select_integer(&padded).unwrap();
            assert_eq!(result, n, "Whitespace tolerance failed for n={}", n);
        }
    }

    #[test]
    fn test_property_invalid_never_panics() {
        let invalid_inputs = vec![
            "", "SELECT", "SELECT ", "SELECT abc",
            "SELECT 1.5", "SELECT 99999999999999999999999",
            "INSERT 42", "SELECT 1 2",
        ];
        for input in invalid_inputs {
            let _ = parse_select_integer(input); // should not panic
        }
    }
}

fn main() {
    assert_eq!(parse_select_integer("SELECT 42").unwrap(), 42);
    assert_eq!(parse_select_integer("SELECT -1").unwrap(), -1);
    assert_eq!(parse_select_integer("SELECT 0").unwrap(), 0);
    assert!(parse_select_integer("SELECT abc").is_err());

    // Run the property tests inline
    for n in -100..=100 {
        let sql = format!("SELECT {}", n);
        assert_eq!(parse_select_integer(&sql).unwrap(), n);
    }

    println!("All checks passed!");
}
```

Without the `proptest` crate available in a standalone example, we use manual loops to simulate property testing. In a real project, you would use `proptest!` with `any::<i64>()` to generate truly random values. The key insight is the same: instead of testing specific examples, we test a *property* ("parsing an integer literal always produces the original value") across many inputs.

</details>

### Drill 2: Benchmark Storage Operations (Medium)

Build a simple benchmark harness that measures operations per second for different storage operations.

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct BenchResult {
    operation: String,
    total_ops: usize,
    elapsed: Duration,
}

impl BenchResult {
    fn ops_per_second(&self) -> f64 {
        self.total_ops as f64 / self.elapsed.as_secs_f64()
    }

    fn display(&self) -> String {
        format!(
            "{}: {} ops in {:?} ({:.0} ops/sec)",
            self.operation, self.total_ops, self.elapsed, self.ops_per_second()
        )
    }
}

fn bench<F>(name: &str, iterations: usize, mut f: F) -> BenchResult
where
    F: FnMut(usize),
{
    // TODO: run the function `iterations` times and measure total time
    todo!()
}

fn main() {
    let mut map: HashMap<String, String> = HashMap::new();

    // Benchmark inserts
    let insert_result = bench("HashMap insert", 100_000, |i| {
        map.insert(format!("key-{}", i), format!("value-{}", i));
    });
    println!("{}", insert_result.display());

    // Benchmark reads (on populated map)
    let read_result = bench("HashMap get", 100_000, |i| {
        let key = format!("key-{}", i % map.len());
        std::hint::black_box(map.get(&key));
    });
    println!("{}", read_result.display());

    // Benchmark misses
    let miss_result = bench("HashMap miss", 100_000, |i| {
        let key = format!("missing-{}", i);
        std::hint::black_box(map.get(&key));
    });
    println!("{}", miss_result.display());

    assert!(insert_result.ops_per_second() > 1000.0);
    assert!(read_result.ops_per_second() > 1000.0);
    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct BenchResult {
    operation: String,
    total_ops: usize,
    elapsed: Duration,
}

impl BenchResult {
    fn ops_per_second(&self) -> f64 {
        self.total_ops as f64 / self.elapsed.as_secs_f64()
    }

    fn display(&self) -> String {
        format!(
            "{}: {} ops in {:?} ({:.0} ops/sec)",
            self.operation, self.total_ops, self.elapsed, self.ops_per_second()
        )
    }
}

fn bench<F>(name: &str, iterations: usize, mut f: F) -> BenchResult
where
    F: FnMut(usize),
{
    let start = Instant::now();
    for i in 0..iterations {
        f(i);
    }
    let elapsed = start.elapsed();

    BenchResult {
        operation: name.to_string(),
        total_ops: iterations,
        elapsed,
    }
}

fn main() {
    let mut map: HashMap<String, String> = HashMap::new();

    let insert_result = bench("HashMap insert", 100_000, |i| {
        map.insert(format!("key-{}", i), format!("value-{}", i));
    });
    println!("{}", insert_result.display());

    let read_result = bench("HashMap get", 100_000, |i| {
        let key = format!("key-{}", i % map.len());
        std::hint::black_box(map.get(&key));
    });
    println!("{}", read_result.display());

    let miss_result = bench("HashMap miss", 100_000, |i| {
        let key = format!("missing-{}", i);
        std::hint::black_box(map.get(&key));
    });
    println!("{}", miss_result.display());

    assert!(insert_result.ops_per_second() > 1000.0);
    assert!(read_result.ops_per_second() > 1000.0);
    println!("All checks passed!");
}
```

The `std::hint::black_box()` function prevents the compiler from optimizing away the read. Without it, the compiler might notice that we never use the return value of `map.get()` and remove the call entirely — making the benchmark measure nothing. `black_box` tells the compiler "pretend this value is used" without actually doing anything at runtime. This is the same technique `criterion` uses internally.

Note: this simple harness measures wall-clock time for all iterations. For serious benchmarking, use `criterion`, which runs multiple rounds, warms up the CPU cache, computes statistics, and handles outliers.

</details>

### Drill 3: Chaos Testing with Simulated Failures (Hard)

Build a key-value store with replication that survives random node failures.

```rust
use std::collections::HashMap;

#[derive(Clone)]
struct ReplicaNode {
    id: usize,
    data: HashMap<String, String>,
    alive: bool,
}

impl ReplicaNode {
    fn new(id: usize) -> Self {
        ReplicaNode {
            id,
            data: HashMap::new(),
            alive: true,
        }
    }

    fn set(&mut self, key: &str, value: &str) -> bool {
        if !self.alive { return false; }
        self.data.insert(key.to_string(), value.to_string());
        true
    }

    fn get(&self, key: &str) -> Option<String> {
        if !self.alive { return None; }
        self.data.get(key).cloned()
    }

    fn crash(&mut self) {
        self.alive = false;
        self.data.clear(); // lose all state
    }
}

struct ReplicatedStore {
    nodes: Vec<ReplicaNode>,
}

impl ReplicatedStore {
    fn new(replica_count: usize) -> Self {
        // TODO
        todo!()
    }

    /// Write to a majority of nodes. Returns true if successful.
    fn set(&mut self, key: &str, value: &str) -> bool {
        // TODO: write to all alive nodes, succeed if majority acknowledges
        todo!()
    }

    /// Read from any alive node.
    fn get(&self, key: &str) -> Option<String> {
        // TODO: return value from first alive node that has it
        todo!()
    }

    /// Crash a specific node.
    fn crash_node(&mut self, id: usize) {
        // TODO
        todo!()
    }

    fn alive_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.alive).count()
    }
}

fn main() {
    let mut store = ReplicatedStore::new(3);

    // Write some data
    assert!(store.set("a", "1"));
    assert!(store.set("b", "2"));

    // Verify reads work
    assert_eq!(store.get("a"), Some("1".to_string()));

    // Crash one node — still have majority
    store.crash_node(0);
    assert_eq!(store.alive_count(), 2);
    assert!(store.set("c", "3")); // should succeed (2 of 3 = majority)
    assert_eq!(store.get("c"), Some("3".to_string()));

    // Crash another — lost majority
    store.crash_node(1);
    assert_eq!(store.alive_count(), 1);
    assert!(!store.set("d", "4")); // should fail (1 of 3 != majority)

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

#[derive(Clone)]
struct ReplicaNode {
    id: usize,
    data: HashMap<String, String>,
    alive: bool,
}

impl ReplicaNode {
    fn new(id: usize) -> Self {
        ReplicaNode {
            id,
            data: HashMap::new(),
            alive: true,
        }
    }

    fn set(&mut self, key: &str, value: &str) -> bool {
        if !self.alive { return false; }
        self.data.insert(key.to_string(), value.to_string());
        true
    }

    fn get(&self, key: &str) -> Option<String> {
        if !self.alive { return None; }
        self.data.get(key).cloned()
    }

    fn crash(&mut self) {
        self.alive = false;
        self.data.clear();
    }
}

struct ReplicatedStore {
    nodes: Vec<ReplicaNode>,
}

impl ReplicatedStore {
    fn new(replica_count: usize) -> Self {
        let nodes = (0..replica_count)
            .map(|id| ReplicaNode::new(id))
            .collect();
        ReplicatedStore { nodes }
    }

    fn set(&mut self, key: &str, value: &str) -> bool {
        let majority = self.nodes.len() / 2 + 1;
        let mut ack_count = 0;

        for node in &mut self.nodes {
            if node.set(key, value) {
                ack_count += 1;
            }
        }

        ack_count >= majority
    }

    fn get(&self, key: &str) -> Option<String> {
        for node in &self.nodes {
            if let Some(value) = node.get(key) {
                return Some(value);
            }
        }
        None
    }

    fn crash_node(&mut self, id: usize) {
        self.nodes[id].crash();
    }

    fn alive_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.alive).count()
    }
}

fn main() {
    let mut store = ReplicatedStore::new(3);

    assert!(store.set("a", "1"));
    assert!(store.set("b", "2"));

    assert_eq!(store.get("a"), Some("1".to_string()));

    store.crash_node(0);
    assert_eq!(store.alive_count(), 2);
    assert!(store.set("c", "3"));
    assert_eq!(store.get("c"), Some("3".to_string()));

    store.crash_node(1);
    assert_eq!(store.alive_count(), 1);
    assert!(!store.set("d", "4"));

    println!("All checks passed!");
}
```

This is a simplified version of quorum replication. In a real system, reads would also require a quorum (read from a majority) to guarantee linearizability. Our `get` reads from the first alive node, which could return stale data if that node missed a recent write. Raft solves this by directing all reads through the leader, who is guaranteed to have all committed writes.

</details>

### Drill 4: Golden Test Runner (Medium)

Build a golden test runner for a simple calculator language.

```rust
use std::collections::HashMap;

/// Simple expression evaluator.
fn evaluate(expr: &str) -> Result<f64, String> {
    let expr = expr.trim();

    // Try parsing as a number
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(n);
    }

    // Try parsing as "a op b"
    for op in [" + ", " - ", " * ", " / "] {
        if let Some(pos) = expr.rfind(op) {
            let left = evaluate(&expr[..pos])?;
            let right = evaluate(&expr[pos + op.len()..])?;
            return match op.trim() {
                "+" => Ok(left + right),
                "-" => Ok(left - right),
                "*" => Ok(left * right),
                "/" => {
                    if right == 0.0 {
                        Err("division by zero".to_string())
                    } else {
                        Ok(left / right)
                    }
                }
                _ => Err(format!("unknown operator: {}", op)),
            };
        }
    }

    Err(format!("cannot evaluate: '{}'", expr))
}

/// Run a golden test from inline data.
fn run_golden(test_name: &str, input: &str, expected: &str) {
    // TODO: evaluate each line of input, build actual output,
    // compare with expected
    todo!()
}

fn main() {
    let input = "1 + 2\n3 * 4\n10 / 3\n5 - 8\n1 / 0";
    let expected = "1 + 2 = 3\n3 * 4 = 12\n10 / 3 = 3.3333333333333335\n5 - 8 = -3\n1 / 0 = ERROR: division by zero\n";

    run_golden("basic_math", input, expected);

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
fn evaluate(expr: &str) -> Result<f64, String> {
    let expr = expr.trim();

    if let Ok(n) = expr.parse::<f64>() {
        return Ok(n);
    }

    for op in [" + ", " - ", " * ", " / "] {
        if let Some(pos) = expr.rfind(op) {
            let left = evaluate(&expr[..pos])?;
            let right = evaluate(&expr[pos + op.len()..])?;
            return match op.trim() {
                "+" => Ok(left + right),
                "-" => Ok(left - right),
                "*" => Ok(left * right),
                "/" => {
                    if right == 0.0 {
                        Err("division by zero".to_string())
                    } else {
                        Ok(left / right)
                    }
                }
                _ => Err(format!("unknown operator: {}", op)),
            };
        }
    }

    Err(format!("cannot evaluate: '{}'", expr))
}

fn run_golden(test_name: &str, input: &str, expected: &str) {
    let mut actual = String::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match evaluate(line) {
            Ok(result) => {
                actual.push_str(&format!("{} = {}\n", line, result));
            }
            Err(e) => {
                actual.push_str(&format!("{} = ERROR: {}\n", line, e));
            }
        }
    }

    if actual != expected {
        eprintln!("Golden test '{}' FAILED!", test_name);
        eprintln!("Expected:\n{}", expected);
        eprintln!("Actual:\n{}", actual);

        // Show first difference
        for (i, (a, b)) in actual.lines().zip(expected.lines()).enumerate() {
            if a != b {
                eprintln!("First difference at line {}:", i + 1);
                eprintln!("  Expected: {}", b);
                eprintln!("  Actual:   {}", a);
                break;
            }
        }

        panic!("golden test failed");
    }

    println!("Golden test '{}' passed.", test_name);
}

fn main() {
    let input = "1 + 2\n3 * 4\n10 / 3\n5 - 8\n1 / 0";
    let expected = "1 + 2 = 3\n3 * 4 = 12\n10 / 3 = 3.3333333333333335\n5 - 8 = -3\n1 / 0 = ERROR: division by zero\n";

    run_golden("basic_math", input, expected);

    println!("All checks passed!");
}
```

The golden test pattern is deceptively simple: run the code, format the output, compare against a saved file. But it scales beautifully — adding a new test case is as simple as adding a line to the input file. And when the behavior changes intentionally, you update the expected file once rather than updating dozens of `assert_eq!` statements.

</details>

---
