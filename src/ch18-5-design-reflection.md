# Design Reflection — A Philosophy of Software Design

You have built a database from scratch. Not a toy. A real system with a storage engine, SQL parser, query planner, MVCC concurrency control, client-server networking, and Raft consensus — all in Rust. Before you move on to interview prep and system design challenges, it is worth pausing to examine what you built through the lens of software design philosophy.

John Ousterhout's *A Philosophy of Software Design* argues that the central challenge of software engineering is managing complexity. Not performance, not features, not shipping speed — **complexity**. Complexity is what makes code hard to understand, hard to modify, and hard to extend. It is what causes bugs, delays, and burnout.

This chapter applies Ousterhout's principles to our database. Every principle is illustrated with code you have already written. This is not abstract theory — it is a retrospective on real design decisions, what worked, what did not, and what you would change if you started over.

---

## Complexity Is Incremental

Ousterhout's most important observation: complexity is not introduced all at once. No one writes a messy codebase on day one. Complexity creeps in one small decision at a time — a shortcut here, an extra parameter there, a special case that seemed harmless. Each addition is small, but they accumulate like sediment in a riverbed until the whole system is clogged.

### How we experienced this in toydb

Look at the layers you built, chapter by chapter:

```
Chapter 1-2:   HashMap/BTreeMap key-value store
Chapter 3:     Persistent storage (BitCask log)
Chapter 4:     Serialization (binary encoding)
Chapter 5:     MVCC (versioned reads/writes)
Chapter 6-7:   SQL lexer + parser
Chapter 8-9:   Query planner + optimizer
Chapter 10-11: Executor + SQL features (joins, aggregations)
Chapter 12-13: Client-server protocol + async networking
Chapter 14-16: Raft consensus (election, replication, durability)
Chapter 17:    Integration — all layers wired together
```

Each layer seemed manageable in isolation. A `Storage` trait with four methods. A `Token` enum with fifteen variants. A `Plan` node with five cases. But by Chapter 17, you were threading a SQL string through seven layers, each with its own error type, its own invariants, and its own edge cases. The total complexity was not the sum of the layers — it was the product of their interactions.

Consider the write path for `INSERT INTO users VALUES (1, 'Alice')`:

```
SQL string
  → Lexer (tokenize)
    → Parser (build AST)
      → Planner (resolve tables, build plan)
        → Optimizer (reorder, simplify)
          → Executor (produce rows/effects)
            → MVCC (version the write)
              → Storage (persist to BitCask log)
                → Raft (replicate to peers)
                  → Network (send to followers)
```

Each arrow is a place where an error can occur, a type must be converted, and invariants must be maintained. The lexer must correctly handle string escapes so the parser receives valid tokens. The parser must build a well-formed AST so the planner can resolve column references. The planner must generate a correct plan so the optimizer does not produce invalid rewrites. Each layer trusts the one above it — and that trust chain is fragile.

### The sediment pattern

The most insidious form of incremental complexity is what I call the **sediment pattern**: early decisions that seem fine at first but create hidden costs as the system grows.

In Chapter 2, you defined the `Storage` trait:

```rust,ignore
pub trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete(&mut self, key: &str) -> Result<(), StorageError>;
    fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;
}
```

This is clean and simple. But by Chapter 5, the MVCC layer needed to encode version numbers into keys — `("users/1", version=3)` became a composite key. By Chapter 8, the query planner needed range scans — not just prefix scans. By Chapter 14, Raft needed to replicate entire state snapshots. Each need was reasonable, but the original four-method trait accumulated complexity that a fresh design would have handled differently.

### The lesson

Expect complexity to grow. Design interfaces with headroom. The `Storage` trait should have included range scans from the beginning — not because you needed them in Chapter 2, but because a storage engine without range scans is not a storage engine. The cost of adding `scan_range(start, end)` in Chapter 2 is near zero. The cost of adding it in Chapter 10, when the MVCC layer, executor, and tests all depend on the existing scan API, is significant.

---

## Deep Modules

A **deep module** has a simple interface but a complex implementation. A **shallow module** has a complex interface relative to the functionality it provides. Ousterhout argues that deep modules are the primary weapon against complexity: they hide a large amount of work behind a small, easy-to-use API.

### The Storage trait: our deepest module

The `Storage` trait is the deepest module in toydb. Its interface is four methods:

```rust,ignore
pub trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete(&mut self, key: &str) -> Result<(), StorageError>;
    fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;
}
```

Behind this interface, the `MemoryStorage` implementation is 30 lines of code. The `LogStorage` (BitCask) implementation is 300+ lines — it manages an append-only log file, an in-memory index, CRC32 checksums, tombstone records, crash recovery, buffered I/O, and startup replay. The caller sees the same four methods regardless.

This is depth. The ratio of implementation complexity to interface complexity is enormous. A developer using `LogStorage` does not need to know about file offsets, checksums, or tombstones. They call `set("user:1", bytes)` and the data persists. They call `get("user:1")` and the data comes back. The 300 lines of complexity are invisible.

### Measuring depth visually

Ousterhout draws modules as rectangles. The width represents the interface (how much the caller must know), and the height represents the implementation (how much work the module does):

```
    ┌───┐
    │   │      Deep module: narrow interface, tall implementation
    │   │      (Storage trait, MVCC, Raft)
    │   │
    │   │
    │   │
    └───┘

    ┌───────────────┐
    │               │   Shallow module: wide interface, short implementation
    └───────────────┘   (a struct with 10 public fields and trivial getters)
```

Our database has several deep modules:

| Module | Interface | Hidden Complexity |
|--------|-----------|-------------------|
| `Storage` trait | 4 methods | File I/O, checksums, indexing, crash recovery |
| `MvccStorage` | `begin()`, `get()`, `set()`, `commit()` | Version chains, snapshot isolation, garbage collection |
| `Parser` | `parse(tokens) -> AST` | Precedence climbing, error recovery, nested expressions |
| `Executor` | `next() -> Option<Row>` | Hash joins, aggregation buffers, sort spills |
| `RaftNode` | `propose(command)`, `step(message)` | Election protocol, log replication, term management |

Each one hides a massive amount of complexity behind a small interface. The rest of the codebase interacts with them through these narrow APIs and never needs to understand the internals.

### A shallow module counterexample

Contrast the `Storage` trait with how you might have designed the `Value` enum:

```rust,ignore
// A shallow design — the caller must handle every variant explicitly
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl Value {
    pub fn as_integer(&self) -> Option<i64> { ... }
    pub fn as_float(&self) -> Option<f64> { ... }
    pub fn as_string(&self) -> Option<&str> { ... }
    pub fn as_boolean(&self) -> Option<bool> { ... }
    pub fn is_null(&self) -> bool { ... }
    pub fn is_truthy(&self) -> bool { ... }
    pub fn type_name(&self) -> &str { ... }
}
```

