# Chapter 18: Testing, Benchmarking & Extensions

You have built a distributed SQL database from scratch — seventeen chapters of storage engines, SQL parsing, query execution, MVCC transactions, client-server networking, and Raft consensus. But building software is only half the job. The other half is knowing whether it works, how fast it is, and where it breaks. This final chapter turns inward. You will learn how to test a system this complex with confidence, how to benchmark its performance to find bottlenecks, and where to take it next.

The spotlight concept is **testing and benchmarking** — Rust's built-in test framework, property-based testing with `proptest`, deterministic testing for distributed systems, and performance measurement with `criterion`. Rust makes testing a first-class citizen: tests live next to the code they verify, run with a single command, and the compiler catches entire categories of bugs before a test is ever written.

By the end of this chapter, you will have:

- A comprehensive testing strategy for each layer of the database
- Unit tests using `#[test]` with assertion macros
- Integration tests in the `tests/` directory for end-to-end verification
- Property-based tests using `proptest` that generate random inputs and verify invariants
- Deterministic tests for Raft that control time and network behavior
- Benchmarks using `criterion` to measure and track performance
- A review of everything you built and ideas for extending it further

---

## Spotlight: Testing & Benchmarking

Every chapter has one spotlight concept. This chapter's spotlight is **testing and benchmarking** — how Rust helps you verify correctness and measure performance.

### Unit tests: #[test] and assert macros

Rust's test framework is built into the language. Any function annotated with `#[test]` is a test:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_add_overflow() {
        let _ = add(i32::MAX, 1); // panics in debug mode
    }
}
```

Run them with `cargo test`. The `#[cfg(test)]` attribute means the `tests` module is only compiled when running tests — it does not appear in the release binary.

Key assertion macros:

| Macro | Purpose | Example |
|-------|---------|---------|
| `assert!(expr)` | Assert expression is true | `assert!(result.is_ok())` |
| `assert_eq!(a, b)` | Assert equality (requires `PartialEq` + `Debug`) | `assert_eq!(count, 42)` |
| `assert_ne!(a, b)` | Assert inequality | `assert_ne!(result, "")` |
| `assert!(expr, "msg")` | Assert with custom message | `assert!(x > 0, "x was {}", x)` |

When an assertion fails, Rust prints both the expected and actual values (thanks to `Debug`). This is why most types in your codebase should derive `Debug` — it makes test failures informative.

### Test organization

Rust has three levels of test organization:

**1. Unit tests** — inside the module they test, using `#[cfg(test)]`:

```rust,ignore
// src/sql/lexer.rs

pub fn tokenize(input: &str) -> Vec<Token> { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_select() {
        let tokens = tokenize("SELECT * FROM users");
        assert_eq!(tokens[0], Token::Keyword(Keyword::Select));
    }
}
```

Unit tests can access private functions (they are inside the module). This is intentional — you should test internal logic, not just the public API.

**2. Integration tests** — in the `tests/` directory, each file is a separate crate:

```
tests/
├── sql_integration.rs    // tests SQL end-to-end
├── storage_test.rs       // tests storage engine
└── raft_test.rs          // tests Raft consensus
```

Integration tests can only use the crate's public API. They compile as separate binaries, so they test your crate as an external consumer would use it.

**3. Doc tests** — code examples in documentation comments:

```rust
/// Parses a SQL string into a statement.
///
/// # Examples
///
/// ```
/// let stmt = toydb_sql::parse("SELECT 1 + 1");
/// assert!(stmt.is_ok());
/// ```
pub fn parse(sql: &str) -> Result<Statement, ParseError> { ... }
```

Doc tests run with `cargo test`. They serve double duty: documentation and test. If the example does not compile, the test fails.

### Testing async code

For testing async functions (like our Tokio-based server), use `#[tokio::test]`:

```rust,ignore
#[tokio::test]
async fn test_server_accepts_connection() {
    let server = TestServer::start().await;
    let mut client = Client::connect(&server.addr()).await.unwrap();

    let response = client.query("SELECT 1").await.unwrap();
    assert_eq!(response.rows[0][0], "1");

    server.shutdown().await;
}
```

The `#[tokio::test]` attribute creates a Tokio runtime for the test. Without it, you would need to manually create a runtime with `tokio::runtime::Runtime::new()`.

### Benchmarking with criterion

Rust's built-in benchmarks (`#[bench]`) are unstable (nightly only). The `criterion` crate provides stable, statistical benchmarks:

```rust,ignore
// benches/storage_bench.rs

use criterion::{criterion_group, criterion_main, Criterion};
use toydb_storage::MvccStorage;

fn bench_insert(c: &mut Criterion) {
    c.bench_function("mvcc_insert", |b| {
        let mut storage = MvccStorage::new_in_memory();
        let mut key_counter = 0u64;

        b.iter(|| {
            key_counter += 1;
            let key = format!("key-{}", key_counter);
            storage.set(&key, "value").unwrap();
        });
    });
}

fn bench_get(c: &mut Criterion) {
    let mut storage = MvccStorage::new_in_memory();

    // Pre-populate
    for i in 0..10_000 {
        storage.set(&format!("key-{}", i), "value").unwrap();
    }

    c.bench_function("mvcc_get", |b| {
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % 10_000;
            storage.get(&format!("key-{}", i)).unwrap();
        });
    });
}

criterion_group!(benches, bench_insert, bench_get);
criterion_main!(benches);
```

Run with `cargo bench`. Criterion runs the benchmark multiple times, computes statistics (mean, standard deviation, confidence intervals), and compares against previous runs to detect regressions.

### Property-based testing with proptest

Traditional tests verify specific examples. Property-based tests verify *properties* — statements that should hold for all inputs:

```rust,ignore
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_lexer_never_panics(input in "\\PC{0,1000}") {
        // The lexer should never panic, regardless of input.
        // It should return Ok or Err, never crash.
        let _ = Lexer::new(&input).tokenize();
    }

    #[test]
    fn test_roundtrip_serialization(
        term in 0u64..1000,
        index in 0u64..1000,
        command in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let entry = LogEntry { term, index, command: command.clone() };
        let bytes = entry.serialize();
        let recovered = LogEntry::deserialize(&bytes).unwrap();
        assert_eq!(recovered, entry);
    }
}
```

`proptest` generates random inputs and checks that the property holds. If it finds a failing input, it *shrinks* it — finding the smallest input that still fails. This produces minimal, understandable test cases.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Unit test | Jest / Mocha | pytest / unittest | `testing` package | `#[test]` (built-in) |
> | Test runner | `npm test` | `pytest` | `go test` | `cargo test` |
> | Assertion | `expect(x).toBe(y)` | `assert x == y` | `t.Errorf(...)` | `assert_eq!(x, y)` |
> | Test file convention | `*.test.js` | `test_*.py` | `*_test.go` | `tests/*.rs` or `#[cfg(test)]` |
> | Property testing | fast-check | hypothesis | rapid | proptest |
> | Benchmarking | benchmark.js | timeit / pytest-benchmark | `testing.B` | criterion |
> | Mocking | jest.mock | unittest.mock | gomock | mockall |
>
> Rust's key advantage: the compiler catches entire categories of bugs that other languages rely on tests to find. Null pointer dereferences, data races, use-after-free, buffer overflows — none of these can occur in safe Rust, so you never need to test for them. Your tests can focus on *logic* bugs instead of *memory* bugs.
>
> The tradeoff: Rust does not have a built-in mocking framework, and its strong type system makes mocking harder. You typically use traits (interfaces) for dependency injection and create test implementations rather than mock objects. This results in more code but more reliable tests.

