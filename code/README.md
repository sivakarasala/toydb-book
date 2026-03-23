# Working Through the Code

Each chapter has a self-contained Rust crate with an **exercise** (your job) and a **solution** (reference).

## Structure

```
code/chXX/
├── Cargo.toml
├── src/
│   ├── main.rs              ← Exercise: has todo!() stubs + tests
│   └── bin/
│       └── solution.rs      ← Solution: complete code + passing tests
```

The capstone has 8 challenges:

```
code/capstone/
├── Cargo.toml
├── src/bin/
│   ├── c1_kv_range_query.rs           ← Exercise
│   ├── c1_kv_range_query_solution.rs  ← Solution
│   ├── c2_sql_expression_eval.rs
│   └── ...
```

## How to Work Through a Chapter

### 1. Read the chapter in the book first

The book explains the concepts. The code is where you practice.

### 2. Run the exercise tests (they will fail)

```bash
cd code/ch06
cargo test --bin exercise
```

You'll see `todo!()` panics — that's expected.

### 3. Fill in the `todo!()` stubs

Open `src/main.rs` and replace each `todo!(...)` with your implementation.
The comments and test cases tell you exactly what's expected.

### 4. Run tests until they pass

```bash
cargo test --bin exercise
```

### 5. Check your work against the solution

```bash
cargo test --bin solution    # verify solution passes
cargo run --bin solution     # run the solution demo
```

Or read `src/bin/solution.rs` directly.

## Chapter Progression

Build the database layer by layer — each chapter builds on the last:

| Ch | What You Build | Key Rust Concept |
|----|---------------|-----------------|
| 01 | Key-value store with `HashMap` | Enums, match, collections |
| 02 | Storage trait + `BTreeMap` engine | Traits, generics |
| 03 | BitCask persistent engine | File I/O, error handling |
| 04 | Binary serialization | serde, derive macros |
| 05 | MVCC transactions | Lifetimes, references |
| 06 | SQL lexer (tokenizer) | Enums, pattern matching |
| 07 | SQL parser (AST builder) | Recursive types, `Box` |
| 08 | Query planner | Iterators, closures |
| 09 | Query optimizer | Trait objects, `dyn` dispatch |
| 10 | Query executor (Volcano model) | Iterator pattern |
| 11 | Joins, GROUP BY, sorting | Collections, algorithms |
| 12 | TCP client-server protocol | Structs, networking |
| 13 | Async server with Tokio | async/await |
| 14 | Raft leader election | State machines |
| 15 | Raft log replication | `Arc<Mutex<T>>` |
| 16 | Write-ahead log (WAL) | Ownership, persistence |
| 17 | Integration: SQL over Raft | Module system |
| 18 | Testing & benchmarking | Testing patterns |

## Capstone Challenges

After finishing all 18 chapters, tackle the capstone:

| # | Challenge | Algorithm |
|---|-----------|-----------|
| C1 | KV Range Query | BTreeMap traversal |
| C2 | SQL Expression Evaluator | Tree recursion |
| C3 | Query Plan Builder | Tree construction |
| C4 | Transaction Scheduler | Topological sort |
| C5 | Raft Log Compaction | Sliding window |
| C6 | Index Scan Optimizer | Cost estimation |
| C7 | Deadlock Detector | Cycle detection (DFS) |
| C8 | Distributed Counter | CRDTs (G-Counter, PN-Counter) |

```bash
cd code/capstone

# Work on a challenge
cargo test --bin c1-exercise

# Check the solution
cargo test --bin c1-solution
```

## The Full Database: `code/toydb/`

The per-chapter exercises teach concepts in isolation. The `my-toydb` crate is where you bring them all together into a **real, working database**.

### How to build it yourself

A starter skeleton is already set up at `code/my-toydb/`. Every file has a `// TODO` stub and a reference pointer to `code/toydb/`. The recommended workflow is: **finish the chapter exercise first, then fill in the corresponding file in `my-toydb/`.**

```bash
cd code/my-toydb
cargo build   # Compiles out of the box — all modules are commented out
```