This is a shallow module: seven public methods for five variants. The interface is almost as complex as the implementation. Every caller must match on variants and handle type mismatches. There is little hidden complexity — the type is essentially transparent.

This is not necessarily wrong. Value types *should* be transparent — the whole point is for the executor to inspect and manipulate values. The lesson is not "make everything deep" but rather "make modules deep where complexity benefits from hiding." Storage and consensus are complex — hide them. Value representation is simple — expose it.

### The depth principle applied to Raft

The Raft module is perhaps the best example of depth in our codebase. Its external interface is essentially:

```rust,ignore
impl RaftNode {
    /// Propose a command to be replicated across the cluster.
    pub fn propose(&mut self, command: Vec<u8>) -> Result<(), RaftError>;

    /// Process an incoming message from another node.
    pub fn step(&mut self, message: Message) -> Result<(), RaftError>;

    /// Check if any timeouts have elapsed and act on them.
    pub fn tick(&mut self);
}
```

Three methods. Behind them: leader election with randomized timeouts, vote counting with quorum logic, log replication with consistency checks, term management, heartbeat scheduling, match index tracking, commit index advancement, and state machine application. Hundreds of lines of subtle distributed systems logic, accessible through three method calls.

A developer integrating Raft into the database does not need to understand election protocols. They call `propose(command)` and the command gets replicated. They call `step(message)` when a network message arrives. They call `tick()` on a timer. The rest is hidden.

---

## Information Hiding

Information hiding is the mechanism that makes deep modules possible. A module that hides information does not expose its internal data structures, algorithms, or implementation decisions. Callers depend on the interface, not the internals.

### MVCC hides versioning from SQL

The SQL layer executes queries without knowing that multiple versions of each row exist. When the executor calls `mvcc.get("users/1")`, it gets back the version visible to the current transaction — or `None` if the row does not exist in the current snapshot. The executor never sees version numbers, never compares timestamps, never resolves conflicts. All of that is hidden inside the MVCC layer.

```rust,ignore
// The executor's view — no version numbers visible
impl ScanExecutor {
    fn next(&mut self) -> Result<Option<Row>, ExecutorError> {
        // Ask the MVCC transaction for the next key-value pair.
        // The version filtering happens inside the transaction.
        while let Some((key, value)) = self.txn.scan_next()? {
            let row = deserialize_row(&value)?;
            return Ok(Some(row));
        }
        Ok(None)
    }
}
```

The MVCC layer's internal structure — version chains, active transaction sets, garbage collection thresholds — is entirely hidden. If you changed from snapshot isolation to serializable isolation, the executor code would not change. If you switched from version chains to a write-ahead log for undo, the executor code would not change. The information boundary is clean.

### Raft hides replication from MVCC

Similarly, the MVCC layer does not know that its writes are being replicated to other servers. It calls `storage.set(key, value)` and the storage layer handles replication. The MVCC layer could be running on a single machine or replicated across five data centers — it does not know and does not care.

```rust,ignore
// MVCC commits a transaction — has no idea about replication
impl MvccTransaction {
    pub fn commit(self) -> Result<(), MvccError> {
        for (key, value) in self.write_set {
            // This might go to a local BTreeMap, a BitCask file,
            // or through Raft to three servers. MVCC doesn't know.
            self.storage.set(key, value)?;
        }
        self.storage.set(
            format!("_txn/{}/status", self.version),
            serialize(&TxnStatus::Committed),
        )?;
        Ok(())
    }
}
```

This layering — SQL unaware of MVCC, MVCC unaware of Raft, Raft unaware of the network transport — is information hiding at work. Each layer depends only on the interface of the layer below it, not on its implementation.

### What we hid well

| Layer | Hidden Information |
|-------|-------------------|
| Storage | File format, checksum algorithm, index structure, crash recovery |
| MVCC | Version numbering scheme, active transaction tracking, snapshot algorithm |
| Parser | Token lookahead count, precedence table, error recovery strategy |
| Optimizer | Cost model, rewrite rules, join ordering algorithm |
| Raft | Election protocol, log compaction triggers, quorum calculation |

### What we leaked

Information hiding is not free — you must actively maintain it. Here are places where our design leaked information across boundaries:

**1. Key encoding scheme leaked into MVCC.**

The MVCC layer encodes version numbers into storage keys: `"table/row_id/version"`. This means the MVCC layer knows about the key format of the layer below it — it must construct keys that the storage engine will sort correctly. A cleaner design would have the storage engine accept composite keys natively:

```rust,ignore
// Current: MVCC constructs string keys that embed version info
let key = format!("{}/{:020}", user_key, version);
storage.set(key, value)?;

// Cleaner: storage accepts structured keys
storage.set_versioned(user_key, version, value)?;
```

**2. Error types crossed boundaries.**

In Chapter 17, you needed `From` implementations to convert `StorageError` to `MvccError` to `ExecutorError` to `ServerError`. This is unavoidable in Rust, but the sheer number of conversions reveals that errors are not fully hidden. Each layer must at least name the error types of the layer below it.

**3. Serialization format leaked into the executor.**

The executor deserializes `Vec<u8>` into `Row` values. This means the executor knows the serialization format — if you changed from bincode to MessagePack, the executor code would change. A deeper design would have the storage layer return typed rows directly:

```rust,ignore
// Current: executor deserializes manually
let bytes = storage.get(key)?;
let row: Row = bincode::deserialize(&bytes)?;

// Deeper: storage returns typed data
let row: Row = storage.get_typed(key)?;
```

### The takeaway

Information hiding is a spectrum, not a binary. Our database hides most information at most boundaries. The leaks are real but manageable. In a redesign, you would focus on the key encoding and serialization boundaries — those are the places where changes in one layer would ripple into others.

---

## Define Errors Out of Existence

Ousterhout's most provocative principle: the best way to handle errors is to design them away. Do not add error-handling code — change the API so the error cannot occur. This sounds reckless, but it is the opposite: it reduces complexity by eliminating code paths that exist only to handle conditions that could have been prevented.

### How Raft simplified Paxos

The original Paxos consensus algorithm is mathematically elegant but notoriously hard to implement. It handles many edge cases — multiple proposers competing simultaneously, promise conflicts, partial acceptances — each requiring careful error handling. Implementors regularly get it wrong.

Raft's key insight was to **define several of these errors out of existence** by imposing a stronger structure:

1. **Single leader per term.** Paxos allows any node to propose at any time, leading to conflicts that must be resolved. Raft restricts proposals to the leader. If you are not the leader, you do not propose — there is no "conflicting proposal" error because only one node proposes.

2. **Log entries are consecutive.** Paxos allows gaps in the log — entry 5 might be decided before entry 4. Raft requires entries to be consecutive. There is no "fill the gap" logic because gaps cannot occur.

3. **Leader Completeness Property.** If an entry is committed, every future leader will have that entry. Raft enforces this through its election protocol — candidates with incomplete logs cannot win elections. There is no "leader missing committed entries" error.