---

## Testing Strategy: Layer by Layer

Each layer of the database has different testing needs. Here is the strategy:

### Layer 1: Storage Engine

**What to test:** Correctness of key-value operations, concurrency safety, persistence.

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_get_delete() {
        let mut store = MemoryStorage::new();

        store.set(b"key1", b"value1").unwrap();
        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        store.delete(b"key1").unwrap();
        assert_eq!(store.get(b"key1").unwrap(), None);
    }

    #[test]
    fn test_scan_range() {
        let mut store = MemoryStorage::new();

        store.set(b"a", b"1").unwrap();
        store.set(b"b", b"2").unwrap();
        store.set(b"c", b"3").unwrap();
        store.set(b"d", b"4").unwrap();

        let results: Vec<_> = store.scan(b"b"..b"d").collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, b"b");
        assert_eq!(results[1].0, b"c");
    }

    #[test]
    fn test_persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();

        {
            let mut store = BitCaskStorage::open(dir.path()).unwrap();
            store.set(b"key", b"value").unwrap();
        }

        {
            let store = BitCaskStorage::open(dir.path()).unwrap();
            assert_eq!(store.get(b"key").unwrap(), Some(b"value".to_vec()));
        }
    }
}
```

**Property tests for storage:**

```rust,ignore
proptest! {
    #[test]
    fn test_set_then_get_returns_value(
        key in prop::collection::vec(any::<u8>(), 1..100),
        value in prop::collection::vec(any::<u8>(), 0..1000),
    ) {
        let mut store = MemoryStorage::new();
        store.set(&key, &value).unwrap();
        let result = store.get(&key).unwrap();
        assert_eq!(result, Some(value));
    }

    #[test]
    fn test_delete_removes_key(
        key in prop::collection::vec(any::<u8>(), 1..100),
        value in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let mut store = MemoryStorage::new();
        store.set(&key, &value).unwrap();
        store.delete(&key).unwrap();
        assert_eq!(store.get(&key).unwrap(), None);
    }
}
```

### Layer 2: SQL Lexer

**What to test:** Token output for known inputs, handling of edge cases (empty input, unterminated strings, Unicode), error messages for invalid input.

```rust,ignore
#[test]
fn test_lex_select_star() {
    let tokens = Lexer::new("SELECT * FROM users").tokenize().unwrap();
    assert_eq!(tokens, vec![
        Token::Keyword(Keyword::Select),
        Token::Asterisk,
        Token::Keyword(Keyword::From),
        Token::Identifier("users".to_string()),
    ]);
}

#[test]
fn test_lex_string_with_escape() {
    let tokens = Lexer::new("'it''s'").tokenize().unwrap();
    assert_eq!(tokens, vec![
        Token::String("it's".to_string()),
    ]);
}

#[test]
fn test_lex_unterminated_string() {
    let result = Lexer::new("'unterminated").tokenize();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unterminated"));
}
```

**Property test: lexer never panics:**

```rust,ignore
proptest! {
    #[test]
    fn test_lexer_never_panics(input in "\\PC{0,500}") {
        // Should return Ok or Err, never panic
        let _ = Lexer::new(&input).tokenize();
    }

    #[test]
    fn test_lex_then_stringify_roundtrip(
        keyword in prop::sample::select(vec![
            "SELECT", "INSERT", "UPDATE", "DELETE",
            "FROM", "WHERE", "AND", "OR",
        ]),
    ) {
        let tokens = Lexer::new(keyword).tokenize().unwrap();
        assert!(!tokens.is_empty());
    }
}
```

### Layer 3: SQL Parser

**What to test:** AST output for known SQL, error handling for malformed SQL, precedence of operators.

```rust,ignore
#[test]
fn test_parse_select() {
    let ast = Parser::parse("SELECT id, name FROM users WHERE id > 5").unwrap();
    match ast {
        Statement::Select { columns, from, filter, .. } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(from, "users");
            assert!(filter.is_some());
        }
        _ => panic!("expected SELECT statement"),
    }
}

#[test]
fn test_parse_operator_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3), not (1 + 2) * 3
    let ast = Parser::parse("SELECT 1 + 2 * 3").unwrap();
    // Verify the AST structure reflects correct precedence
    match ast {
        Statement::Select { columns, .. } => {
            // The top-level expression should be Add
            match &columns[0] {
                Expression::BinaryOp { op: Op::Add, right, .. } => {
                    // The right side should be Multiply
                    match right.as_ref() {
                        Expression::BinaryOp { op: Op::Multiply, .. } => {}
                        other => panic!("expected Multiply, got {:?}", other),
                    }
                }
                other => panic!("expected Add, got {:?}", other),
            }
        }
        _ => panic!("expected SELECT"),
    }
}
```

### Layer 4: MVCC

**What to test:** Snapshot isolation, write conflicts, visibility rules.

```rust,ignore
#[test]
fn test_snapshot_isolation() {
    let mut storage = MvccStorage::new_in_memory();

    // Transaction 1: write a value
    let mut txn1 = storage.begin().unwrap();
    txn1.set(b"key", b"value1").unwrap();
    txn1.commit().unwrap();

    // Transaction 2: start (sees value1)
    let txn2 = storage.begin().unwrap();

    // Transaction 3: update the value
    let mut txn3 = storage.begin().unwrap();
    txn3.set(b"key", b"value2").unwrap();
    txn3.commit().unwrap();

    // Transaction 2 should still see value1 (snapshot isolation)
    assert_eq!(txn2.get(b"key").unwrap(), Some(b"value1".to_vec()));
}

#[test]
fn test_write_conflict() {
    let mut storage = MvccStorage::new_in_memory();

    // Both transactions write to the same key
    let mut txn1 = storage.begin().unwrap();
    let mut txn2 = storage.begin().unwrap();

    txn1.set(b"key", b"from-txn1").unwrap();
    txn2.set(b"key", b"from-txn2").unwrap();

    txn1.commit().unwrap();

    // txn2 should fail with a write conflict
    let result = txn2.commit();
    assert!(result.is_err());
}
```

### Layer 5: Raft

**What to test:** Leader election, log replication, safety properties. This is the hardest layer to test because it involves timing, network communication, and non-determinism.

---

## Deterministic Testing for Distributed Systems

Testing Raft is hard because it involves:
- **Time:** election timeouts, heartbeat intervals
- **Network:** message delivery, ordering, partitions
- **Concurrency:** multiple nodes running simultaneously

Real-world Raft implementations use **deterministic simulation** — controlling all sources of non-determinism so tests are repeatable.

### Step 1: Replace real time with a fake clock

```rust,ignore
// src/raft/testing.rs

use std::time::Duration;

/// A fake clock that only advances when told to.
/// Used in tests to control timing precisely.
pub struct FakeClock {
    now: u64, // milliseconds since epoch
}

impl FakeClock {
    pub fn new() -> Self {
        FakeClock { now: 0 }
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    /// Advance time by the given duration.
    pub fn advance(&mut self, duration: Duration) {
        self.now += duration.as_millis() as u64;
    }
}

/// Trait for things that need a clock.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.now
    }
}

/// Real clock for production use.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
```

### Step 2: Replace real network with a fake transport

```rust,ignore
// src/raft/testing.rs (continued)

