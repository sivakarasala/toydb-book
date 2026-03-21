# Chapter 18: Testing, Benchmarking & Extensions

You built a distributed SQL database from scratch. Seventeen chapters of storage engines, SQL parsing, query execution, MVCC transactions, client-server networking, and Raft consensus. That is a real achievement.

But building software is only half the job. The other half is knowing whether it works, how fast it is, and where it breaks. This chapter turns inward. You will learn how to test each layer with confidence, measure performance to find bottlenecks, and then step back to see the full picture of what you built.

By the end of this chapter, you will have:

- A thorough understanding of Rust's testing framework
- Unit tests using `#[test]` and assertion macros
- Integration tests in the `tests/` directory
- Benchmarks that measure and track performance
- A complete architecture review of your database
- Ideas for extending it further and where to go next

---

## Spotlight: Testing & Benchmarking

Every chapter has one **spotlight concept**. This chapter's spotlight is **testing and benchmarking** -- how Rust helps you verify correctness and measure performance.

### Why testing matters

Imagine you change the query optimizer to be faster. You run a quick test -- SELECT works. You deploy. Three days later, a user reports that DELETE queries silently skip rows. The optimizer change broke a code path you did not think to test.

Tests prevent this. A well-tested database has hundreds of tests that run in seconds. Every code change runs all of them. If the optimizer change breaks DELETE, the test fails immediately, before anyone is affected.

Rust makes testing especially powerful because the compiler already catches entire categories of bugs: null pointer dereferences, data races, use-after-free, buffer overflows. None of these can happen in safe Rust. So your tests can focus on **logic bugs** -- the kind the compiler cannot catch.

### Unit tests: `#[test]`

Rust's test framework is built into the language. You do not need to install a separate testing library. Any function annotated with `#[test]` is a test:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }

    #[test]
    fn test_add_zero() {
        assert_eq!(add(0, 0), 0);
    }
}
```

Run all tests with:

```
$ cargo test
running 3 tests
test tests::test_add_positive ... ok
test tests::test_add_negative ... ok
test tests::test_add_zero ... ok

test result: ok. 3 passed; 0 failed
```

> **Programming Concept: `#[cfg(test)]`**
>
> The `#[cfg(test)]` attribute tells the compiler: "only include this module when running tests." The test code is not included in your release binary. This means tests add zero overhead to the compiled database -- they exist only in the test build.
>
> `use super::*;` imports everything from the parent module (where `add` is defined) so the tests can call the function.

### Assertion macros

Rust provides several assertion macros for tests:

| Macro | What it checks | Example |
|-------|---------------|---------|
| `assert!(expr)` | Expression is true | `assert!(result.is_ok())` |
| `assert_eq!(a, b)` | a equals b | `assert_eq!(count, 42)` |
| `assert_ne!(a, b)` | a does not equal b | `assert_ne!(name, "")` |
| `assert!(expr, "msg")` | True, with custom message | `assert!(x > 0, "x was {}", x)` |

When an assertion fails, Rust prints both the expected and actual values:

```
thread 'tests::test_add' panicked at 'assertion failed: `(left == right)`
  left: `6`,
 right: `5`'
```

This is why most types in your codebase should derive `Debug` -- it makes test failure messages informative. Without `Debug`, Rust cannot print the values, and you get a much less helpful error.

### Testing for panics

Sometimes you want to verify that a function panics (crashes) on bad input:

```rust,ignore
#[test]
#[should_panic(expected = "index out of bounds")]
fn test_invalid_log_index() {
    let log = RaftLog::new();
    let _entry = log.get(999).unwrap();  // should panic
}
```

The `#[should_panic]` attribute says: "This test passes if the function panics." The `expected` parameter checks that the panic message contains the given text.

### Testing for errors

More often, you want to verify that a function returns an error (not a panic):

```rust,ignore
#[test]
fn test_parse_invalid_sql() {
    let result = Parser::parse("SELECTT * FROMM users");
    assert!(result.is_err());

    let error_message = result.unwrap_err().to_string();
    assert!(
        error_message.contains("unexpected"),
        "Expected error about unexpected token, got: {}",
        error_message
    );
}
```

> **What Just Happened?**
>
> We covered three ways to test edge cases:
> 1. `assert!(result.is_ok())` -- verify success
> 2. `assert!(result.is_err())` -- verify failure
> 3. `#[should_panic]` -- verify that bad input causes a panic
>
> In production code, you should prefer returning `Err` over panicking. Panics crash the entire program. Errors can be handled gracefully.

---

## Exercise 1: Unit Tests for Each Layer