Each of these decisions eliminates entire categories of error handling code. The Raft paper is explicit about this: "Raft is designed for understandability" — and a key tool for understandability is making error cases impossible.

### Where we applied this in toydb

**1. The lexer never returns invalid tokens.**

The lexer in Chapter 6 does not return a `Result<Token, LexError>`. Instead, it produces a `Token::Invalid(String)` variant. The parser handles invalid tokens as part of its normal flow — there is no separate error-handling path for lexer failures. The error has been defined into a normal token variant:

```rust,ignore
enum Token {
    // Normal tokens
    Select,
    From,
    Where,
    Integer(i64),
    Ident(String),
    // ...

    // Not an error — just another token type
    Invalid(String),
}
```

The parser sees `Token::Invalid` and produces a parse error with a good message. The lexer itself never fails — it always produces a token stream. This eliminates the need for the caller to handle two types of errors (lexer errors and parser errors). There is one error path: the parser returns `Err(ParseError)`.

**2. Default values eliminate missing-field errors.**

In the query planner, optional clauses default to values that make the query correct without special cases:

```rust,ignore
// No WHERE clause? Default to a predicate that always returns true.
let predicate = where_clause.unwrap_or(Expression::Literal(Value::Boolean(true)));

// No ORDER BY? Default to an empty sort key list.
let order_by = order_by_clause.unwrap_or_default();

// No LIMIT? Default to usize::MAX — effectively unlimited.
let limit = limit_clause.unwrap_or(usize::MAX);
```

The executor does not check "is there a WHERE clause?" It always applies the predicate. If the predicate is `true`, every row passes — which is exactly what "no WHERE clause" means. The "missing WHERE clause" case has been defined out of existence.

**3. Tombstones define delete out of existence for the storage engine.**

The BitCask storage engine does not actually delete data from its log file — it appends a **tombstone** record that marks the key as deleted. The in-memory index removes the key. From the perspective of anyone calling `get()`, the key does not exist. But the storage engine never has to seek to a specific file offset and overwrite data — a complex, error-prone operation. Delete is defined as "write a special marker," which uses the same append path as any other write.

```rust,ignore
fn delete(&mut self, key: &str) -> Result<(), StorageError> {
    // Write a tombstone — same code path as set()
    self.append_record(key, None)?;
    // Remove from in-memory index
    self.index.remove(key);
    Ok(())
}
```

No file truncation. No hole management. No compaction-during-delete. The delete operation is just another write.

### Where we could have applied this

**Missing table errors in the planner.** When the planner encounters `SELECT * FROM nonexistent`, it returns an error. An alternative: define a system where every possible table name maps to something. If the table does not exist in the user's schema, it maps to an empty table. This is how some systems handle `SELECT * FROM information_schema.tables` — the system tables always exist, even if they are empty. We chose not to do this because silently returning empty results for typos would be confusing. Sometimes errors should remain errors.

---

## Design It Twice

Ousterhout recommends designing every module at least twice before committing to an implementation. Not because the first design is always bad — but because comparing two designs reveals tradeoffs you would not see otherwise.

### Memory vs BitCask: two storage engine designs

This is the most explicit "design it twice" moment in the book. You built two implementations of the same `Storage` trait:

**MemoryStorage (Chapter 2):**

```rust,ignore
pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl Storage for MemoryStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError> {
        self.data.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.data.get(key).cloned())
    }
}
```

**LogStorage (Chapter 3):**

```rust,ignore
pub struct LogStorage {
    writer: BufWriter<File>,
    index: HashMap<String, u64>,   // key -> file offset
    path: PathBuf,
}

impl Storage for LogStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError> {
        let offset = self.append_record(&key, Some(&value))?;
        self.index.insert(key, offset);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match self.index.get(key) {
            Some(&offset) => self.read_record_at(offset),
            None => Ok(None),
        }
    }
}
```

By building both, you discovered tradeoffs that would have been invisible if you had built only one:

| Dimension | MemoryStorage | LogStorage |
|-----------|--------------|------------|
| Write speed | O(log n) — BTreeMap insert | O(1) — file append |
| Read speed | O(log n) — BTreeMap lookup | O(1) — HashMap lookup + file seek |
| Durability | None — data dies with process | Full — data survives crashes |
| Memory usage | All data in memory | Only keys in memory; values on disk |
| Startup time | Instant | O(n) — must replay log to rebuild index |
| Space efficiency | Good — no duplication | Poor — old values linger until compaction |
| Crash recovery | N/A | Requires CRC validation, truncation handling |

The MemoryStorage taught you the interface. The LogStorage taught you the trade-offs of persistence. Having both available let you use MemoryStorage in tests (fast, deterministic) and LogStorage in production (durable). This is the practical benefit of "design it twice" — you end up with both designs, each serving a different purpose.

### Two parser designs we did not build

Another "design it twice" opportunity we did not take: the SQL parser. We built a hand-written recursive descent parser with precedence climbing for expressions. The alternative was a parser combinator library like `nom` or `chumsky`. Comparing the two designs reveals:

| Dimension | Hand-written | Parser combinator |
|-----------|-------------|-------------------|
| Error messages | Custom, precise, contextual | Often generic, hard to customize |
| Learning value | Deep — you understand every character consumed | Surface — you learn the library's API |
| Maintainability | Easy to add one SQL feature | Easy to add many features at once |
| Code volume | 400+ lines | ~150 lines |
| Dependencies | Zero | One crate |

We chose hand-written because the book's goal is learning, and you learn more by writing a parser than by configuring one. But the comparison is valuable: in a production system where you need to support full SQL syntax, a parser combinator saves weeks of work.

### The "what if" exercise

For every major module in toydb, ask: "what would the alternative design look like?"

| Module | Our Design | Alternative |
|--------|-----------|-------------|
| Storage | BTreeMap + BitCask log | LSM tree (LevelDB-style) |
| MVCC | Version chains in storage keys | Write-ahead log for undo |
| Lexer | Character-by-character state machine | Regex-based tokenizer |
| Query plan | Tree of Plan nodes | Array of instruction opcodes |
| Executor | Volcano (pull-based) | Vectorized (batch) or push-based |
| Raft | Message-passing with `step()` | Actor model with channels |
| Serialization | Custom binary format | Protocol Buffers or MessagePack |

You do not need to build all of these. But sketching the alternative on paper — even for 15 minutes — reveals assumptions in your original design and strengthens your understanding of why you chose what you chose.

---

## Pull Complexity Downward

When complexity must exist somewhere, push it into the lower layers of the system. The lower layers are used by many callers; the complexity is paid once and hidden from everyone. Pushing complexity upward means every caller must deal with it independently.

### The executor handles complexity so users write simple SQL

Consider a `GROUP BY` query:

```sql
SELECT department, COUNT(*), AVG(salary)
FROM employees
WHERE salary > 50000
GROUP BY department
HAVING COUNT(*) > 5
ORDER BY AVG(salary) DESC;
```

The user writes seven lines of SQL. Behind the scenes, the executor must:

1. Scan the `employees` table
2. Filter rows where `salary > 50000`
3. Group remaining rows by `department` — accumulating them in a `HashMap<Value, Vec<Row>>`
4. For each group, compute `COUNT(*)` and `AVG(salary)` — maintaining running counts and sums
5. Filter groups where `COUNT(*) > 5` (the HAVING clause)
6. Sort the surviving groups by `AVG(salary)` descending
7. Project the final columns: `department`, `COUNT(*)`, `AVG(salary)`

Seven operations. The user specified the *what*; the executor handles the *how*. All the complexity — hash table management, running aggregates, sort comparators, projection mapping — lives inside the executor. The user never sees it.

This is "pull complexity downward" in action. If the complexity were pushed upward, the user would need to write something like a program instead of a query. The relational model's power comes from pulling complexity into the query engine so that users can think declaratively.

### The optimizer pulls complexity away from the planner

The planner produces a correct but possibly inefficient plan. The optimizer takes that plan and rewrites it for efficiency — pushing filters below joins, eliminating redundant projections, choosing join algorithms. This keeps the planner simple (it translates SQL to a plan tree) and concentrates optimization logic in one place.

```rust,ignore
// The planner produces a naive plan:
// Project -> Sort -> Filter -> Scan
let plan = planner.plan(ast)?;

// The optimizer rewrites it:
// Project -> Sort -> IndexScan(with filter pushed down)
let optimized = optimizer.optimize(plan)?;
```

Without the optimizer, the planner would need to consider performance during planning — mixing correctness logic with efficiency logic. By pulling the efficiency complexity down into a separate optimizer pass, both modules become simpler.

### The Raft module pulls consensus complexity away from the server

The server handles client requests. When a write arrives, the server calls `raft.propose(command)`. It does not manage elections, count votes, track replication progress, or handle term conflicts. All of that complexity is pulled downward into the Raft module:

```rust,ignore
// Server code — simple because Raft handles the hard parts
async fn handle_write(
    &self,
    statement: Statement,
) -> Result<ResultSet, ServerError> {
    let command = serialize(&statement)?;
    self.raft.propose(command)?;   // All of consensus happens here
    let result = self.execute(statement)?;
    Ok(result)
}
```

The server is a thin layer: parse request, call Raft, execute, return response. The distributed systems complexity is invisible at this level.

### Where we pushed complexity upward (and should not have)

**Error handling at the integration layer.** In Chapter 17, the `Server` struct needed to convert between five error types: `StorageError`, `MvccError`, `ParseError`, `ExecutorError`, `RaftError`. Each conversion was a `From` implementation, but the server code was littered with error context additions:

```rust,ignore
// This pushes error-formatting complexity upward into the server
let result = self.executor.execute(plan).map_err(|e| {
    ServerError::Execution(format!("Query execution failed: {}", e))
})?;
```

A cleaner approach: define a unified `DatabaseError` type in the lowest common layer, and have all subsystems produce that type. The server would never need to convert or wrap errors — it would just propagate them with `?`. The complexity of error taxonomy would be pulled downward into the error module.

---

## General-Purpose vs Special-Purpose

Ousterhout warns against over-specializing modules. A general-purpose module serves many use cases and changes less frequently. A special-purpose module serves one use case and must change whenever that use case evolves.

### The Storage trait: general-purpose by design

The `Storage` trait does not know about SQL tables, column types, or transaction versions. It stores bytes and retrieves bytes. This generality is intentional:

```rust,ignore
pub trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete(&mut self, key: &str) -> Result<(), StorageError>;
    fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;
}
```

The trait works for:
- Storing user rows (`"table/users/1"` -> serialized row)
- Storing MVCC versions (`"table/users/1/v3"` -> versioned value)
- Storing Raft log entries (`"raft/log/42"` -> serialized command)
- Storing metadata (`"meta/schema/users"` -> table definition)

One interface, four completely different use cases. The MVCC layer, the Raft module, and the schema catalog all use the same storage engine. If you had built a `UserStorage` with `get_user(id)` and `set_user(id, user)`, you would need a separate storage interface for Raft logs, another for metadata, another for MVCC bookkeeping. Each one would duplicate I/O logic, error handling, and crash recovery.

### BitCask: special-purpose but adequate

The `LogStorage` implementation is more specialized than the trait it implements. It uses an append-only log — a design that works well for write-heavy workloads but poorly for range-heavy workloads (every range scan reads the entire index). For our database, this is adequate because:

1. Most range scans happen through the MVCC layer, which adds its own indexing
2. Our dataset fits in memory (the index is a `HashMap`)
3. We are building a learning project, not a production database

A production database would use a B-tree or LSM tree for storage. But the general-purpose `Storage` trait means you can swap `LogStorage` for a B-tree implementation without touching any other code. The general-purpose interface protects you from the special-purpose implementation.

### The Expression evaluator: correctly general-purpose

The expression evaluator in the executor handles arbitrary nested expressions:

```rust,ignore
fn evaluate(expr: &Expression, row: &Row) -> Result<Value, ExecutorError> {
    match expr {
        Expression::Literal(v) => Ok(v.clone()),
        Expression::Column(idx) => Ok(row[*idx].clone()),
        Expression::BinaryOp { op, left, right } => {
            let l = evaluate(left, row)?;
            let r = evaluate(right, row)?;
            apply_binary_op(op, l, r)
        }
        Expression::UnaryOp { op, operand } => {
            let v = evaluate(operand, row)?;
            apply_unary_op(op, v)
        }
        Expression::Function { name, args } => {
            let evaluated_args: Vec<Value> = args
                .iter()
                .map(|a| evaluate(a, row))
                .collect::<Result<_, _>>()?;
            call_function(name, evaluated_args)
        }
    }
}
```

This evaluator is general-purpose: it handles any expression tree. It does not special-case `WHERE id = 1` differently from `WHERE salary * 1.1 > threshold + bonus`. The recursive structure handles arbitrary nesting for free. If you had built special-case evaluators for common patterns (equality check, range check, null check), you would have faster evaluation for those cases but a proliferation of code paths to maintain.

The right balance: start general-purpose, then add special-case fast paths only when profiling shows they matter. Our general-purpose evaluator is correct, clear, and fast enough.

---

## Strategic vs Tactical Programming

Ousterhout draws a sharp line between **tactical** and **strategic** programming:

- **Tactical:** Get the feature working as fast as possible. Take shortcuts. Fix problems later.
- **Strategic:** Invest time in good design now to reduce total development time over the life of the project.

### Our strategic investments

Several decisions in the book were deliberately strategic — they cost more upfront but paid dividends later:

**1. The Storage trait before any implementation.**