use std::collections::VecDeque;

/// A message in the fake network.
#[derive(Debug, Clone)]
pub struct NetworkMessage {
    pub from: u64,
    pub to: u64,
    pub payload: Vec<u8>,
}

/// A fake network that queues messages instead of sending them
/// over TCP. Allows tests to control message delivery order,
/// drop messages, and introduce delays.
pub struct FakeNetwork {
    /// Queued messages, waiting to be delivered.
    queue: VecDeque<NetworkMessage>,
    /// Messages that have been dropped (for inspection).
    dropped: Vec<NetworkMessage>,
    /// If true, drop all messages to this node (simulate partition).
    partitioned: std::collections::HashSet<u64>,
}

impl FakeNetwork {
    pub fn new() -> Self {
        FakeNetwork {
            queue: VecDeque::new(),
            dropped: Vec::new(),
            partitioned: std::collections::HashSet::new(),
        }
    }

    /// Queue a message for delivery.
    pub fn send(&mut self, msg: NetworkMessage) {
        if self.partitioned.contains(&msg.to) {
            self.dropped.push(msg);
        } else {
            self.queue.push_back(msg);
        }
    }

    /// Deliver the next message. Returns None if the queue is empty.
    pub fn deliver_next(&mut self) -> Option<NetworkMessage> {
        self.queue.pop_front()
    }

    /// Deliver all queued messages.
    pub fn deliver_all(&mut self) -> Vec<NetworkMessage> {
        self.queue.drain(..).collect()
    }

    /// Partition a node — all messages to it are dropped.
    pub fn partition(&mut self, node_id: u64) {
        self.partitioned.insert(node_id);
    }

    /// Heal a partition — messages to this node are delivered again.
    pub fn heal(&mut self, node_id: u64) {
        self.partitioned.remove(&node_id);
    }

    /// Return the number of queued messages.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}
```

### Step 3: Build a test cluster

```rust,ignore
// src/raft/testing.rs (continued)

/// A test cluster with fake time and fake network.
pub struct TestCluster {
    nodes: Vec<RaftNode>,
    network: FakeNetwork,
    clock: FakeClock,
}

impl TestCluster {
    /// Create a cluster of N nodes.
    pub fn new(node_count: usize) -> Self {
        let mut nodes = Vec::new();
        for id in 1..=node_count {
            let node = RaftNode::new_for_testing(
                id as u64,
                node_count,
            );
            nodes.push(node);
        }

        TestCluster {
            nodes,
            network: FakeNetwork::new(),
            clock: FakeClock::new(),
        }
    }

    /// Advance time and process messages until the cluster is stable
    /// (no more messages to deliver and no timeouts firing).
    pub fn stabilize(&mut self) {
        for _ in 0..1000 { // safety limit
            // Deliver all queued messages
            let messages = self.network.deliver_all();
            if messages.is_empty() {
                break;
            }
            for msg in messages {
                let node = &mut self.nodes[msg.to as usize - 1];
                let responses = node.handle_message(msg.payload);
                for response in responses {
                    self.network.send(response);
                }
            }
        }
    }

    /// Trigger an election timeout on a specific node.
    pub fn trigger_election(&mut self, node_id: u64) {
        let node = &mut self.nodes[node_id as usize - 1];
        let messages = node.election_timeout();
        for msg in messages {
            self.network.send(msg);
        }
    }

    /// Find the current leader, if any.
    pub fn leader(&self) -> Option<u64> {
        self.nodes.iter()
            .find(|n| n.is_leader())
            .map(|n| n.id())
    }

    /// Propose a command to the leader.
    pub fn propose(&mut self, command: Vec<u8>) -> Result<(), String> {
        let leader_id = self.leader()
            .ok_or("no leader")?;
        let node = &mut self.nodes[leader_id as usize - 1];
        let messages = node.propose_command(command)?;
        for msg in messages {
            self.network.send(msg);
        }
        Ok(())
    }

    /// Get a reference to a specific node.
    pub fn node(&self, id: u64) -> &RaftNode {
        &self.nodes[id as usize - 1]
    }
}
```

### Step 4: Deterministic Raft tests

```rust,ignore
#[cfg(test)]
mod raft_tests {
    use super::*;

    #[test]
    fn test_leader_election_basic() {
        let mut cluster = TestCluster::new(3);

        // No leader initially
        assert!(cluster.leader().is_none());

        // Node 1 times out and starts an election
        cluster.trigger_election(1);
        cluster.stabilize();

        // Node 1 should be elected leader
        assert_eq!(cluster.leader(), Some(1));
        assert_eq!(cluster.node(1).current_term(), 1);
    }

    #[test]
    fn test_leader_election_with_partition() {
        let mut cluster = TestCluster::new(5);

        // Elect node 1 as leader
        cluster.trigger_election(1);
        cluster.stabilize();
        assert_eq!(cluster.leader(), Some(1));

        // Partition node 1 from the rest
        cluster.network.partition(1);

        // Node 2 should eventually become leader
        cluster.trigger_election(2);
        cluster.stabilize();

        // Node 2 is leader (nodes 2,3,4,5 form a majority)
        assert!(cluster.node(2).is_leader());
        assert!(cluster.node(2).current_term() > 1);
    }

    #[test]
    fn test_log_replication() {
        let mut cluster = TestCluster::new(3);

        // Elect leader
        cluster.trigger_election(1);
        cluster.stabilize();

        // Propose a command
        cluster.propose(b"SET x 1".to_vec()).unwrap();
        cluster.stabilize();

        // All nodes should have the entry
        for id in 1..=3 {
            let node = cluster.node(id);
            assert_eq!(node.log_length(), 1);
        }
    }

    #[test]
    fn test_committed_entries_survive_leader_change() {
        let mut cluster = TestCluster::new(3);

        // Elect node 1 and replicate some entries
        cluster.trigger_election(1);
        cluster.stabilize();

        cluster.propose(b"SET x 1".to_vec()).unwrap();
        cluster.propose(b"SET y 2".to_vec()).unwrap();
        cluster.stabilize();

        // Verify all nodes have the entries
        for id in 1..=3 {
            assert_eq!(cluster.node(id).log_length(), 2);
        }

        // Kill the leader (partition it)
        cluster.network.partition(1);

        // Elect a new leader
        cluster.trigger_election(2);
        cluster.stabilize();

        // New leader should have all committed entries
        let new_leader = cluster.leader().unwrap();
        assert_ne!(new_leader, 1);
        assert_eq!(cluster.node(new_leader).log_length(), 2);
    }
}
```

These tests are fully deterministic — no real time, no real network, no threads. They run in milliseconds and produce the same result every time. This is the gold standard for testing distributed protocols.

---

## Exercise 1: Property Tests for the SQL Parser

**Goal:** Write property-based tests that verify invariants of the SQL parser, using `proptest` to generate random but valid SQL fragments.

### Step 1: Generate valid SQL expressions

```rust,ignore
use proptest::prelude::*;

/// Strategy to generate valid SQL integer literals.
fn sql_integer() -> impl Strategy<Value = String> {
    (-1000i64..1000).prop_map(|n| n.to_string())
}

/// Strategy to generate valid SQL string literals.
fn sql_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{0,20}".prop_map(|s| format!("'{}'", s))
}

/// Strategy to generate valid SQL identifiers.
fn sql_identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,10}".prop_map(|s| s)
}