**Goal:** Write thorough unit tests for every layer of the database.

### Layer 1: Storage Engine

The storage engine is the foundation. If it is buggy, nothing else matters.

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut store = MemoryStorage::new();

        // Set a key
        store.set(b"name", b"Alice").unwrap();

        // Get it back
        let value = store.get(b"name").unwrap();
        assert_eq!(value, Some(b"Alice".to_vec()));
    }

    #[test]
    fn test_get_nonexistent_key() {
        let store = MemoryStorage::new();

        // Getting a key that was never set should return None
        let value = store.get(b"missing").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_delete() {
        let mut store = MemoryStorage::new();

        store.set(b"key", b"value").unwrap();
        store.delete(b"key").unwrap();

        let value = store.get(b"key").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_overwrite() {
        let mut store = MemoryStorage::new();

        store.set(b"key", b"first").unwrap();
        store.set(b"key", b"second").unwrap();

        let value = store.get(b"key").unwrap();
        assert_eq!(value, Some(b"second".to_vec()));
    }
}
```

These tests verify the basic contract of a key-value store: set, get, delete, and overwrite. They are simple but catch real bugs -- like accidentally returning the old value after an overwrite.

### Layer 2: SQL Lexer

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
fn test_lex_string_literal() {
    let tokens = Lexer::new("'hello world'").tokenize().unwrap();
    assert_eq!(tokens, vec![
        Token::String("hello world".to_string()),
    ]);
}

#[test]
fn test_lex_unterminated_string() {
    let result = Lexer::new("'unterminated").tokenize();
    assert!(result.is_err());
}

#[test]
fn test_lex_empty_input() {
    let tokens = Lexer::new("").tokenize().unwrap();
    assert!(tokens.is_empty());
}
```

Lexer tests check that specific SQL inputs produce the expected tokens. Edge cases -- unterminated strings, empty input, unusual whitespace -- are especially important.

### Layer 3: SQL Parser

```rust,ignore
#[test]
fn test_parse_select() {
    let ast = Parser::parse("SELECT id, name FROM users").unwrap();
    match ast {
        Statement::Select { columns, from, .. } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(from, "users");
        }
        _ => panic!("expected SELECT statement"),
    }
}

#[test]
fn test_parse_insert() {
    let ast = Parser::parse("INSERT INTO users VALUES (1, 'Alice')").unwrap();
    match ast {
        Statement::Insert { table, values, .. } => {
            assert_eq!(table, "users");
            assert_eq!(values.len(), 1);  // one row
        }
        _ => panic!("expected INSERT statement"),
    }
}

#[test]
fn test_parse_invalid_sql() {
    let result = Parser::parse("SELECTT * FROMM");
    assert!(result.is_err());
}
```

Parser tests verify that SQL strings produce the correct syntax tree. The `match` + `panic!` pattern lets you assert specific properties of the AST.

### Layer 4: MVCC

```rust,ignore
#[test]
fn test_snapshot_isolation() {
    let mut storage = MvccStorage::new_in_memory();

    // Transaction 1: write a value
    let mut txn1 = storage.begin().unwrap();
    txn1.set(b"key", b"value1").unwrap();
    txn1.commit().unwrap();

    // Transaction 2: start reading (sees value1)
    let txn2 = storage.begin().unwrap();

    // Transaction 3: overwrite the value
    let mut txn3 = storage.begin().unwrap();
    txn3.set(b"key", b"value2").unwrap();
    txn3.commit().unwrap();

    // Transaction 2 should STILL see value1 (snapshot isolation)
    assert_eq!(
        txn2.get(b"key").unwrap(),
        Some(b"value1".to_vec())
    );
}
```

This test verifies **snapshot isolation** -- a transaction sees the database as it was when the transaction started, regardless of later writes by other transactions. This is one of the most important correctness properties of the storage engine.

### Layer 5: Raft

```rust,ignore
#[test]
fn test_leader_election() {
    // Create a 3-node cluster
    let mut node1 = RaftNode::new(1, vec![2, 3]);
    let mut node2 = RaftNode::new(2, vec![1, 3]);
    let mut node3 = RaftNode::new(3, vec![1, 2]);

    // Node 1 starts an election
    let messages = node1.start_election();
    assert_eq!(node1.state, NodeState::Candidate);
    assert_eq!(node1.current_term, 1);

    // Deliver RequestVote to nodes 2 and 3
    for (to, msg) in messages {
        match to {
            2 => {
                let responses = node2.handle_message(1, msg);
                // Deliver response back to node 1
                for (_, resp) in responses {
                    node1.handle_message(2, resp);
                }
            }
            3 => {
                let responses = node3.handle_message(1, msg);
                for (_, resp) in responses {
                    node1.handle_message(3, resp);
                }
            }
            _ => {}
        }
    }

    // Node 1 should be the leader
    assert_eq!(node1.state, NodeState::Leader);
}
```

This test manually drives a leader election by delivering messages between nodes. It verifies that the election protocol works correctly without any real networking.

> **What Just Happened?**
>
> We wrote unit tests for every layer of the database. Each layer has different testing needs:
> - **Storage:** basic CRUD operations and edge cases
> - **Lexer:** specific inputs produce expected tokens
> - **Parser:** SQL strings produce correct ASTs
> - **MVCC:** transaction isolation properties
> - **Raft:** election and replication correctness
>
> Running `cargo test` executes all of these in seconds, giving you confidence that every layer works correctly.

---

## Exercise 2: Integration Tests

**Goal:** Write end-to-end tests that exercise multiple layers together.

### Step 1: Where integration tests live

Rust has a dedicated place for integration tests: the `tests/` directory at the root of your project:

```
toydb/
+-- src/
|   +-- lib.rs
|   +-- ...
+-- tests/                     <-- integration tests
|   +-- sql_integration.rs
|   +-- storage_integration.rs
+-- Cargo.toml
```

Each file in `tests/` is compiled as a **separate crate**. This means integration tests can only use your library's **public API** -- they cannot access private functions. This is intentional: integration tests verify the API contract, not internal implementation details.

### Step 2: A SQL integration test

Create `tests/sql_integration.rs`:

```rust,ignore
// tests/sql_integration.rs

use toydb::Server;
use toydb::ServerConfig;

#[test]
fn test_create_insert_select() {
    let dir = tempfile::tempdir().unwrap();
    let config = ServerConfig {
        listen_addr: "127.0.0.1:0".to_string(),
        node_id: 1,
        data_dir: dir.path().to_str().unwrap().to_string(),
        peers: vec![],
    };

    let mut server = Server::new(config).unwrap();
    server.become_leader_for_test();

    // Create table
    let resp = server.execute("CREATE TABLE books (id INT, title TEXT)");
    assert!(matches!(resp, Response::Ok { .. }));

    // Insert rows
    server.execute("INSERT INTO books VALUES (1, 'Rust in Action')");
    server.execute("INSERT INTO books VALUES (2, 'Programming Rust')");
    server.execute("INSERT INTO books VALUES (3, 'The Rust Programming Language')");

    // Query
    let resp = server.execute("SELECT * FROM books");
    match resp {
        Response::Rows { rows, .. } => {
            assert_eq!(rows.len(), 3);
        }
        _ => panic!("expected rows"),
    }

    // Query with filter
    let resp = server.execute("SELECT title FROM books WHERE id = 2");
    match resp {
        Response::Rows { rows, .. } => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0][0], "Programming Rust");
        }
        _ => panic!("expected rows"),
    }
}
```

### Step 3: Run integration tests

```
$ cargo test --test sql_integration
running 1 test
test test_create_insert_select ... ok
```

The `--test` flag runs only the specified integration test file. Plain `cargo test` runs both unit tests and integration tests.

> **Programming Concept: Unit Tests vs Integration Tests**
>
> | | Unit Tests | Integration Tests |
> |---|---|---|
> | Location | Inside `src/` files, in `#[cfg(test)] mod tests` | In `tests/` directory |
> | Access | Can test private functions | Only public API |
> | Scope | One function or module | Multiple layers together |
> | Purpose | Verify internal logic | Verify external behavior |
> | Speed | Very fast | Slightly slower (more setup) |
>
> Both are valuable. Unit tests catch bugs early and precisely. Integration tests catch bugs that only appear when layers interact.

---

## Exercise 3: Benchmarking

**Goal:** Measure how fast your database operations are.

### Step 1: Why benchmarking matters

You have a working database. But is it fast enough? Benchmarking answers questions like:

- How many inserts per second can we do?
- How long does a SELECT with 1,000 rows take?
- Is the B-tree or the hash map faster for lookups?
- Did the optimizer actually make queries faster?

### Step 2: Simple timing with Instant

The simplest benchmark is just measuring elapsed time:

```rust,ignore
use std::time::Instant;

fn main() {
    let mut storage = MemoryStorage::new();

    // Benchmark inserts
    let start = Instant::now();
    let count = 10_000;

    for i in 0..count {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i);
        storage.set(key.as_bytes(), value.as_bytes()).unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = count as f64 / elapsed.as_secs_f64();
    println!(
        "Inserted {} keys in {:.2?} ({:.0} ops/sec)",
        count, elapsed, ops_per_sec
    );

    // Benchmark reads
    let start = Instant::now();

    for i in 0..count {
        let key = format!("key-{}", i);
        storage.get(key.as_bytes()).unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = count as f64 / elapsed.as_secs_f64();
    println!(
        "Read {} keys in {:.2?} ({:.0} ops/sec)",
        count, elapsed, ops_per_sec
    );
}
```

Sample output:

```
Inserted 10000 keys in 12.34ms (810,373 ops/sec)
Read 10000 keys in 5.67ms (1,764,082 ops/sec)
```

### Step 3: Using the criterion crate (advanced)

For more rigorous benchmarking, use the `criterion` crate. It runs benchmarks multiple times, computes statistics, and detects performance regressions.

Add to `Cargo.toml`:

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "storage_bench"
harness = false
```

Create `benches/storage_bench.rs`:

```rust,ignore
use criterion::{criterion_group, criterion_main, Criterion};
use toydb::storage::MemoryStorage;

fn bench_insert(c: &mut Criterion) {
    c.bench_function("memory_insert", |b| {
        let mut storage = MemoryStorage::new();
        let mut counter = 0u64;

        b.iter(|| {
            counter += 1;
            let key = format!("key-{}", counter);
            storage.set(key.as_bytes(), b"value").unwrap();
        });
    });
}

fn bench_get(c: &mut Criterion) {
    let mut storage = MemoryStorage::new();

    // Pre-populate with 10,000 keys
    for i in 0..10_000 {
        storage.set(
            format!("key-{}", i).as_bytes(),
            b"value",
        ).unwrap();
    }

    c.bench_function("memory_get", |b| {
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % 10_000;
            storage.get(format!("key-{}", i).as_bytes()).unwrap();
        });
    });
}