In Chapter 2, you defined the `Storage` trait before writing `MemoryStorage`. This took 10 minutes of design time. The payoff: when Chapter 3 added `LogStorage`, you slotted it in with zero changes to the rest of the codebase. When Chapter 5 added MVCC, it composed with any storage engine automatically. The trait was a strategic investment.

**2. Comprehensive error types per layer.**

Each layer has its own error enum: `StorageError`, `ParseError`, `PlanError`, `ExecutorError`, `RaftError`. Defining these upfront took time. The payoff: when debugging fails, the error tells you exactly which layer failed and why. Compare `Error: "something went wrong"` with `ExecutorError::TypeMismatch { expected: Integer, got: String, column: "age" }`. The detailed errors save hours of debugging.

**3. The Volcano model for query execution.**

A simpler approach would have been to evaluate queries eagerly — load all matching rows into a `Vec`, filter in place, sort the `Vec`, project columns. This would have been faster to implement. The Volcano model took longer because each operator must implement `next()` with internal state management. But the payoff: lazy evaluation means you can process tables larger than memory, compose operators freely, and add new operators without modifying existing ones.

### Our tactical shortcuts

Not everything was strategic. Some decisions were deliberately tactical:

**1. `Vec<(String, Vec<u8>)>` for scan results.**

The `scan()` method returns all matching key-value pairs as a `Vec`. This is eager — it loads everything into memory at once. A strategic design would return an iterator. We chose the tactical approach because it was simpler to implement and our datasets are small. For a production database, this would be a scalability bottleneck.

**2. String-based key encoding.**

Keys like `"table/users/1/v3"` are human-readable but inefficient. A strategic design would use binary key encoding — fixed-width integers, length-prefixed strings, sort-preserving byte order. We chose string keys because they are easier to debug and understand. The cost: slightly slower comparisons and larger storage footprint.

**3. Single-threaded Raft.**

Our Raft implementation processes one message at a time. A strategic design would pipeline message processing, batch append entries, and parallel-send to followers. We chose single-threaded because it is easier to reason about correctness. The cost: lower throughput under heavy load.

Each tactical shortcut is a reasonable trade-off for a learning project. In a production system, you would address them in order of impact.

---

## Designing the Layered Architecture

Zooming out from individual principles, the most important design decision in our database was the **layered architecture** itself. Let us examine it as a whole.

### The layer cake

```
┌────────────────────────────────────────────┐
│             SQL Interface                   │
│        (Parser, Planner, Optimizer)         │
├────────────────────────────────────────────┤
│           Query Executor                    │
│        (Scan, Filter, Join, Sort)          │
├────────────────────────────────────────────┤
│       Transaction Layer (MVCC)              │
│     (begin, commit, snapshot isolation)     │
├────────────────────────────────────────────┤
│         Storage Engine                      │
│      (Memory / BitCask / B-Tree)           │
├────────────────────────────────────────────┤
│      Consensus (Raft)                       │
│  (leader election, log replication)         │
├────────────────────────────────────────────┤
│         Network Transport                   │
│       (TCP, async I/O, framing)            │
└────────────────────────────────────────────┘
```

Each layer depends only on the layer directly below it. The SQL interface does not know about file I/O. The executor does not know about Raft. MVCC does not know about TCP. These boundaries are enforced by Rust's module visibility: lower layers do not `use` upper layers.

### Why this architecture works

**1. Independent testability.** Each layer can be tested in isolation. Storage tests use `MemoryStorage` and never touch the network. Parser tests feed token streams and check ASTs. Raft tests use simulated networks. This makes tests fast and deterministic.

**2. Swappable implementations.** The `Storage` trait lets you swap `MemoryStorage` for `LogStorage` without affecting any other layer. In theory, you could swap the SQL layer for a different query language (GraphQL? A custom DSL?) without touching storage or Raft.

**3. Clear ownership.** Each layer owns its data. Storage owns file handles. MVCC owns version metadata. The executor owns row buffers. Rust's ownership model naturally enforces these boundaries — you cannot accidentally share a file handle between layers without explicit reference passing.

**4. Incremental construction.** The book builds one layer at a time, from bottom to top. Each chapter adds one layer and tests it against the layers below. This pedagogical approach works because the architecture supports it — you can build and use the storage layer before the SQL layer exists.

### Where the architecture has friction

**1. Cross-cutting concerns.** Logging, metrics, and configuration do not fit neatly into one layer. A log statement in the executor needs the transaction ID (from MVCC) and the client IP (from the network layer). This requires passing context through layers that do not otherwise care about it.

**2. The read path vs write path split.** Reads can use local storage directly. Writes must go through Raft. This split does not map to a single layer — it is a decision made in the server layer that affects how the executor calls MVCC. The layered architecture does not naturally express "some operations bypass the consensus layer."

**3. Performance overhead.** Each layer boundary is a function call, a type conversion, or a serialization step. Data flows through seven layers from SQL string to disk write. In a production database, you would add fast paths that bypass layers for common operations — but that breaks the clean layering.

---

## Tests as Design Feedback

Ousterhout does not say much about testing, but our experience building toydb reveals a deep connection between testability and design quality. **Code that is hard to test is usually poorly designed.** The difficulty of writing a test is feedback about the module's interface.

### The Storage trait made testing easy

Because every storage engine implements the same `Storage` trait, we can write tests once and run them against every implementation:

```rust,ignore
fn test_storage_set_and_get<S: Storage>(storage: &mut S) {
    storage.set("key1".into(), b"value1".to_vec()).unwrap();
    let result = storage.get("key1").unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));
}

#[test]
fn memory_storage_set_and_get() {
    let mut store = MemoryStorage::new();
    test_storage_set_and_get(&mut store);
}

#[test]
fn log_storage_set_and_get() {
    let dir = tempdir().unwrap();
    let mut store = LogStorage::new(dir.path().join("test.log")).unwrap();
    test_storage_set_and_get(&mut store);
}
```

The trait boundary is a natural testing seam. You test the contract (does `get` return what `set` stored?) not the implementation (does the file have the right bytes at the right offset?). This is the same principle as Ousterhout's deep modules: the interface is small and stable, so tests against the interface are robust.

### MVCC tests revealed a design issue

When writing MVCC tests, we discovered that testing concurrent transactions was awkward. The test needed to:

1. Begin transaction T1
2. Set a value in T1
3. Begin transaction T2
4. Read the value in T2 (should not see T1's write — T1 has not committed)
5. Commit T1
6. Read the value in T2 again (still should not see T1's write — T2's snapshot is frozen)

The awkwardness: steps 2-4 require interleaving operations from two transactions on the same storage. If the MVCC API required a mutable reference to the storage, you could not have two active transactions simultaneously (Rust's borrow checker would prevent it).

This forced a design decision: use `Arc<RwLock<Storage>>` for shared access, or use message-passing. We chose the interior mutability approach:

```rust,ignore
struct MvccStorage {
    inner: Arc<RwLock<Box<dyn Storage>>>,
    next_version: Arc<AtomicU64>,
}
```

The testing difficulty was a signal that single-threaded ownership was wrong for a concurrency layer. The test was hard to write because the API was wrong. After fixing the API, the test became straightforward:

```rust,ignore
#[test]
fn snapshot_isolation() {
    let store = MvccStorage::new(Box::new(MemoryStorage::new()));

    let mut t1 = store.begin();
    t1.set("key".into(), b"v1".to_vec()).unwrap();

    let t2 = store.begin(); // Starts BEFORE t1 commits
    assert_eq!(t2.get("key").unwrap(), None); // Does not see t1's write

    t1.commit().unwrap();

    // t2 still does not see it — snapshot is frozen
    assert_eq!(t2.get("key").unwrap(), None);

    // A NEW transaction sees the committed value
    let t3 = store.begin();
    assert_eq!(t3.get("key").unwrap(), Some(b"v1".to_vec()));
}
```

The lesson: if your test is fighting the API, the API is wrong. Fix the API, not the test.

### Raft testing required a simulated network

Testing Raft with real TCP connections would be slow (network latency), flaky (port conflicts, firewall rules), and incomplete (how do you test network partitions?). Instead, we built a simulated network:

```rust,ignore
struct SimulatedNetwork {
    nodes: HashMap<u64, RaftNode>,
    messages: VecDeque<(u64, u64, Message)>, // (from, to, message)
    partitions: HashSet<(u64, u64)>,          // blocked connections
}

impl SimulatedNetwork {
    fn deliver_one(&mut self) {
        if let Some((from, to, msg)) = self.messages.pop_front() {
            if !self.partitions.contains(&(from, to)) {
                self.nodes.get_mut(&to).unwrap().step(msg);
            }
            // If partitioned, the message is silently dropped
        }
    }

    fn partition(&mut self, a: u64, b: u64) {
        self.partitions.insert((a, b));
        self.partitions.insert((b, a));
    }

    fn heal(&mut self, a: u64, b: u64) {
        self.partitions.remove(&(a, b));
        self.partitions.remove(&(b, a));
    }
}
```

This made tests deterministic and comprehensive:

```rust,ignore
#[test]
fn leader_election_after_partition() {
    let mut net = SimulatedNetwork::new(3); // 3-node cluster

    // Elect a leader
    net.tick_until_leader();
    let leader = net.leader().unwrap();

    // Partition the leader from the other two nodes
    for &peer in &net.peers_of(leader) {
        net.partition(leader, peer);
    }

    // The remaining two nodes should elect a new leader
    net.tick_until_leader_among(&net.peers_of(leader));
    let new_leader = net.leader_among(&net.peers_of(leader)).unwrap();
    assert_ne!(new_leader, leader);

    // Heal the partition — old leader should step down
    for &peer in &net.peers_of(leader) {
        net.heal(leader, peer);
    }
    net.tick(10);
    assert_eq!(net.nodes[&leader].state(), NodeState::Follower);
}
```

This test covers a scenario that would be nearly impossible to reproduce reliably with real networking. The simulated network is the testing equivalent of a deep module: a simple interface (`deliver_one`, `partition`, `heal`) that hides the complexity of deterministic message ordering.

---

## Retrospective: What We Would Change

If you were redesigning toydb from scratch, knowing everything you know now, what would you change? This is the most valuable question a software engineer can ask. Here are five changes, ordered by impact.

### 1. Binary key encoding from day one

String keys like `"table/users/1/v3"` are readable but wasteful. Binary keys — big-endian u64 for table IDs, fixed-width row IDs, big-endian u64 for version numbers — sort correctly in byte order and use less space. The change:

```rust,ignore
// Current: string key construction scattered through MVCC
let key = format!("table/{}/{}/v{}", table_name, row_id, version);

// Redesigned: structured key with binary encoding
struct MvccKey {
    table_id: u32,
    row_id: u64,
    version: u64,
}

impl MvccKey {
    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);
        buf.extend_from_slice(&self.table_id.to_be_bytes());
        buf.extend_from_slice(&self.row_id.to_be_bytes());
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf
    }
}
```

Big-endian encoding ensures that byte-wise comparison produces the correct ordering — keys sort by table first, then row, then version. This is the approach used by CockroachDB, TiKV, and FoundationDB.

### 2. Iterator-based scans in the Storage trait

Replace `Vec<(String, Vec<u8>)>` with an iterator:

```rust,ignore
pub trait Storage {
    type ScanIter<'a>: Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a
    where
        Self: 'a;

    fn scan<'a>(&'a self, start: &[u8], end: &[u8]) -> Self::ScanIter<'a>;
}
```

This uses Rust's Generic Associated Types (GATs) — available since Rust 1.65. The iterator approach lets you scan tables larger than memory, stop early when a `LIMIT` is reached, and compose scans lazily with the Volcano executor. The `Vec` approach forces eager loading of all matching rows.

### 3. A unified error type

Instead of five error enums with `From` conversions between them, define one `DatabaseError` enum at the lowest level:

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    // Storage errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Corrupted data at offset {offset}: {message}")]
    Corruption { offset: u64, message: String },

    // SQL errors
    #[error("Parse error at position {position}: {message}")]
    Parse { position: usize, message: String },
    #[error("Unknown table: {0}")]
    UnknownTable(String),

    // MVCC errors
    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

    // Raft errors
    #[error("Not the leader; leader is node {leader_id}")]
    NotLeader { leader_id: u64 },

    // General
    #[error("{0}")]
    Internal(String),
}
```

One error type, one `?` operator, no conversions. The downside: the error type grows large and every layer can see every variant. But in practice, error handling is already cross-cutting — having one type acknowledges that reality.

### 4. Separate read and write storage interfaces

The `Storage` trait combines reads and writes. But in a replicated database, reads and writes take very different paths — writes go through Raft, reads can use local state. A cleaner design:

```rust,ignore
pub trait ReadStorage {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError>;
    fn scan(&self, start: &[u8], end: &[u8]) -> ScanIterator;
}

pub trait WriteStorage: ReadStorage {
    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), DatabaseError>;
    fn delete(&mut self, key: &[u8]) -> Result<(), DatabaseError>;
}
```

The executor's scan operators need only `ReadStorage`. The MVCC commit path needs `WriteStorage`. Raft wraps `WriteStorage` to replicate writes. The type system would enforce the read/write split — a component with only a `&dyn ReadStorage` reference cannot accidentally modify data.

### 5. A proper write-ahead log (WAL)

Our BitCask engine uses the data log itself as the recovery mechanism — on startup, it replays the entire log to rebuild the in-memory index. A production database separates the WAL from the data store:

```rust,ignore
// Current: the data log IS the recovery log
// Recovery: replay entire log from beginning
impl LogStorage {
    fn recover(&mut self) -> Result<(), StorageError> {
        let mut reader = BufReader::new(File::open(&self.path)?);
        while let Some((key, value, offset)) = self.read_next_record(&mut reader)? {
            match value {
                Some(_) => { self.index.insert(key, offset); }
                None => { self.index.remove(&key); } // tombstone
            }
        }
        Ok(())
    }
}