/// Strategy to generate valid SQL comparison expressions.
fn sql_comparison() -> impl Strategy<Value = String> {
    (sql_identifier(), prop::sample::select(vec![
        "=", "!=", "<", ">", "<=", ">="
    ]), sql_integer()).prop_map(|(col, op, val)| {
        format!("{} {} {}", col, op, val)
    })
}

/// Strategy to generate valid SELECT statements.
fn sql_select() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(sql_identifier(), 1..5),
        sql_identifier(),
        proptest::option::of(sql_comparison()),
    ).prop_map(|(cols, table, filter)| {
        let col_list = cols.join(", ");
        match filter {
            Some(f) => format!("SELECT {} FROM {} WHERE {}", col_list, table, f),
            None => format!("SELECT {} FROM {}", col_list, table),
        }
    })
}
```

### Step 2: Write property tests

```rust,ignore
proptest! {
    /// The parser should never panic on any input.
    #[test]
    fn parser_never_panics(input in "\\PC{0,200}") {
        let _ = Parser::parse(&input);
    }

    /// Valid SELECT statements should always parse successfully.
    #[test]
    fn valid_selects_parse(sql in sql_select()) {
        let result = Parser::parse(&sql);
        assert!(
            result.is_ok(),
            "Failed to parse valid SQL: {}\nError: {:?}",
            sql, result.unwrap_err()
        );
    }

    /// Parsing then converting back to SQL should produce a parseable result.
    #[test]
    fn parse_display_roundtrip(sql in sql_select()) {
        let ast = Parser::parse(&sql).unwrap();
        let regenerated = ast.to_sql(); // Statement::to_sql() produces SQL from AST
        let reparsed = Parser::parse(&regenerated);
        assert!(
            reparsed.is_ok(),
            "Roundtrip failed.\nOriginal: {}\nRegenerated: {}\nError: {:?}",
            sql, regenerated, reparsed.unwrap_err()
        );
    }

    /// Integer literals should parse to integer values.
    #[test]
    fn integer_literals_parse_correctly(n in -10000i64..10000) {
        let sql = format!("SELECT {}", n);
        let ast = Parser::parse(&sql).unwrap();
        match ast {
            Statement::Select { columns, .. } => {
                match &columns[0] {
                    Expression::Literal(Value::Integer(parsed)) => {
                        assert_eq!(*parsed, n);
                    }
                    other => panic!("Expected integer literal, got {:?}", other),
                }
            }
            _ => panic!("Expected SELECT"),
        }
    }
}
```

Property-based tests are powerful because they explore the input space far more broadly than hand-written examples. A human might write 10 test cases for the parser. `proptest` generates thousands of random inputs in seconds, finding edge cases you would never think of — empty strings, strings with only whitespace, very long identifiers, deeply nested expressions.

### Step 3: Test the complete roundtrip

```rust
/// Verifies that parsing and re-serializing preserves semantics.
fn roundtrip_check(sql: &str) {
    let ast1 = match Parser::parse(sql) {
        Ok(ast) => ast,
        Err(_) => return, // Skip unparseable inputs
    };

    let regenerated = ast1.to_sql();
    let ast2 = Parser::parse(&regenerated).unwrap_or_else(|e| {
        panic!(
            "Roundtrip produced unparseable SQL.\n\
             Input:       {}\n\
             Regenerated: {}\n\
             Error:       {}",
            sql, regenerated, e
        );
    });

    // ASTs should be semantically equivalent
    assert_eq!(
        format!("{:?}", ast1),
        format!("{:?}", ast2),
        "ASTs differ after roundtrip.\n\
         Input:       {}\n\
         Regenerated: {}",
        sql, regenerated
    );
}

#[test]
fn test_roundtrip_examples() {
    roundtrip_check("SELECT 1");
    roundtrip_check("SELECT a, b FROM t");
    roundtrip_check("SELECT * FROM users WHERE id = 1");
    roundtrip_check("SELECT a FROM t WHERE a > 1 AND b < 2");
    roundtrip_check("INSERT INTO t VALUES (1, 'hello')");
}
```

---

## Exercise 2: Benchmark Storage Engines

**Goal:** Benchmark the Memory and BitCask storage engines to compare their performance characteristics, using `criterion` for statistical rigor.

### Step 1: Set up criterion

Add to `Cargo.toml`:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "storage_bench"
harness = false
```

### Step 2: Write the benchmarks

```rust,ignore
// benches/storage_bench.rs

use criterion::{
    criterion_group, criterion_main,
    Criterion, BenchmarkId, Throughput,
};

/// Trait abstracting storage engines for benchmarking.
trait BenchStorage {
    fn new_instance() -> Self;
    fn set(&mut self, key: &[u8], value: &[u8]);
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn delete(&mut self, key: &[u8]);
}

/// In-memory storage (HashMap-based).
struct MemoryBench {
    data: std::collections::HashMap<Vec<u8>, Vec<u8>>,
}

impl BenchStorage for MemoryBench {
    fn new_instance() -> Self {
        MemoryBench {
            data: std::collections::HashMap::new(),
        }
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.data.insert(key.to_vec(), value.to_vec());
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    fn delete(&mut self, key: &[u8]) {
        self.data.remove(key);
    }
}

/// Simulated BitCask storage (append-only file + in-memory index).
struct BitCaskBench {
    index: std::collections::HashMap<Vec<u8>, usize>,
    data: Vec<(Vec<u8>, Vec<u8>)>,
}

impl BenchStorage for BitCaskBench {
    fn new_instance() -> Self {
        BitCaskBench {
            index: std::collections::HashMap::new(),
            data: Vec::new(),
        }
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        let offset = self.data.len();
        self.data.push((key.to_vec(), value.to_vec()));
        self.index.insert(key.to_vec(), offset);
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.index.get(key)
            .map(|&offset| self.data[offset].1.clone())
    }

    fn delete(&mut self, key: &[u8]) {
        self.index.remove(key);
    }
}

fn bench_sequential_writes<S: BenchStorage>(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_writes");

    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("Memory", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut store = MemoryBench::new_instance();
                    for i in 0..size {
                        let key = format!("key-{:010}", i);
                        store.set(key.as_bytes(), b"value-data-here");
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("BitCask", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut store = BitCaskBench::new_instance();
                    for i in 0..size {
                        let key = format!("key-{:010}", i);
                        store.set(key.as_bytes(), b"value-data-here");
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_random_reads<S: BenchStorage>(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_reads");
    let dataset_size = 10_000;

    // Pre-populate stores
    let mut memory = MemoryBench::new_instance();
    let mut bitcask = BitCaskBench::new_instance();

    for i in 0..dataset_size {
        let key = format!("key-{:010}", i);
        let value = format!("value-{}", i);
        memory.set(key.as_bytes(), value.as_bytes());
        bitcask.set(key.as_bytes(), value.as_bytes());
    }

    group.throughput(Throughput::Elements(1000));

    group.bench_function("Memory", |b| {
        let mut i = 0u64;
        b.iter(|| {
            for _ in 0..1000 {
                i = (i.wrapping_mul(6364136223846793005).wrapping_add(1))
                    % dataset_size;
                let key = format!("key-{:010}", i);
                criterion::black_box(memory.get(key.as_bytes()));
            }
        });
    });

    group.bench_function("BitCask", |b| {
        let mut i = 0u64;
        b.iter(|| {
            for _ in 0..1000 {
                i = (i.wrapping_mul(6364136223846793005).wrapping_add(1))
                    % dataset_size;
                let key = format!("key-{:010}", i);
                criterion::black_box(bitcask.get(key.as_bytes()));
            }
        });
    });

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_80read_20write");

    group.bench_function("Memory", |b| {
        let mut store = MemoryBench::new_instance();
        // Pre-populate with 1000 keys
        for i in 0..1000 {
            store.set(
                format!("key-{}", i).as_bytes(),
                b"initial-value",
            );
        }

        let mut op_counter = 0u64;
        b.iter(|| {
            op_counter += 1;
            if op_counter % 5 == 0 {
                // 20% writes
                let key = format!("key-{}", op_counter % 1000);
                store.set(key.as_bytes(), b"updated-value");
            } else {
                // 80% reads
                let key = format!("key-{}", op_counter % 1000);
                criterion::black_box(store.get(key.as_bytes()));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_writes::<MemoryBench>,
    bench_random_reads::<MemoryBench>,
    bench_mixed_workload,
);
criterion_main!(benches);
```

