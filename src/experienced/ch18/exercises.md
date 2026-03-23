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