// Redesigned: separate WAL from data store
// WAL records operations; data store holds current state
struct WalStorage {
    wal: File,           // append-only log of operations
    data: BTreeMap<String, Vec<u8>>,  // current state
    wal_offset: u64,     // last applied WAL position
}

impl WalStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError> {
        // 1. Write to WAL first (durability)
        self.wal_append(WalEntry::Set { key: key.clone(), value: value.clone() })?;
        self.wal.sync_data()?;  // fsync — data is now durable

        // 2. Apply to in-memory data store (performance)
        self.data.insert(key, value);
        Ok(())
    }

    fn recover(&mut self) -> Result<(), StorageError> {
        // Replay only entries after the last checkpoint
        let entries = self.read_wal_from(self.wal_offset)?;
        for entry in entries {
            self.apply(entry);
        }
        Ok(())
    }
}
```

The key advantage: the WAL is append-only and sequentially written (fast), while the data store can use any structure (B-tree, LSM tree, hash map) optimized for reads. Recovery replays only the WAL entries since the last checkpoint, not the entire history. PostgreSQL, MySQL, SQLite, and every production database uses this pattern.

### 6. Batch operations for Raft

Our Raft implementation proposes one command at a time. A production system batches multiple client commands into a single Raft proposal:

```rust,ignore
// Current: one proposal per command
raft.propose(serialize(&insert_statement))?;

// Redesigned: batch multiple commands
let batch = vec![
    serialize(&insert_1)?,
    serialize(&insert_2)?,
    serialize(&insert_3)?,
];
raft.propose_batch(batch)?;
```

Batching amortizes the cost of a Raft round — one disk flush and one network round-trip for N commands instead of N flushes and N round-trips. This is the single most impactful performance improvement for a replicated database.

The implementation is straightforward: instead of calling `raft.propose()` immediately when a client request arrives, buffer requests for a short window (e.g., 1ms) and then propose them as a single batch:

```rust,ignore
struct BatchProposer {
    buffer: Vec<Vec<u8>>,
    max_batch_size: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl BatchProposer {
    fn submit(&mut self, command: Vec<u8>, raft: &mut RaftNode) -> Result<(), RaftError> {
        self.buffer.push(command);

        if self.buffer.len() >= self.max_batch_size
            || self.last_flush.elapsed() >= self.flush_interval
        {
            let batch = std::mem::take(&mut self.buffer);
            let serialized = bincode::serialize(&batch)?;
            raft.propose(serialized)?;
            self.last_flush = Instant::now();
        }
        Ok(())
    }
}
```

At 10,000 commands per second with a 1ms flush interval, each batch contains ~10 commands. This reduces Raft consensus rounds from 10,000 to 1,000 per second — a 10x improvement in throughput with at most 1ms of added latency.

### 7. Connection pooling and concurrent clients

Our client-server protocol (Chapter 12) handles one connection at a time. A production database needs connection pooling — a fixed pool of connections shared across many client requests. Without pooling, 1,000 concurrent clients create 1,000 connections, each consuming memory for buffers, transaction state, and parser state. With a pool of 50 connections, the 1,000 clients share those 50 — queueing when all are busy. This bounds memory usage and prevents connection storms from overwhelming the database.

```rust,ignore
struct ConnectionPool {
    connections: Vec<DatabaseConnection>,
    available: tokio::sync::Semaphore,
}

impl ConnectionPool {
    async fn execute(&self, query: String) -> Result<ResultSet, Error> {
        let permit = self.available.acquire().await?;
        let conn = self.checkout();
        let result = conn.execute(query).await;
        self.return_connection(conn);
        drop(permit);
        result
    }
}
```

---

## Comments Should Describe Things That Are Not Obvious

Ousterhout has strong opinions about comments: they should describe *why*, not *what*. A comment that restates the code is worse than no comment — it adds visual noise and creates a second thing to maintain. A comment that explains intent, invariants, or non-obvious behavior is invaluable.

### Good comments in our codebase

The best comments in toydb explain invariants — things the code depends on but does not enforce:

```rust,ignore
/// Append a record to the log file.
///
/// Records are written as: [key_len: u32][value_len: u32][key][value][crc32: u32]
/// A tombstone record has value_len == u32::MAX and no value bytes.
///
/// INVARIANT: The writer's position after this call equals the start offset
/// of the next record. This is critical for the index — we store the offset
/// of each record so that get() can seek directly to it.
fn append_record(&mut self, key: &str, value: Option<&[u8]>) -> Result<u64, StorageError> {
    // ...
}
```

Without this comment, a reader must reverse-engineer the record format from the write code and understand *why* the offset tracking matters. The comment saves 15 minutes of reading.

```rust,ignore
/// Returns the entries that are safe to apply to the state machine.
///
/// An entry is safe to apply if:
/// 1. Its index <= commit_index (the leader has confirmed a majority of replicas have it)
/// 2. Its index > last_applied (we have not already applied it)
///
/// WHY NOT just apply everything up to commit_index?
/// Because after a restart, we replay the log from the beginning. Entries that were
/// applied before the crash have already affected the state machine. Applying them
/// again would be incorrect (e.g., incrementing a counter twice). The last_applied
/// marker prevents this.
fn entries_to_apply(&self) -> Vec<LogEntry> {
    // ...
}
```

This comment explains a subtle correctness property that is not obvious from the code. A developer modifying the recovery logic needs to understand *why* `last_applied` exists — the code only shows *that* it is checked.

### Bad comments we should remove

```rust,ignore
// Create a new HashMap
let mut index = HashMap::new();

// Loop through all entries
for entry in entries {
    // Insert the entry
    index.insert(entry.key.clone(), entry.offset);
}
```

These comments restate the code. They add nothing. Worse, if someone changes the data structure from `HashMap` to `BTreeMap`, the comment becomes actively misleading unless someone remembers to update it.

### Comments as design documentation

The most valuable comments in our codebase are module-level documentation that describes the design rationale:

```rust,ignore
//! # MVCC — Multi-Version Concurrency Control
//!
//! This module provides snapshot isolation for concurrent transactions.
//! Instead of locking rows, it maintains multiple versions of each value.
//! Readers see a consistent snapshot; writers create new versions.
//!
//! ## Key design decisions:
//!
//! 1. **Versions are encoded in storage keys.** A value at key "users/1"
//!    with version 3 is stored as "users/1\0\0\0\0\0\0\0\x03". This means
//!    version ordering is handled by the storage engine's key ordering.
//!    Trade-off: the MVCC layer knows the key encoding scheme.
//!
//! 2. **Active transactions are tracked in storage.** The set of active
//!    transaction IDs is stored under the "_txn/" key prefix. This means
//!    snapshot visibility checks require storage reads. Trade-off: slower
//!    reads, but the active set survives crashes.
//!
//! 3. **No garbage collection yet.** Old versions accumulate forever.
//!    A production system would need a background GC process that removes
//!    versions no longer visible to any active transaction.
```

This is Ousterhout's ideal: the comment describes the design space, the decisions made, and the trade-offs accepted. A new developer reading this module starts with a mental model of the system, not just a pile of code.

---

## Consistency and Naming

Ousterhout dedicates a full chapter to naming because names are the most common form of documentation. A good name tells you what something does without reading the implementation. A bad name forces you to read the implementation to understand the name.

### Good naming in toydb

Our codebase generally follows Rust conventions, which are themselves designed for clarity:

```rust,ignore
// Types are PascalCase — clear and consistent
struct MemoryStorage { ... }
struct LogStorage { ... }
struct MvccTransaction { ... }

// Methods describe what they do
fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), StorageError>;
fn begin(&mut self) -> MvccTransaction;
fn propose(&mut self, command: Vec<u8>) -> Result<(), RaftError>;