### Step 3: Interpret the results

After running `cargo bench`, criterion produces output like:

```
sequential_writes/Memory/10000
                        time:   [1.2345 ms 1.2567 ms 1.2789 ms]
                        thrpt:  [7.8193 Melem/s 7.9573 Melem/s 8.1000 Melem/s]

sequential_writes/BitCask/10000
                        time:   [2.3456 ms 2.3891 ms 2.4326 ms]
                        thrpt:  [4.1108 Melem/s 4.1856 Melem/s 4.2616 Melem/s]
```

The three numbers are the lower bound, estimate, and upper bound of a 95% confidence interval. Criterion runs the benchmark many times and uses statistical analysis to produce stable, reliable numbers. If the confidence interval is wide, the benchmark is noisy — try closing other programs or increasing the measurement time.

Key observations you will likely see:
- **Memory is faster for everything** — no disk I/O, no serialization overhead
- **BitCask writes are slower** — append to file + update index
- **BitCask reads are similar to Memory** — both are HashMap lookups (BitCask keeps an in-memory index)
- **The gap narrows with larger values** — the overhead of memory allocation dominates over storage engine overhead

---

## Exercise 3: Chaos Testing

**Goal:** Build a chaos testing framework that introduces random failures (network partitions, message drops, node crashes) and verifies that the system maintains its safety invariants.

### Step 1: Define safety invariants

Before injecting failures, we need to know what "correct" means. For our database, the invariants are:

1. **Agreement:** All non-crashed nodes that have committed an entry at index N have the same entry at index N.
2. **Durability:** A committed entry is never lost (assuming a majority of nodes survive).
3. **Linearizability:** Once a write is acknowledged, subsequent reads return that write (or a later one).
4. **Leader uniqueness:** At most one leader exists per term.

### Step 2: Build the chaos engine

```rust,ignore
// src/raft/chaos.rs

use rand::Rng;
use std::time::Duration;

/// Types of failures the chaos engine can inject.
#[derive(Debug, Clone)]
pub enum Fault {
    /// Drop a specific percentage of messages.
    MessageDrop { rate: f64 },
    /// Delay messages by a random duration.
    MessageDelay { min: Duration, max: Duration },
    /// Partition a node from the rest of the cluster.
    NetworkPartition { node_id: u64, duration: Duration },
    /// Crash a node (lose all volatile state).
    NodeCrash { node_id: u64 },
    /// Restart a previously crashed node.
    NodeRestart { node_id: u64 },
}

/// The chaos engine applies random faults to a test cluster.
pub struct ChaosEngine {
    faults_applied: Vec<Fault>,
    rng: rand::rngs::StdRng,
}

impl ChaosEngine {
    pub fn new(seed: u64) -> Self {
        use rand::SeedableRng;
        ChaosEngine {
            faults_applied: Vec::new(),
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    /// Generate a random fault.
    pub fn random_fault(&mut self, cluster_size: usize) -> Fault {
        let fault_type = self.rng.gen_range(0..5);
        match fault_type {
            0 => Fault::MessageDrop {
                rate: self.rng.gen_range(0.1..0.5),
            },
            1 => Fault::MessageDelay {
                min: Duration::from_millis(self.rng.gen_range(10..50)),
                max: Duration::from_millis(self.rng.gen_range(50..200)),
            },
            2 => Fault::NetworkPartition {
                node_id: self.rng.gen_range(1..=cluster_size) as u64,
                duration: Duration::from_millis(self.rng.gen_range(100..1000)),
            },
            3 => Fault::NodeCrash {
                node_id: self.rng.gen_range(1..=cluster_size) as u64,
            },
            _ => Fault::NodeRestart {
                node_id: self.rng.gen_range(1..=cluster_size) as u64,
            },
        }
    }

    pub fn faults_applied(&self) -> &[Fault] {
        &self.faults_applied
    }
}
```

### Step 3: Run chaos tests

```rust,ignore
#[test]
fn test_chaos_safety() {
    // Run with a fixed seed for reproducibility.
    // If this test fails, the seed tells you exactly how to reproduce.
    for seed in 0..100 {
        let mut cluster = TestCluster::new(5);
        let mut chaos = ChaosEngine::new(seed);

        // Elect initial leader
        cluster.trigger_election(1);
        cluster.stabilize();

        // Run 50 rounds of operations + faults
        let mut committed_values: Vec<Vec<u8>> = Vec::new();

        for round in 0..50 {
            // Maybe inject a fault
            if round % 3 == 0 {
                let fault = chaos.random_fault(5);
                apply_fault(&mut cluster, &fault);
            }

            // Try to propose a command
            let cmd = format!("SET key-{} value-{}", round, round);
            if cluster.propose(cmd.as_bytes().to_vec()).is_ok() {
                committed_values.push(cmd.into_bytes());
            }

            cluster.stabilize();
        }

        // Heal all partitions
        for id in 1..=5 {
            cluster.network.heal(id);
        }
        cluster.stabilize();

        // Verify safety: all alive nodes agree on committed entries
        verify_agreement(&cluster);
    }
}

fn apply_fault(cluster: &mut TestCluster, fault: &Fault) {
    match fault {
        Fault::NetworkPartition { node_id, .. } => {
            cluster.network.partition(*node_id);
        }
        Fault::NodeCrash { node_id } => {
            cluster.network.partition(*node_id);
            // In a full implementation, we would also clear the
            // node's volatile state
        }
        _ => {} // Other faults would be applied to the network layer
    }
}

fn verify_agreement(cluster: &TestCluster) {
    // Find the highest commit index across all nodes
    let max_commit = (1..=5)
        .map(|id| cluster.node(id as u64).commit_index())
        .max()
        .unwrap();

    // For each committed index, verify all nodes that have
    // committed it have the same entry
    for index in 1..=max_commit {
        let mut entries: Vec<(u64, &LogEntry)> = Vec::new();

        for id in 1..=5u64 {
            if let Some(entry) = cluster.node(id).log_entry(index) {
                if cluster.node(id).commit_index() >= index {
                    entries.push((id, entry));
                }
            }
        }

        // All committed entries at this index should be identical
        if entries.len() > 1 {
            let first = &entries[0].1;
            for (node_id, entry) in &entries[1..] {
                assert_eq!(
                    first.term, entry.term,
                    "Agreement violation at index {}: node {} has term {}, \
                     node {} has term {}",
                    index, entries[0].0, first.term, node_id, entry.term
                );
                assert_eq!(
                    first.command, entry.command,
                    "Agreement violation at index {}: different commands",
                    index
                );
            }
        }
    }
}
```