| After Chapter | What to fill in (`code/my-toydb/src/`) |
|--------------|------------------------------|
| Ch 1-2 | Uncomment `storage` in `lib.rs`. Fill in `storage/mod.rs` (Storage trait) and `storage/memory.rs` (BTreeMap engine). Your database can store and retrieve key-value pairs. |
| Ch 3 | Uncomment `error` in `lib.rs`. Fill in `error.rs` with a proper error enum using `thiserror`. Refactor Storage methods to return `Result`. |
| Ch 6 | Uncomment `sql` in `lib.rs`. Fill in `sql/lexer.rs`. Your database can now tokenize SQL strings. |
| Ch 7 | Fill in `sql/parser.rs`. Tokens become an AST — CREATE TABLE, INSERT, SELECT are now structured data. |
| Ch 8-9 | Fill in `sql/planner.rs`. The AST becomes an execution plan with filter expressions. |
| Ch 10-11 | Fill in `sql/executor.rs` and `sql/types.rs`. **This is the big milestone** — your database can now actually execute SQL queries against stored data. |
| Ch 12-13 | (Optional) Add a TCP server so clients can connect over the network. |
| Ch 14-16 | Uncomment `raft` in `lib.rs`. Fill in `raft/mod.rs` and `raft/wal.rs`. Every SQL command is logged to a WAL before execution. Your database now survives crashes. |
| Ch 17 | Fill in `lib.rs` (Database struct) and `main.rs` (REPL). Wire all layers together. |
| Ch 18 | Add integration tests that exercise the full pipeline. |

At each step, run `cargo build` to make sure everything compiles, and `cargo test` to verify your layers work together.

### The reference implementation (`code/toydb/`)

The `code/toydb/` directory contains a **complete reference implementation** — the finished product after all 18 chapters. You can use it to:

- **Compare your work** — see how your implementation differs from the reference
- **Get unstuck** — if you're stuck wiring layers together, peek at how the reference does it
- **Run it immediately** — try the full database before you build it yourself

```
code/toydb/
├── src/
│   ├── main.rs           ← SQL REPL (interactive shell)
│   ├── lib.rs            ← Database engine (wires all layers)
│   ├── error.rs          ← Error types
│   ├── storage/
│   │   ├── mod.rs        ← Storage trait (Ch2)
│   │   └── memory.rs     ← In-memory engine (Ch1-2)
│   ├── sql/
│   │   ├── types.rs      ← Type system (Value, DataType, Schema)
│   │   ├── lexer.rs      ← Tokenizer (Ch6)
│   │   ├── parser.rs     ← AST builder (Ch7)
│   │   ├── planner.rs    ← Query planner (Ch8-9)
│   │   └── executor.rs   ← Query executor (Ch10-11)
│   └── raft/
│       ├── mod.rs        ← Raft log + recovery (Ch14-16)
│       └── wal.rs        ← Write-ahead log (Ch16)
```

### Your starter skeleton (`code/my-toydb/`)

The `code/my-toydb/` directory mirrors the same structure with `// TODO` stubs:

```
code/my-toydb/
├── Cargo.toml            ← Dependencies commented out — uncomment as needed
├── src/
│   ├── main.rs           ← Prints milestones (replace with REPL at Ch17)
│   ├── lib.rs            ← All modules commented out — uncomment as you build
│   ├── error.rs          ← TODO: Error types (Ch3)
│   ├── storage/
│   │   ├── mod.rs        ← TODO: Storage trait (Ch2)
│   │   └── memory.rs     ← TODO: BTreeMap engine (Ch1-2)
│   ├── sql/
│   │   ├── mod.rs        ← Module declarations
│   │   ├── types.rs      ← TODO: Value, Schema types (Ch10)
│   │   ├── lexer.rs      ← TODO: Tokenizer (Ch6)
│   │   ├── parser.rs     ← TODO: AST builder (Ch7)
│   │   ├── planner.rs    ← TODO: Query planner (Ch8-9)
│   │   └── executor.rs   ← TODO: Executor (Ch10-11)
│   └── raft/
│       ├── mod.rs        ← TODO: Raft log (Ch14-16)
│       └── wal.rs        ← TODO: Write-ahead log (Ch16)
```

Each TODO file has a doc comment pointing to the corresponding file in `code/toydb/` for reference.

### Running the reference

```bash
cd code/toydb

# In-memory mode
cargo run

# With WAL persistence (survives restarts)
cargo run -- --wal /tmp/toydb.wal
```

Then interact with your database:

```sql
toydb> CREATE TABLE users (id INT, name TEXT, age INT)
toydb> INSERT INTO users VALUES (1, 'Alice', 30)
toydb> INSERT INTO users VALUES (2, 'Bob', 25)
toydb> SELECT name, age FROM users WHERE age > 28 ORDER BY age DESC
toydb> SELECT COUNT(*) FROM users
toydb> DELETE FROM users WHERE age < 27
toydb> DROP TABLE users
```

## Tips

- **Don't peek at the solution first.** Struggle with the `todo!()` stubs — that's where learning happens.
- **Read the test cases.** They're your specification. Each test name tells you what behavior to implement.
- **Use `cargo test --bin exercise -- --nocapture`** to see println output during tests.
- **One chapter at a time.** Later chapters assume you understand earlier ones.