// Enum variants describe what they are
enum NodeState { Follower, Candidate, Leader }
enum Token { Select, From, Where, Integer(i64), Ident(String) }
```

The `Storage` trait's method names — `set`, `get`, `delete`, `scan` — are deliberately simple. They are the same names every key-value store uses. A developer who has used Redis, DynamoDB, or LevelDB immediately knows what they do.

### Naming mistakes to learn from

```rust,ignore
// Too generic — what does "process" mean?
fn process(&mut self, input: &str) -> Result<Output, Error>;
// Better: fn execute_query(&mut self, sql: &str) -> Result<ResultSet, QueryError>;

// Abbreviated — save 5 characters, cost 5 minutes of confusion
fn mk_plan(ast: &Stmt) -> Pln;
// Better: fn build_plan(ast: &Statement) -> Plan;

// Boolean names that require mental negation
fn is_not_committed(&self) -> bool;
// Better: fn is_active(&self) -> bool;
// Or: fn is_committed(&self) -> bool; (caller negates if needed)
```

Ousterhout's rule: if you need a comment to explain what a name means, the name is wrong. The comment should be unnecessary because the name itself communicates the intent.

### Consistency across the codebase

The most important naming rule is consistency. If the storage layer calls its main method `set`, the MVCC layer should not call the same operation `put`, and the Raft layer should not call it `write`. Inconsistent naming forces readers to build a mental mapping table: "wait, does `put` do the same thing as `set`? Or is it different?"

In our codebase, the naming is mostly consistent:
- Storage operations: `set`, `get`, `delete`, `scan`
- Transaction operations: `begin`, `commit`, `abort` (not `rollback`)
- Raft operations: `propose`, `step`, `tick`
- Executor operations: `next` (the Volcano interface)

Each layer has its own vocabulary, and the vocabulary is consistent within the layer. This is exactly what Ousterhout recommends.

---

## Complexity Budget

A useful mental model: every project has a fixed **complexity budget**. Each design decision spends some of that budget. Strategic decisions spend complexity where it matters (deep modules, clean abstractions). Tactical decisions waste complexity on shortcuts that create long-term maintenance costs.

### Where we spent our budget wisely

| Investment | Complexity Cost | Value Returned |
|-----------|----------------|----------------|
| `Storage` trait abstraction | Low (4 methods, 1 trait) | High (swappable engines, testability) |
| MVCC snapshot isolation | High (version chains, active sets) | High (concurrent reads without locks) |
| Volcano executor model | Medium (per-operator state) | High (composable, lazy, memory-efficient) |
| Raft state machine | High (election, replication) | High (fault tolerance, durability) |
| CRC32 checksums | Low (one function call per record) | Medium (data integrity verification) |

### Where we overspent

| Decision | Complexity Cost | Value Returned |
|---------|----------------|----------------|
| String-based key encoding | Medium (format strings, parsing, validation) | Low (readable but fragile) |
| Separate error type per layer | Medium (5 error enums, 10+ From impls) | Medium (clear attribution but verbose) |
| Hand-written parser | High (400+ lines) | Medium (educational but a parser combinator would work) |

### Where we underspent

| Area | Complexity Saved | Risk Accepted |
|------|-----------------|---------------|
| No garbage collection for MVCC | Significant | Old versions accumulate forever |
| No log compaction in Raft | Significant | Unbounded log growth |
| No connection pooling | Low | Cannot handle concurrent clients |
| No write-ahead log | Moderate | Recovery replays entire BitCask log |

The underspent areas are explicit trade-offs for a learning project. In a production system, each one would be prioritized based on the specific workload and failure requirements.

---

## The Meta-Lesson

Ousterhout's principles are not rules — they are lenses. Each one reveals something different about the same codebase:

- **Complexity is incremental** reminds you to watch for accumulated design debt.
- **Deep modules** tells you where to invest in interface design.
- **Information hiding** shows you where to draw boundaries.
- **Define errors out of existence** challenges you to rethink APIs instead of adding error handling.
- **Design it twice** forces you to compare alternatives before committing.
- **Pull complexity downward** tells you which layer should own the hard work.
- **General-purpose vs special-purpose** guides you on how specific to make each module.
- **Strategic vs tactical** frames every shortcut as a conscious trade-off.

No single principle would have led to our architecture. Together, they produce a system where each layer is deep, each boundary hides information, errors are minimized, and complexity lives where it causes the least harm.

You built a database. That is a significant engineering achievement. But building it is not the point — understanding why it works, where it struggles, and how you would improve it is the real skill. That understanding is what separates a programmer who can follow a tutorial from an engineer who can design systems.

---

## Summary

| Principle | Our Best Example | What We Would Change |
|-----------|-----------------|---------------------|
| Complexity is incremental | The write path crosses 9 layers | Binary key encoding to simplify MVCC |
| Deep modules | Storage trait: 4 methods, 300+ lines hidden | Iterator-based scans for better composability |
| Information hiding | MVCC hides versions from SQL | Fix key encoding leakage |
| Define errors out of existence | Tombstones, default predicates | Unified error type |
| Design it twice | Memory vs BitCask storage | Consider parser combinator alternative |
| Pull complexity downward | Executor handles GROUP BY complexity | Unified error type in lowest layer |
| General-purpose vs special-purpose | Storage trait works for MVCC, Raft, schema | Separate read/write interfaces |
| Strategic vs tactical | Trait-first design, Volcano model | Batch Raft proposals |

The database you built is not perfect. No software is. But it is well-designed — because the design decisions were intentional, the trade-offs were conscious, and the architecture reflects principles that have stood the test of decades of software engineering practice.