The key technique: **seeded randomness**. By using a fixed seed for the random number generator, the test is deterministic — the same seed always produces the same sequence of faults. If a test fails, you can reproduce it exactly by running with the same seed. This combines the breadth of random testing with the reproducibility of deterministic testing.

---

## Exercise 4: Golden Test Suite for SQL

**Goal:** Build a golden test suite that stores expected query results in files, making it easy to add new test cases and detect regressions.

### Step 1: The golden test pattern

A golden test compares actual output against a stored "golden" file. If they match, the test passes. If not, either the code has a bug (regression) or the expected behavior has changed (update the golden file).

```
tests/
└── golden/
    ├── basic_select.sql        ← input
    ├── basic_select.expected   ← expected output (the "golden" file)
    ├── aggregations.sql
    ├── aggregations.expected
    ├── joins.sql
    └── joins.expected
```

### Step 2: Implement the test runner

```rust,ignore
// tests/golden_tests.rs

use std::fs;
use std::path::Path;

/// Run a golden test: execute the SQL in the .sql file and compare
/// the output to the .expected file.
fn run_golden_test(test_name: &str) {
    let sql_path = format!("tests/golden/{}.sql", test_name);
    let expected_path = format!("tests/golden/{}.expected", test_name);

    let sql = fs::read_to_string(&sql_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {}", sql_path, e));

    let mut server = test_server();
    let mut actual_output = String::new();

    for line in sql.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("--") {
            continue; // skip blanks and comments
        }

        actual_output.push_str(&format!("> {}\n", line));

        let response = server.execute(line);
        match response {
            Response::Rows { columns, rows } => {
                actual_output.push_str(&format_table(&columns, &rows));
            }
            Response::Ok { message } => {
                actual_output.push_str(&format!("OK: {}\n", message));
            }
            Response::Error { message } => {
                actual_output.push_str(&format!("ERROR: {}\n", message));
            }
        }
        actual_output.push('\n');
    }

    if Path::new(&expected_path).exists() {
        let expected = fs::read_to_string(&expected_path).unwrap();
        if actual_output != expected {
            // Write the actual output for easy diffing
            let actual_path = format!("tests/golden/{}.actual", test_name);
            fs::write(&actual_path, &actual_output).unwrap();

            panic!(
                "Golden test '{}' failed.\n\
                 Expected: {}\n\
                 Actual:   {}\n\
                 Run `diff {} {}` to see differences.\n\
                 To update the golden file: cp {} {}",
                test_name, expected_path, actual_path,
                expected_path, actual_path,
                actual_path, expected_path,
            );
        }
    } else {
        // No expected file — create it (first run)
        fs::write(&expected_path, &actual_output).unwrap();
        println!(
            "Created golden file: {}\n\
             Review it and commit if correct.",
            expected_path,
        );
    }
}

fn format_table(columns: &[String], rows: &[Vec<String>]) -> String {
    let mut output = String::new();

    // Calculate column widths
    let widths: Vec<usize> = (0..columns.len())
        .map(|i| {
            let header_width = columns[i].len();
            let max_data_width = rows.iter()
                .map(|row| row.get(i).map(|s| s.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            header_width.max(max_data_width)
        })
        .collect();

    // Header
    for (i, col) in columns.iter().enumerate() {
        if i > 0 { output.push_str(" | "); }
        output.push_str(&format!("{:width$}", col, width = widths[i]));
    }
    output.push('\n');

    // Separator
    for (i, width) in widths.iter().enumerate() {
        if i > 0 { output.push_str("-+-"); }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');

    // Rows
    for row in rows {
        for (i, value) in row.iter().enumerate() {
            if i > 0 { output.push_str(" | "); }
            output.push_str(&format!("{:width$}", value, width = widths[i]));
        }
        output.push('\n');
    }

    output
}
```

### Step 3: Create test SQL files

```sql
-- tests/golden/basic_select.sql

-- Setup
CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)
INSERT INTO users VALUES (1, 'Alice', 30)
INSERT INTO users VALUES (2, 'Bob', 25)
INSERT INTO users VALUES (3, 'Charlie', 35)

-- Basic queries
SELECT * FROM users
SELECT name FROM users WHERE age > 28
SELECT * FROM users ORDER BY age
SELECT COUNT(*) FROM users
```

The expected output file would be:

```
-- tests/golden/basic_select.expected

> CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)
OK: Table 'users' created

> INSERT INTO users VALUES (1, 'Alice', 30)
OK: 1 row(s) affected

> INSERT INTO users VALUES (2, 'Bob', 25)
OK: 1 row(s) affected

> INSERT INTO users VALUES (3, 'Charlie', 35)
OK: 1 row(s) affected

> SELECT * FROM users
id | name    | age
---+---------+----
1  | Alice   | 30
2  | Bob     | 25
3  | Charlie | 35

> SELECT name FROM users WHERE age > 28
name
-------
Alice
Charlie

> SELECT * FROM users ORDER BY age
id | name    | age
---+---------+----
2  | Bob     | 25
1  | Alice   | 30
3  | Charlie | 35

> SELECT COUNT(*) FROM users
COUNT(*)
--------
3
```

### Step 4: Wire up the tests

```rust,ignore
#[test]
fn golden_basic_select() {
    run_golden_test("basic_select");
}

#[test]
fn golden_aggregations() {
    run_golden_test("aggregations");
}

#[test]
fn golden_joins() {
    run_golden_test("joins");
}

#[test]
fn golden_errors() {
    run_golden_test("errors");
}
```

Golden tests have a powerful property: they are easy to write (just write SQL) and easy to review (the expected output is human-readable). When you change the query engine's behavior, the golden tests immediately show you every query whose output changed — making it easy to verify that changes are intentional.

CockroachDB uses this pattern extensively — they have thousands of golden test files that verify SQL behavior. SQLite uses a similar approach with its "test harness" that runs SQL scripts and compares output.

---

## What We Built: The Complete Architecture

Let us step back and review what you have built across 18 chapters:

```
┌─────────────────────────────────────────────────────────────────┐
│                        toydb                                     │
│                                                                  │
│  Ch 12-13: Client/Server         Ch 6-11: SQL Engine             │
│  ┌──────────────────────┐       ┌──────────────────────────────┐ │
│  │ TCP Server (async)   │       │ Lexer     → Tokens           │ │
│  │ Wire Protocol        │       │ Parser    → AST              │ │
│  │ REPL Client          │       │ Planner   → Plan             │ │
│  └──────────┬───────────┘       │ Optimizer → Optimized Plan   │ │
│             │                   │ Executor  → Results          │ │
│             │                   └──────────────┬───────────────┘ │
│             │                                  │                 │
│             └────────────┬─────────────────────┘                 │
│                          │                                       │
│  Ch 14-16: Raft          │         Ch 1-5: Storage               │
│  ┌──────────────────┐    │        ┌─────────────────────────────┐│
│  │ Leader Election  │    │        │ Key-Value (HashMap)         ││
│  │ Log Replication  │◄───┤        │ BitCask (append-only disk)  ││
│  │ WAL + Recovery   │    │        │ MVCC (multi-version)        ││
│  │ Snapshots        │    └───────>│ Serialization               ││
│  └──────────────────┘             └─────────────────────────────┘│
│                                                                  │
│  Ch 17: Integration     Ch 18: Testing                           │
│  ┌──────────────────┐   ┌──────────────────────────────────────┐ │
│  │ Server struct     │   │ Unit + Integration + Property tests  │ │
│  │ Error propagation│   │ Deterministic distributed testing    │ │
│  │ Config + startup │   │ Benchmarking with criterion          │ │
│  │ Read/write paths │   │ Golden test suite                    │ │
│  └──────────────────┘   └──────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Chapter-by-chapter summary

| Chapter | What you built | Spotlight Rust concept |
|---------|---------------|----------------------|
| 1 | Key-value store with REPL | Variables, types, HashMap |
| 2 | In-memory storage engine | Traits, generics |
| 3 | BitCask disk storage | File I/O, serialization |
| 4 | Binary serialization | Bytes, endianness, encoding |
| 5 | MVCC transactions | Lifetimes, borrows |
| 6 | SQL lexer | Enums, pattern matching |
| 7 | SQL parser | Recursion, Box, tree structures |
| 8 | Query planner | From AST to execution plan |
| 9 | Query optimizer | Tree transformations |
| 10 | Query executor | Iterators, Volcano model |
| 11 | SQL features (JOIN, GROUP BY) | Collections, closures |
| 12 | Client-server protocol | Structs, networking |
| 13 | Async networking | async/await, Tokio |
| 14 | Raft leader election | State machines, timers |
| 15 | Raft log replication | Channels, message passing |
| 16 | Raft durability | Ownership, persistence |
| 17 | Full integration | Module system, workspace |
| 18 | Testing and benchmarks | Testing, benchmarking |

---

## Extension Ideas

Your database is complete but minimal. Here are directions you could take it:

### B-tree index

Replace the hash-based storage with a B-tree. This enables:
- Range queries (`WHERE age BETWEEN 20 AND 30`) without full table scans
- Ordered iteration (useful for `ORDER BY` without sorting)
- Secondary indexes on arbitrary columns

A B-tree index is the single most impactful feature for query performance. PostgreSQL, MySQL, and SQLite all use B-trees as their primary data structure.

### More SQL features

- **DISTINCT:** Remove duplicate rows from results
- **HAVING:** Filter after GROUP BY (vs WHERE which filters before)
- **Subqueries:** `SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)`
- **Window functions:** `SELECT name, RANK() OVER (ORDER BY score DESC) FROM players`
- **ALTER TABLE:** Add/drop columns, rename tables
- **CREATE INDEX:** Build secondary indexes

### WAL improvements

- **Segmented WAL:** Split the log into fixed-size segments for O(1) compaction
- **Group commit:** Batch multiple concurrent writes into a single fsync
- **Direct I/O:** Bypass the OS page cache for predictable latency
- **Compression:** Compress WAL entries to reduce disk usage

### Concurrency

- **Read-write locking:** Allow multiple concurrent readers
- **Optimistic concurrency control:** Retry instead of lock
- **Connection pooling:** Reuse TCP connections across queries
- **Parallel query execution:** Execute independent plan nodes concurrently

### Operations

- **Metrics:** Expose query latency, throughput, and error rates via Prometheus
- **Slow query log:** Log queries that exceed a time threshold
- **EXPLAIN:** Show the query plan without executing
- **Configuration reload:** Update settings without restarting

### Distributed features

- **Sharding:** Partition data across multiple Raft groups by key range
- **Multi-Raft:** Run multiple Raft groups on the same set of nodes
- **Cross-shard transactions:** Two-phase commit across Raft groups
- **Follower reads:** Serve reads from followers (sacrificing linearizability for throughput)

---

## Where to Go from Here

You have built a database from scratch. Not a toy — a real, working system with SQL, transactions, networking, and distributed consensus. Here is how to go deeper:

### Read the source code of real databases

- **toydb** by Erik Grinaker — the reference implementation this book is based on. Clean Rust code, well-documented. [github.com/erikgrinaker/toydb](https://github.com/erikgrinaker/toydb)
- **etcd/raft** — the most widely-used Raft implementation. Written in Go. Powers Kubernetes. [github.com/etcd-io/raft](https://github.com/etcd-io/raft)
- **SQLite** — the most widely-deployed database in the world. Written in C. Beautifully documented with extensive comments. [sqlite.org/src](https://sqlite.org/src)
- **CockroachDB** — a distributed SQL database written in Go. Uses Raft, SQL, MVCC — the same architecture you built. [github.com/cockroachdb/cockroach](https://github.com/cockroachdb/cockroach)

### Read the papers

- **Raft:** "In Search of an Understandable Consensus Algorithm" by Ongaro and Ousterhout (2014)
- **MVCC:** "A Critique of ANSI SQL Isolation Levels" by Berenson et al. (1995)
- **LSM-trees:** "The Log-Structured Merge-Tree" by O'Neil et al. (1996)
- **B-trees:** "Organization and Maintenance of Large Ordered Indices" by Bayer and McCreight (1970)

### Read the books

- *Designing Data-Intensive Applications* by Martin Kleppmann — the definitive guide to distributed systems concepts
- *Database Internals* by Alex Petrov — deep dive into storage engines and distributed databases
- *A Philosophy of Software Design* by John Ousterhout — the design principles referenced throughout this book

---

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

## DSA in Context: Testing Strategies and Coverage

Testing is itself a computer science problem. How do you test a system with infinite possible inputs?

### The testing pyramid

```
         ╱ ╲
        ╱ E2E╲        Few, slow, expensive
       ╱───────╲       Test the full system
      ╱ Integra-╲      Many modules together
     ╱  tion     ╲
    ╱─────────────╲
   ╱  Unit Tests   ╲   Many, fast, cheap
  ╱  (per function) ╲  Test one thing at a time
 ╱───────────────────╲
```

- **Unit tests:** Test individual functions. Fast (milliseconds), deterministic, easy to debug. Our `test_crc32_detects_corruption` is a unit test.
- **Integration tests:** Test multiple components together. Slower (seconds), may need setup/teardown. Our `test_recovery_after_writes` is an integration test.
- **End-to-end tests:** Test the complete system as a user would use it. Slow (seconds to minutes), may be flaky. Our golden SQL tests are end-to-end tests.

The ratio should be many unit tests, fewer integration tests, and few end-to-end tests. Unit tests catch most bugs quickly. Integration tests catch interface mismatches. End-to-end tests catch systemic issues.

### Coverage is not completeness

100% code coverage means every line of code was executed during testing. It does not mean every behavior was tested:

```rust,ignore
fn divide(a: i32, b: i32) -> i32 {
    a / b
}

#[test]
fn test_divide() {
    assert_eq!(divide(10, 2), 5); // 100% coverage!
}
// But divide(10, 0) panics — not covered by any test.
```

Property-based testing addresses this: instead of testing specific inputs, test properties across random inputs. But even property tests cannot test everything — the input space is infinite. The art of testing is choosing the right properties and the right edge cases.

### Mutation testing

A more powerful measure than coverage: **mutation testing**. The idea: automatically mutate your code (change `>` to `>=`, change `+` to `-`, remove a line) and check if any test fails. If a mutation does not cause a test failure, you have a gap in your test suite.

```
Original:  if count > 0 { ... }
Mutation:  if count >= 0 { ... }   ← does any test fail?