criterion_group!(benches, bench_insert, bench_get);
criterion_main!(benches);
```

Run with:

```
$ cargo bench
memory_insert     time:   [245.3 ns 248.1 ns 251.2 ns]
memory_get        time:   [123.4 ns 125.7 ns 128.3 ns]
```

Criterion gives you the mean time per operation, plus confidence intervals. Run it again after a change to see if performance improved or degraded.

> **What Just Happened?**
>
> Criterion measures each operation thousands of times, discards outliers, and computes statistical results. This is much more reliable than a single `Instant::now()` measurement, which can be noisy due to background processes, cache effects, and other system activity.

---

## Exercise 4: What You Built -- The Full Architecture

**Goal:** Step back and see the complete system you created.

### The layer cake

```
+-------------------------------------------------------+
|                      CLIENT                           |
|  toydb-repl or any TCP client                         |
+---------------------------+---------------------------+
                            |
                    TCP connection
                            |
+---------------------------v---------------------------+
|                      SERVER                           |
|  Accepts connections, routes queries                  |
+---------------------------+---------------------------+
                            |
              +-------------+-------------+
              |                           |
     +--------v--------+        +--------v--------+
     |   WRITE PATH    |        |   READ PATH     |
     +--------+--------+        +--------+--------+
              |                           |
     +--------v--------+                  |
     |      RAFT       |                  |
     |  Replicate to   |                  |
     |  majority       |                  |
     +--------+--------+                  |
              |                           |
     +--------v--------+        +---------v-------+
     |   SQL PIPELINE  |        |  SQL PIPELINE   |
     |  Lex -> Parse   |        |  Lex -> Parse   |
     |  -> Plan ->     |        |  -> Plan ->     |
     |  Optimize ->    |        |  Optimize ->    |
     |  Execute        |        |  Execute        |
     +--------+--------+        +--------+--------+
              |                           |
              +-------------+-------------+
                            |
              +-------------v-------------+
              |      MVCC STORAGE         |
              |  Transactions,            |
              |  Snapshot Isolation       |
              +-------------+-------------+
                            |
              +-------------v-------------+
              |      KV STORE             |
              |  Bitcask / Memory         |
              +---------------------------+