If no test fails, your tests do not distinguish between > and >=
for this variable. You need a test where count == 0.
```

Rust has the `cargo-mutants` tool for mutation testing. It is slow (it recompiles and runs tests for every mutation) but very effective at finding testing gaps.

---

## System Design Corner: Observability and Monitoring

Building a database is step one. Running it in production is step two. You need to know what your database is doing — is it healthy? Is it slow? Is it about to run out of disk?

### The three pillars of observability

**1. Metrics** — numerical measurements over time:

```
toydb_query_duration_seconds{type="select"}    0.023
toydb_query_duration_seconds{type="insert"}    0.045
toydb_raft_term                                5
toydb_raft_commit_index                        12847
toydb_storage_keys_total                       5032
toydb_storage_bytes_total                      15728640
toydb_connections_active                       3
```

Metrics answer: "How fast? How many? How much?" They are cheap to collect (a counter increment per operation), cheap to store (a few bytes per data point), and easy to alert on ("if query latency p99 exceeds 100ms, page the on-call engineer").

**2. Logs** — discrete events with context:

```
2024-01-15T10:23:45Z INFO  [server] Client connected from 192.168.1.5:43210
2024-01-15T10:23:45Z INFO  [sql]    Executing: SELECT * FROM users WHERE id = 42
2024-01-15T10:23:45Z DEBUG [plan]   Plan: Scan(users) -> Filter(id=42)
2024-01-15T10:23:46Z INFO  [sql]    Query completed: 1 row, 12ms
2024-01-15T10:23:47Z WARN  [raft]   Heartbeat to node 3 timed out
2024-01-15T10:23:48Z ERROR [raft]   Node 3 unreachable, marking as failed
```

Logs answer: "What happened?" They are the narrative record of the system's behavior. Structured logging (JSON format) makes logs searchable and parseable.

**3. Traces** — the path of a single request through the system:

```
Trace: query-abc123
├─ server.handle_request         2ms
│  ├─ lexer.tokenize             0.1ms
│  ├─ parser.parse               0.3ms
│  ├─ planner.plan               0.2ms
│  ├─ optimizer.optimize         0.05ms
│  ├─ raft.propose               15ms
│  │  ├─ wal.append_sync         3ms
│  │  └─ replicate_to_followers  12ms
│  └─ executor.execute           1ms
│     └─ mvcc.scan               0.8ms
└─ total                         18.65ms
```

Traces answer: "Why is this request slow?" They connect the dots between logs and metrics, showing exactly where time is spent. Distributed tracing (OpenTelemetry) follows requests across multiple services.

### What to monitor in a database

| Metric | Why it matters |
|--------|---------------|
| Query latency (p50, p95, p99) | User experience |
| Queries per second | Capacity planning |
| Error rate | Health |
| Raft term | Leader stability |
| Raft commit index lag | Replication health |
| WAL size | Disk usage, compaction needed |
| Connection count | Load |
| Memory usage | Capacity |
| fsync latency | Disk health |

> **Interview talking point:** *"I would add observability in three layers: Prometheus metrics for query latency histograms, error rates, and Raft health indicators; structured logging with request IDs for debugging specific queries; and distributed tracing with OpenTelemetry spans for each processing stage (lex, parse, plan, optimize, execute, replicate). For alerting, I would set up PagerDuty alerts on p99 latency exceeding SLO, Raft leader instability (frequent elections), and WAL size exceeding the compaction threshold. The metrics endpoint would be a /metrics HTTP handler that Prometheus scrapes every 15 seconds."*

---

## Design Insight: Testing Philosophy

> *"Software testing proves the presence of bugs, never their absence."*
> — Edsger Dijkstra

Ousterhout approaches testing pragmatically in *A Philosophy of Software Design*: tests should reduce the fear of modifying code. If you are afraid to change a module because "something might break," you need more tests for that module. Tests are a safety net that gives you confidence to refactor.

The debate between "test first" (TDD) and "code first" is less important than the outcome: **do you have enough tests to refactor with confidence?** For our database:

- **Storage engine:** Needs extensive tests because bugs cause data loss. Property tests are especially valuable — they explore the input space far more broadly than hand-written tests.
- **SQL parser:** Needs many specific tests because SQL has complex grammar rules. Golden tests are ideal — each test case is a single SQL statement and its expected AST.
- **Raft:** Needs deterministic simulation because real-world failures are hard to reproduce. The fake clock + fake network pattern eliminates timing-dependent test flakiness.
- **Integration:** Needs end-to-end tests that verify the layers work together. These are the most valuable tests in the entire suite — they catch bugs that no unit test would find.

The practical approach: write tests at the level where they are most effective. Do not force unit tests on code that is better tested at the integration level. Do not force integration tests when a unit test is sufficient. Match the testing strategy to the complexity of the code.

One more thing: **the compiler is your first test suite.** Rust's type system catches null pointer dereferences, data races, use-after-free, buffer overflows, and type mismatches at compile time. These are entire categories of bugs that JavaScript, Python, and Go developers must write tests to catch. Your test suite can focus on logic bugs because the compiler handles the rest.

---

## What You Built

In this chapter, you:

1. **Mastered Rust's testing framework** — `#[test]`, `#[cfg(test)]`, assertion macros, `#[should_panic]`, test modules, integration tests in `tests/`, doc tests
2. **Wrote property-based tests** — `proptest` strategies that generate random valid SQL and verify parser invariants across thousands of inputs
3. **Built deterministic distributed tests** — fake clock, fake network, test cluster, chaos engine with seeded randomness for reproducible failure scenarios
4. **Benchmarked storage engines** — `criterion` for statistical rigor, `black_box` to prevent dead code elimination, throughput measurements for Memory vs BitCask
5. **Created a golden test suite** — SQL scripts with expected output files, automatic diff on failure, easy-to-add test cases
6. **Reviewed the complete system** — all 18 chapters, all layers, the full architecture of a distributed SQL database built from scratch in Rust

---

## A Final Word

You started with a `HashMap` and a REPL. You ended with a distributed SQL database that parses queries, optimizes execution plans, provides transactional isolation, replicates data across a cluster for fault tolerance, persists state to survive crashes, and serves clients over a network.

This is not a toy. It is a real database — small and incomplete compared to PostgreSQL, but architecturally identical to CockroachDB, TiDB, and YugabyteDB. The same layers, the same patterns, the same tradeoffs. The difference is scale, not kind.

More importantly, you learned Rust by building something real. Not by reading about ownership in isolation, but by discovering why ownership matters when you need exclusive access to a WAL file. Not by memorizing trait syntax, but by defining a `Storage` trait so the executor does not need to know about BitCask. Not by studying lifetimes in the abstract, but by building MVCC where lifetimes determine which versions a transaction can see.

The code is yours. Extend it. Break it. Rewrite it. Use it as a reference when you encounter these patterns in production systems. And the next time someone asks "how does a database work?" — you know, because you built one.

---

### DS Deep Dive

Our testing chapter scratches the surface of distributed systems testing. The gold standard is the approach used by FoundationDB: deterministic simulation that models every source of non-determinism (time, network, disk, thread scheduling) and runs millions of simulated hours of cluster operation in minutes of wall-clock time. This deep dive explores the "simulation testing" paradigm, how it compares to TLA+ model checking, and why FoundationDB credits it with catching bugs that no amount of traditional testing would find.