```

### What each layer does

| Layer | Chapter(s) | Purpose |
|-------|-----------|---------|
| KV Store | 2-3 | Store and retrieve bytes by key |
| MVCC | 5 | Transactions with snapshot isolation |
| SQL Lexer | 6 | Turn SQL text into tokens |
| SQL Parser | 7 | Turn tokens into a syntax tree |
| Query Planner | 8 | Turn syntax tree into execution plan |
| Query Optimizer | 9 | Make the execution plan faster |
| Query Executor | 10 | Run the plan against storage |
| SQL Features | 11 | CREATE TABLE, data types, constraints |
| Client-Server | 12 | TCP protocol, REPL client |
| Async Networking | 13 | Handle many clients concurrently |
| Raft Election | 14 | Choose a leader among servers |
| Raft Replication | 15 | Copy writes to multiple servers |
| Raft Durability | 16 | Survive crashes and restarts |
| Integration | 17 | Wire everything together |
| Testing | 18 | Verify it all works |

### The numbers

Roughly speaking, your database has:

- **~5,000-8,000 lines of Rust code** (depending on how many exercises you completed)
- **~10-15 source files** organized into 4-5 modules
- **~50-100 tests** covering every layer
- **4 external dependencies** (tokio, serde, tempfile, and optionally criterion)

This is a real, working distributed database. It is not production-ready (it is missing many features that production databases have), but it demonstrates every core concept: storage, SQL, transactions, networking, and consensus.

---

## Exercise 5: Ideas for Extending the Database

**Goal:** Explore what you could build next.

### Feature ideas

**1. More SQL features**
- `JOIN` -- combine rows from multiple tables
- `GROUP BY` and aggregate functions (`COUNT`, `SUM`, `AVG`)
- `ORDER BY` with `LIMIT` and `OFFSET`
- `ALTER TABLE` to modify existing tables
- Subqueries: `SELECT * FROM users WHERE id IN (SELECT ...)`

**2. Indexing**
- B-tree indexes for fast lookups
- `CREATE INDEX` and `DROP INDEX` SQL commands
- Automatic index selection in the query optimizer

**3. Better storage**
- LSM-tree (Log-Structured Merge tree) -- the engine behind LevelDB, RocksDB, and Cassandra
- Compression for stored data
- Compaction strategies

**4. Cluster operations**
- Dynamic membership changes (adding/removing nodes)
- Leader transfer (gracefully moving leadership to another node)
- Read replicas (serve reads from followers)

**5. Client improvements**
- Connection pooling
- Prepared statements (parse once, execute many times)
- A proper client library that other programs can use

**6. Observability**
- Metrics (queries per second, latency percentiles, replication lag)
- Query explain plan (`EXPLAIN SELECT ...`)
- Slow query log

### What to study next

If you want to go deeper into databases:

- **"Designing Data-Intensive Applications"** by Martin Kleppmann -- the best overview of distributed systems and databases
- **The Raft paper** -- "In Search of an Understandable Consensus Algorithm" by Diego Ongaro and John Ousterhout
- **CMU Database Course** (15-445/645) -- Andy Pavlo's lectures on YouTube are excellent and free
- **"Database Internals"** by Alex Petrov -- detailed coverage of storage engines and distributed systems

If you want to go deeper into Rust:

- **"Programming Rust"** by Jim Blandy, Jason Orendorff, and Leonora Tindall -- the comprehensive Rust reference
- **"Rust for Rustaceans"** by Jon Gjengset -- advanced Rust techniques
- **The Rust Book** (online, free) -- the official guide at doc.rust-lang.org

### Project ideas using what you learned

- **A distributed key-value store** (like Redis, but replicated with Raft)
- **A message queue** (like Kafka, with log-based storage and replication)
- **A distributed lock service** (like ZooKeeper or etcd)
- **A time-series database** (optimized for metrics and monitoring data)

Each of these projects uses concepts from this book: storage engines, networking, serialization, consensus, and Rust's type system.

---

## Summary

You tested, benchmarked, and reviewed your database:

- **`#[test]`** and assertion macros make testing a first-class feature of Rust
- **`#[cfg(test)]`** keeps test code out of production binaries
- **Unit tests** verify individual functions and modules
- **Integration tests** in `tests/` verify that layers work together
- **Benchmarking** with `Instant` or `criterion` measures performance
- **Testing edge cases** (invalid input, concurrent operations, crash recovery) builds confidence

And stepping back, you built something remarkable:

- A **storage engine** with MVCC transactions and snapshot isolation
- A **SQL engine** with lexing, parsing, planning, optimization, and execution
- A **networking layer** with async I/O handling thousands of concurrent connections
- A **consensus protocol** (Raft) with leader election, log replication, and crash recovery
- **Integration** that wires every layer into a single working database

Every line is Rust code you wrote. You understand what happens at every level, from the bytes on disk to the SQL query a user types. That understanding is rare and valuable.

Whatever you build next -- whether it is a web application, a systems tool, a game engine, or another database -- you now have the foundation to build it well. The ownership model, the type system, the module system, the error handling, the testing framework -- these are tools you will use in every Rust project.

Congratulations. You built a database from scratch.
