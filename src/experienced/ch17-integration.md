# Chapter 17: Integration — SQL over Raft

You have built a SQL parser, a query planner, an optimizer, an executor, an MVCC storage engine, a client-server protocol, and a Raft consensus layer. Each piece works in isolation. But a database is not a collection of parts — it is a system where a user types `INSERT INTO users VALUES (1, 'Alice')` and the data appears on three machines, survives crashes, and is immediately visible to subsequent queries. This chapter connects every layer into a single executable that does exactly that.

The spotlight concept is **module system and workspace** — how Rust organizes code into modules, crates, and workspaces, and how visibility rules (`pub`, `pub(crate)`, private) enforce the boundaries between layers that we have carefully maintained throughout this book.

By the end of this chapter, you will have:

- A clear map of the full request path: SQL string to parsed AST to planned operations to optimized plan to executed results to replicated state to client response
- A `Server` struct that wires together all layers with proper ownership
- Separate read and write paths (writes go through Raft, reads can use local state)
- A Rust workspace with separate crates for storage, SQL, Raft, and server
- Integration tests that execute SQL queries end-to-end through the complete stack
- Error propagation across layer boundaries using Rust's `From` trait and the `?` operator

---

## Spotlight: Module System & Workspace

Every chapter has one spotlight concept. This chapter's spotlight is **module system and workspace** — how Rust organizes code at every scale, from a single function's visibility to a multi-crate project.

### Modules: organizing code within a crate

A Rust crate is a compilation unit — the smallest thing the compiler processes as a whole. Within a crate, **modules** organize code into namespaces:

```rust,ignore
// src/lib.rs

mod storage;     // loads from src/storage.rs or src/storage/mod.rs
mod sql;         // loads from src/sql.rs or src/sql/mod.rs
mod raft;        // loads from src/raft.rs or src/raft/mod.rs
mod server;      // loads from src/server.rs or src/server/mod.rs
```

Each `mod` declaration creates a namespace and tells the compiler to include that file. Modules can be nested:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;      // src/sql/lexer.rs
pub mod parser;     // src/sql/parser.rs
pub mod planner;    // src/sql/planner.rs
pub mod optimizer;  // src/sql/optimizer.rs
pub mod executor;   // src/sql/executor.rs
```

The file system structure mirrors the module hierarchy. This is not a convention — it is a rule. The compiler looks for `src/sql/lexer.rs` because the module path is `sql::lexer`.

### Visibility: pub, pub(crate), and private

Rust defaults to private. Everything is hidden unless you explicitly expose it:

```rust,ignore
// src/sql/parser.rs

pub struct Parser {           // visible to anyone who can see the `sql::parser` module
    tokens: Vec<Token>,       // PRIVATE — only Parser's own methods can access this
    position: usize,          // PRIVATE
}

pub(crate) fn validate_ast(  // visible within this crate, but not to external crates
    ast: &Statement,
) -> Result<(), String> {
    // ...
}

fn consume_token(            // PRIVATE — only functions in this module can call this
    tokens: &[Token],
    pos: &mut usize,
) -> Option<&Token> {
    // ...
}
```

Three levels:
- **Private** (no keyword): visible only within the same module and its children
- **`pub(crate)`**: visible anywhere in the same crate, but not exported to dependents
- **`pub`**: visible to anyone, including external crates

This maps directly to our database layers. The parser's internal state (`tokens`, `position`) is private — no one outside the parser needs to know about token positions. The `validate_ast` function is `pub(crate)` — the server uses it, but external users of our library should not. The `Parser` struct and its `parse()` method are `pub` — they are the public API.

### The `use` keyword: bringing names into scope

Without `use`, you write full paths everywhere:

```rust,ignore
// Verbose — every type fully qualified
fn execute(
    plan: crate::sql::planner::Plan,
    storage: &mut crate::storage::mvcc::MvccStorage,
) -> crate::sql::executor::ResultSet {
    // ...
}
```

With `use`, you import names into the current scope:

```rust,ignore
use crate::sql::planner::Plan;
use crate::sql::executor::ResultSet;
use crate::storage::mvcc::MvccStorage;

fn execute(
    plan: Plan,
    storage: &mut MvccStorage,
) -> ResultSet {
    // ...
}
```

The convention: import types (structs, enums, traits) directly. Import functions through their parent module to avoid ambiguity:

```rust,ignore
use std::io;            // then use: io::Read, io::Write
use std::io::BufReader; // type imported directly
use std::collections::HashMap; // type imported directly
```

### Workspaces: multiple crates in one repository

As a project grows, a single crate becomes unwieldy. Compile times increase because any change recompiles everything. Testing is slower. Dependencies are shared when they should not be.

A **workspace** splits the project into multiple crates that live in the same repository:

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "toydb-storage",
    "toydb-sql",
    "toydb-raft",
    "toydb-server",
]
```

Each member is an independent crate with its own `Cargo.toml`, `src/`, and tests:

```
toydb/
├── Cargo.toml          (workspace root)
├── toydb-storage/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── kv.rs
│       ├── bitcask.rs
│       └── mvcc.rs
├── toydb-sql/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── lexer.rs
│       ├── parser.rs
│       ├── planner.rs
│       ├── optimizer.rs
│       └── executor.rs
├── toydb-raft/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── node.rs
│       ├── wal.rs
│       ├── state.rs
│       └── snapshot.rs
└── toydb-server/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── server.rs
        └── client.rs
```

Crates in a workspace can depend on each other:

```toml
# toydb-server/Cargo.toml
[dependencies]
toydb-storage = { path = "../toydb-storage" }
toydb-sql = { path = "../toydb-sql" }
toydb-raft = { path = "../toydb-raft" }
```

The dependency graph enforces layer boundaries: `toydb-sql` does not depend on `toydb-raft`, so SQL code cannot accidentally call Raft functions. This is information hiding enforced by the build system.

### Re-exports: simplifying the public API

A workspace crate might have deep module paths. Re-exports flatten them for consumers:

```rust,ignore
// toydb-sql/src/lib.rs

pub mod lexer;
pub mod parser;
pub mod planner;
pub mod optimizer;
pub mod executor;

// Re-export the most commonly used types at the crate root
pub use parser::Parser;
pub use planner::{Plan, Planner};
pub use executor::{Executor, ResultSet};
pub use lexer::Lexer;
```

Now `toydb-server` can write `use toydb_sql::Parser` instead of `use toydb_sql::parser::Parser`. The internal module structure is an implementation detail hidden behind convenient re-exports.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Module | ES module (file) | Module (file) | Package (directory) | Module (file or directory) |
> | Import | `import { X } from './x'` | `from x import X` | `import "pkg"` | `use crate::x::X` |
> | Private | No enforcement (convention: `_`) | Convention: `_prefix` | Lowercase first letter | Default (no keyword) |
> | Public | `export` keyword | No enforcement | Uppercase first letter | `pub` keyword |
> | Workspace | npm workspaces / yarn | Not built-in (setuptools) | Go modules | Cargo workspace |
> | Re-export | `export { X } from './x'` | `from x import X` in `__init__.py` | Not needed (package = directory) | `pub use x::X` |
>
> Go's approach is closest to Rust's: packages are directories, visibility is controlled by case. But Go has only two levels (exported/unexported), while Rust has three (pub/pub(crate)/private). And Go's package system is based on directories, while Rust's module system can nest arbitrarily within a single file.
>
> The biggest difference from JavaScript and Python: Rust's visibility rules are enforced by the compiler. In JS, "private" is a naming convention (or the relatively new `#private` syntax). In Python, `_private` is a suggestion that tools and people routinely ignore. In Rust, if a field is not `pub`, external code literally cannot access it — the program will not compile.

---

## The Big Picture: Request Flow

Before writing code, let us trace a SQL query through every layer. Understanding the full path is essential for designing the integration points.

A client sends: `INSERT INTO users (id, name) VALUES (1, 'Alice')`

```
Client                          Server
  │                               │
  │  TCP: send SQL string         │
  │──────────────────────────────>│
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 1. Protocol Layer   │
  │                    │    Deserialize       │
  │                    │    Request::Query    │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 2. Lexer            │
  │                    │    SQL → Tokens     │
  │                    │    [INSERT, INTO,   │
  │                    │     users, ...]     │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 3. Parser           │
  │                    │    Tokens → AST     │
  │                    │    Statement::      │
  │                    │    Insert { ... }   │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 4. Planner          │
  │                    │    AST → Plan       │
  │                    │    Plan::Insert {   │
  │                    │      table, values  │
  │                    │    }                │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 5. Optimizer        │
  │                    │    Plan → Plan      │
  │                    │    (no-op for       │
  │                    │     INSERT)         │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 6. Raft (replicate) │
  │                    │    Serialize plan,  │
  │                    │    append to log,   │
  │                    │    replicate to     │
  │                    │    followers, wait  │
  │                    │    for majority     │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 7. Executor         │
  │                    │    (after commit)   │
  │                    │    Apply the INSERT │
  │                    │    to MVCC storage  │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 8. MVCC Storage     │
  │                    │    Begin txn,       │
  │                    │    write row,       │
  │                    │    commit txn       │
  │                    └──────────┬──────────┘
  │                               │
  │                    ┌──────────▼──────────┐
  │                    │ 9. Protocol Layer   │
  │                    │    Serialize        │
  │                    │    Response::Ok     │
  │                    └──────────┬──────────┘
  │                               │
  │  TCP: receive response        │
  │<──────────────────────────────│
```

Nine layers. Each one takes input from the layer above and produces output for the layer below. The interfaces between layers are the types we defined in earlier chapters: `Token`, `Statement`, `Plan`, `LogEntry`, `Row`, `Response`.

### Write path vs read path

The flow above is the **write path** — it goes through Raft for replication. The **read path** is shorter:

```
SELECT * FROM users WHERE id = 1

1. Protocol: deserialize request
2. Lexer: tokenize
3. Parser: parse to AST
4. Planner: create Plan::Select
5. Optimizer: push down filters, constant fold
6. Executor: scan MVCC storage, apply filters
7. MVCC Storage: read rows at current snapshot
8. Protocol: serialize response

Notice: no Raft. Reads go directly to local storage.
```

This works because Raft guarantees that all committed entries have been applied to the state machine (MVCC storage) on the leader. If we only serve reads from the leader, the local storage is always up-to-date.

But there is a subtlety: how does the leader know it is still the leader? It might have been deposed by a network partition and not know it yet. A stale leader serving reads could return stale data. The solution is a **read lease** — the leader periodically confirms its leadership by exchanging heartbeats with a majority. As long as the lease has not expired, reads from local storage are safe.

---

## Exercise 1: The Server Struct

**Goal:** Build a `Server` struct that owns all the layers and wires them together. Handle both read and write queries through the appropriate path.

### Step 1: Define the layer ownership

The server owns every layer. Each layer owns its dependencies. The ownership hierarchy is:

```rust,ignore
// src/server.rs

use toydb_sql::{Lexer, Parser, Planner, Optimizer, Executor, ResultSet, Plan};
use toydb_storage::MvccStorage;
use toydb_raft::RaftNode;
use crate::protocol::{Request, Response};

/// The database server. Owns all layers and coordinates query execution.
///
/// Ownership hierarchy:
///   Server
///   ├── RaftNode (owns WAL, state persister, snapshot store)
///   ├── MvccStorage (owns the underlying KV store)
///   ├── Planner (stateless — borrows schema from storage)
///   └── Optimizer (stateless)
pub struct Server {
    /// The Raft consensus node. Handles replication and leader election.
    raft: RaftNode,
    /// The MVCC storage engine. Provides transactional reads and writes.
    storage: MvccStorage,
    /// Server configuration.
    config: ServerConfig,
}

pub struct ServerConfig {
    pub listen_addr: String,
    pub node_id: u64,
    pub data_dir: String,
    pub peers: Vec<String>,
}
```

Why does the `Server` own `RaftNode` and `MvccStorage` separately, rather than having `RaftNode` own the storage? Because the read path bypasses Raft entirely. If `RaftNode` owned the storage, every read would have to go through the Raft layer, which is unnecessary overhead. By keeping them as siblings, the server can route reads directly to storage and writes through Raft.

### Step 2: Implement the query router

```rust,ignore
// src/server.rs (continued)

impl Server {
    pub fn new(config: ServerConfig) -> Result<Self, String> {
        let storage = MvccStorage::new(&config.data_dir)?;
        let raft = RaftNode::new(config.node_id, &config.data_dir)?;

        Ok(Server {
            raft,
            storage,
            config,
        })
    }

    /// Execute a SQL query. Routes writes through Raft, reads directly
    /// to storage.
    pub fn execute(&mut self, sql: &str) -> Response {
        // Step 1: Lex
        let tokens = match Lexer::new(sql).tokenize() {
            Ok(tokens) => tokens,
            Err(e) => return Response::Error {
                message: format!("Lexer error: {}", e),
            },
        };

        // Step 2: Parse
        let statement = match Parser::new(tokens).parse() {
            Ok(stmt) => stmt,
            Err(e) => return Response::Error {
                message: format!("Parse error: {}", e),
            },
        };

        // Step 3: Plan
        let plan = match Planner::new(&self.storage).plan(statement) {
            Ok(plan) => plan,
            Err(e) => return Response::Error {
                message: format!("Planner error: {}", e),
            },
        };

        // Step 4: Optimize
        let plan = Optimizer::optimize(plan);

        // Step 5: Route based on read/write
        if plan.is_read_only() {
            self.execute_read(plan)
        } else {
            self.execute_write(plan)
        }
    }

    /// Execute a read query directly against local storage.
    fn execute_read(&mut self, plan: Plan) -> Response {
        // Verify we are the leader (or have a valid read lease)
        if !self.raft.is_leader() {
            return Response::Error {
                message: format!(
                    "not the leader; redirect to {}",
                    self.raft.leader_addr().unwrap_or("unknown".to_string())
                ),
            };
        }

        match Executor::new(&mut self.storage).execute(plan) {
            Ok(result_set) => self.result_set_to_response(result_set),
            Err(e) => Response::Error {
                message: format!("Execution error: {}", e),
            },
        }
    }

    /// Execute a write query through Raft consensus.
    fn execute_write(&mut self, plan: Plan) -> Response {
        // Verify we are the leader
        if !self.raft.is_leader() {
            return Response::Error {
                message: format!(
                    "not the leader; redirect to {}",
                    self.raft.leader_addr().unwrap_or("unknown".to_string())
                ),
            };
        }

        // Serialize the plan and propose it to Raft
        let command = match plan.serialize() {
            Ok(bytes) => bytes,
            Err(e) => return Response::Error {
                message: format!("Serialization error: {}", e),
            },
        };

        // Replicate through Raft — blocks until committed by majority
        match self.raft.propose(command) {
            Ok(()) => {
                // Entry committed — now execute against local storage
                match Executor::new(&mut self.storage).execute(plan) {
                    Ok(result_set) => self.result_set_to_response(result_set),
                    Err(e) => Response::Error {
                        message: format!("Execution error: {}", e),
                    },
                }
            }
            Err(e) => Response::Error {
                message: format!("Replication error: {}", e),
            },
        }
    }

    fn result_set_to_response(&self, result_set: ResultSet) -> Response {
        match result_set {
            ResultSet::Rows { columns, rows } => {
                let string_rows: Vec<Vec<String>> = rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|v| v.to_string()).collect())
                    .collect();
                Response::Rows {
                    columns,
                    rows: string_rows,
                }
            }
            ResultSet::Modified { count } => Response::Ok {
                message: format!("{} row(s) affected", count),
            },
            ResultSet::Created { name } => Response::Ok {
                message: format!("Table '{}' created", name),
            },
        }
    }
}
```

### Step 3: Classifying read vs write queries

The `Plan::is_read_only()` method determines the routing:

```rust,ignore
// In toydb-sql/src/planner.rs

impl Plan {
    /// Returns true if this plan only reads data (no mutations).
    pub fn is_read_only(&self) -> bool {
        match self {
            Plan::Select { .. } => true,
            Plan::Insert { .. } => false,
            Plan::Update { .. } => false,
            Plan::Delete { .. } => false,
            Plan::CreateTable { .. } => false,
            Plan::DropTable { .. } => false,
        }
    }

    /// Serialize the plan to bytes for Raft replication.
    pub fn serialize(&self) -> Result<Vec<u8>, String> {
        // In a real implementation, you would use serde or a custom
        // binary format. For simplicity, we serialize the original
        // SQL string.
        Ok(self.original_sql().as_bytes().to_vec())
    }
}
```

An important design decision: what do we replicate through Raft? We have two choices:

1. **Replicate the SQL string.** Each follower independently lexes, parses, plans, optimizes, and executes the SQL. This is what we show above.
2. **Replicate the execution plan.** The leader does the parsing and planning, and followers just execute the plan.
3. **Replicate the storage mutations.** The leader executes the query and replicates the raw key-value writes.

Option 1 is simplest but has a risk: if the parser or planner has non-deterministic behavior (e.g., using `DEFAULT` values with `NOW()`), different nodes might produce different results. Option 3 is most robust but generates more data to replicate. CockroachDB uses option 2 (replicate evaluated commands). TiDB uses option 3 (replicate raw Raft log entries that encode storage operations). Our implementation uses option 1 for simplicity, accepting the non-determinism risk since our SQL engine does not have functions like `NOW()`.

### Step 4: The state machine interface

Raft's state machine is the bridge between consensus and storage. When an entry is committed (acknowledged by a majority), the state machine applies it:

```rust,ignore
// src/state_machine.rs

use toydb_sql::{Lexer, Parser, Planner, Optimizer, Executor, ResultSet};
use toydb_storage::MvccStorage;

/// The database state machine. Applies committed Raft entries
/// to the MVCC storage engine.
///
/// This implements the StateMachine trait from the raft module.
pub struct SqlStateMachine {
    storage: MvccStorage,
}

impl SqlStateMachine {
    pub fn new(storage: MvccStorage) -> Self {
        SqlStateMachine { storage }
    }

    /// Apply a committed entry. The entry is a serialized SQL command
    /// that has been agreed upon by the Raft cluster.
    pub fn apply(&mut self, command: &[u8]) -> Result<(), String> {
        let sql = std::str::from_utf8(command)
            .map_err(|e| format!("invalid UTF-8 in command: {}", e))?;

        let tokens = Lexer::new(sql).tokenize()
            .map_err(|e| format!("lex error in apply: {}", e))?;
        let statement = Parser::new(tokens).parse()
            .map_err(|e| format!("parse error in apply: {}", e))?;
        let plan = Planner::new(&self.storage).plan(statement)
            .map_err(|e| format!("plan error in apply: {}", e))?;
        let plan = Optimizer::optimize(plan);

        Executor::new(&mut self.storage).execute(plan)
            .map_err(|e| format!("execute error in apply: {}", e))?;

        Ok(())
    }

    /// Serialize the storage state for snapshotting.
    pub fn snapshot(&self) -> Result<Vec<u8>, String> {
        self.storage.serialize()
    }

    /// Restore storage state from a snapshot.
    pub fn restore(&mut self, data: &[u8]) -> Result<(), String> {
        self.storage.restore(data)
    }
}
```

The `apply` method is called by Raft when an entry is committed. It re-parses and re-executes the SQL — this is the "replicate SQL string" approach. On the leader, this means the SQL is parsed and executed twice (once for planning/routing, once for application). This is wasteful but simple. A production system would cache the plan from the first parse.

---

## Exercise 2: Error Propagation Across Layers

**Goal:** Design an error type hierarchy that allows errors from any layer to propagate to the client with useful context, using Rust's `From` trait and `?` operator.

### Step 1: The problem

Each layer has its own error type:

```rust,ignore
// Storage errors
enum StorageError {
    KeyNotFound(String),
    WriteConflict,
    IoError(std::io::Error),
}

// SQL errors
enum SqlError {
    LexError(String),
    ParseError(String),
    PlanError(String),
    ExecutionError(String),
}

// Raft errors
enum RaftError {
    NotLeader(Option<String>),  // contains leader address
    ReplicationFailed(String),
    Timeout,
}
```

The server needs to handle all three. Without a unifying type, you end up with nested `match` expressions:

```rust,ignore
// Ugly: manual error conversion everywhere
match raft.propose(command) {
    Ok(()) => {
        match executor.execute(plan) {
            Ok(result) => {
                match storage.commit() {
                    Ok(()) => Response::Ok { ... },
                    Err(StorageError::WriteConflict) => Response::Error { ... },
                    Err(e) => Response::Error { ... },
                }
            }
            Err(e) => Response::Error { ... },
        }
    }
    Err(e) => Response::Error { ... },
}
```

### Step 2: Define a unified error type

```rust,ignore
// src/error.rs

use std::fmt;

/// Unified error type for the database server.
/// Wraps errors from all layers with context about where they occurred.
#[derive(Debug)]
pub enum DbError {
    /// SQL parsing or planning error (user's fault — bad SQL)
    Sql(String),
    /// Storage engine error (system fault — I/O, conflicts)
    Storage(String),
    /// Raft consensus error (cluster issue — not leader, timeout)
    Raft(RaftErrorKind),
    /// Internal error (bug — should not happen)
    Internal(String),
}

#[derive(Debug)]
pub enum RaftErrorKind {
    NotLeader { leader_addr: Option<String> },
    ReplicationFailed(String),
    Timeout,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sql(msg) => write!(f, "SQL error: {}", msg),
            DbError::Storage(msg) => write!(f, "Storage error: {}", msg),
            DbError::Raft(kind) => match kind {
                RaftErrorKind::NotLeader { leader_addr } => {
                    match leader_addr {
                        Some(addr) => write!(f, "not the leader; try {}", addr),
                        None => write!(f, "not the leader; leader unknown"),
                    }
                }
                RaftErrorKind::ReplicationFailed(msg) => {
                    write!(f, "replication failed: {}", msg)
                }
                RaftErrorKind::Timeout => write!(f, "request timed out"),
            },
            DbError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}
```

### Step 3: Implement From conversions

The `From` trait enables the `?` operator to automatically convert errors:

```rust,ignore
// src/error.rs (continued)

impl From<SqlError> for DbError {
    fn from(e: SqlError) -> Self {
        match e {
            SqlError::LexError(msg) => DbError::Sql(format!("lex: {}", msg)),
            SqlError::ParseError(msg) => DbError::Sql(format!("parse: {}", msg)),
            SqlError::PlanError(msg) => DbError::Sql(format!("plan: {}", msg)),
            SqlError::ExecutionError(msg) => DbError::Sql(format!("exec: {}", msg)),
        }
    }
}

impl From<StorageError> for DbError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::KeyNotFound(key) => {
                DbError::Storage(format!("key not found: {}", key))
            }
            StorageError::WriteConflict => {
                DbError::Storage("write conflict — retry the transaction".to_string())
            }
            StorageError::IoError(e) => {
                DbError::Storage(format!("I/O: {}", e))
            }
        }
    }
}

impl From<RaftError> for DbError {
    fn from(e: RaftError) -> Self {
        match e {
            RaftError::NotLeader(addr) => DbError::Raft(
                RaftErrorKind::NotLeader { leader_addr: addr }
            ),
            RaftError::ReplicationFailed(msg) => DbError::Raft(
                RaftErrorKind::ReplicationFailed(msg)
            ),
            RaftError::Timeout => DbError::Raft(RaftErrorKind::Timeout),
        }
    }
}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        DbError::Internal(format!("I/O: {}", e))
    }
}
```

### Step 4: Clean error propagation with ?

Now the server code is clean — the `?` operator handles all conversions:

```rust,ignore
impl Server {
    pub fn execute(&mut self, sql: &str) -> Result<ResultSet, DbError> {
        let tokens = Lexer::new(sql).tokenize()?;   // SqlError → DbError
        let statement = Parser::new(tokens).parse()?; // SqlError → DbError
        let plan = Planner::new(&self.storage).plan(statement)?; // SqlError → DbError
        let plan = Optimizer::optimize(plan);

        if plan.is_read_only() {
            self.verify_leader()?;                    // RaftError → DbError
            let result = Executor::new(&mut self.storage)
                .execute(plan)?;                      // SqlError → DbError
            Ok(result)
        } else {
            self.verify_leader()?;                    // RaftError → DbError
            let command = plan.serialize()?;           // SqlError → DbError
            self.raft.propose(command)?;               // RaftError → DbError
            let result = Executor::new(&mut self.storage)
                .execute(plan)?;                      // SqlError/StorageError → DbError
            Ok(result)
        }
    }

    fn verify_leader(&self) -> Result<(), DbError> {
        if self.raft.is_leader() {
            Ok(())
        } else {
            Err(DbError::Raft(RaftErrorKind::NotLeader {
                leader_addr: self.raft.leader_addr(),
            }))
        }
    }
}
```

Compare this to the nested `match` version. The `?` operator and `From` implementations reduce 30 lines of error handling to 5 question marks. Each `?` is a potential exit point — if the call returns `Err`, the function returns immediately with the converted error. The compiler verifies that every `From` implementation exists, so you cannot forget to handle an error type.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Error type | `Error` class (or any thrown value) | Exception classes | `error` interface | `enum` + `From` trait |
> | Error propagation | `throw` / `try-catch` | `raise` / `try-except` | `if err != nil { return err }` | `?` operator |
> | Error wrapping | `new Error('msg', { cause: e })` | `raise X from e` | `fmt.Errorf("ctx: %w", err)` | `From` trait |
> | Checked errors | No (runtime only) | No (runtime only) | No (convention only) | Yes (compile-time) |
>
> Go and Rust both use return values for errors (no exceptions). But Go's `if err != nil { return err }` is a runtime pattern — the compiler does not enforce it. You can ignore an error by not checking the return value. In Rust, `Result<T, E>` must be used — if you call a function returning `Result` and do not handle it, the compiler warns you. The `?` operator is syntactic sugar for Go's `if err != nil` pattern, but it is type-checked and composable.

---

## Exercise 3: Configuration and Startup

**Goal:** Build the startup sequence that initializes all layers, recovers state from disk, connects to peers, and begins serving queries.

### Step 1: Configuration

```rust,ignore
// src/config.rs

use std::path::PathBuf;

/// Server configuration, typically loaded from a YAML file or
/// command-line arguments.
#[derive(Debug, Clone)]
pub struct Config {
    /// Unique node identifier (1, 2, 3, etc.)
    pub node_id: u64,
    /// Address to listen for client connections
    pub listen_addr: String,
    /// Address to listen for Raft peer connections
    pub raft_addr: String,
    /// Directory for persistent data (WAL, snapshots, state)
    pub data_dir: PathBuf,
    /// Addresses of all Raft peers (including this node)
    pub peers: Vec<PeerConfig>,
    /// Raft election timeout range in milliseconds
    pub election_timeout_min: u64,
    pub election_timeout_max: u64,
    /// Raft heartbeat interval in milliseconds
    pub heartbeat_interval: u64,
    /// Maximum log entries before triggering a snapshot
    pub snapshot_threshold: u64,
}

#[derive(Debug, Clone)]
pub struct PeerConfig {
    pub id: u64,
    pub raft_addr: String,
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &str) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config: {}", e))?;
        Self::parse_toml(&contents)
    }

    /// Parse a TOML configuration string.
    fn parse_toml(contents: &str) -> Result<Self, String> {
        // Simple line-by-line parser for our known fields.
        // A production system would use the `toml` crate.
        let mut config = Config {
            node_id: 0,
            listen_addr: "127.0.0.1:4000".to_string(),
            raft_addr: "127.0.0.1:5000".to_string(),
            data_dir: PathBuf::from("data"),
            peers: Vec::new(),
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
            snapshot_threshold: 10_000,
        };

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match key {
                    "node_id" => {
                        config.node_id = value.parse()
                            .map_err(|_| format!("invalid node_id: {}", value))?;
                    }
                    "listen_addr" => config.listen_addr = value.to_string(),
                    "raft_addr" => config.raft_addr = value.to_string(),
                    "data_dir" => config.data_dir = PathBuf::from(value),
                    _ => {} // ignore unknown keys
                }
            }
        }

        if config.node_id == 0 {
            return Err("node_id must be set and non-zero".to_string());
        }

        Ok(config)
    }
}
```

### Step 2: The startup sequence

```rust,ignore
// src/main.rs

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: toydb <config-file>");
        std::process::exit(1);
    }

    let config = match Config::from_file(&args[1]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };

    println!("Starting toydb node {} ...", config.node_id);
    println!("  Client address: {}", config.listen_addr);
    println!("  Raft address:   {}", config.raft_addr);
    println!("  Data directory: {}", config.data_dir.display());
    println!("  Peers: {:?}", config.peers);

    // Initialize the server
    let mut server = match Server::new_from_config(config.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Server initialization failed: {}", e);
            std::process::exit(1);
        }
    };

    println!("Recovery complete. Listening for connections...");

    // Start accepting client connections
    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

### Step 3: Server initialization with recovery

```rust,ignore
// src/server.rs (continued)

impl Server {
    pub fn new_from_config(config: Config) -> Result<Self, DbError> {
        // Step 1: Create data directory
        std::fs::create_dir_all(&config.data_dir)?;

        // Step 2: Recover Raft state from disk
        let recovery = RecoveryManager::new(&config.data_dir);
        let recovered = recovery.recover()
            .map_err(|e| DbError::Internal(e))?;

        println!(
            "  Raft state: term={}, voted_for={:?}, log_entries={}, commit_index={}",
            recovered.state.current_term,
            recovered.state.voted_for,
            recovered.log.len(),
            recovered.state.commit_index,
        );

        // Step 3: Initialize MVCC storage
        let storage_path = config.data_dir.join("storage");
        let storage = MvccStorage::open(&storage_path)
            .map_err(|e| DbError::Storage(format!("storage init: {}", e)))?;

        // Step 4: Check for and apply any snapshot
        let snapshot_dir = config.data_dir.join("snapshots");
        let snapshot_store = SnapshotStore::new(&snapshot_dir)?;

        let mut state_machine = SqlStateMachine::new(storage);

        if let Some(snapshot) = snapshot_store.load_latest()
            .map_err(|e| DbError::Internal(e))?
        {
            println!(
                "  Restoring snapshot at index {}",
                snapshot.last_included_index,
            );
            state_machine.restore(&snapshot.data)
                .map_err(|e| DbError::Internal(e))?;
        }

        // Step 5: Replay committed WAL entries after the snapshot
        let replay_start = recovered.state.commit_index;
        let mut replayed = 0u64;
        for entry in &recovered.log {
            if entry.index <= replay_start {
                continue; // already applied via snapshot
            }
            state_machine.apply(&entry.command)
                .map_err(|e| DbError::Internal(
                    format!("replay failed at index {}: {}", entry.index, e)
                ))?;
            replayed += 1;
        }
        if replayed > 0 {
            println!("  Replayed {} committed entries", replayed);
        }

        // Step 6: Build the Raft node
        let raft = RaftNode::from_recovered(
            config.node_id,
            recovered,
            &config,
        ).map_err(|e| DbError::Internal(e))?;

        println!("  Initialization complete");

        Ok(Server {
            raft,
            storage: state_machine.into_storage(),
            config,
        })
    }

    pub fn run(&mut self) -> Result<(), DbError> {
        let listener = std::net::TcpListener::bind(&self.config.listen_addr)?;
        println!("Listening on {}", self.config.listen_addr);

        for stream in listener.incoming() {
            let stream = stream?;
            println!("Client connected: {}", stream.peer_addr()?);
            self.handle_connection(stream)?;
        }

        Ok(())
    }

    fn handle_connection(
        &mut self,
        stream: std::net::TcpStream,
    ) -> Result<(), DbError> {
        use crate::protocol::{read_message, write_message};
        use std::io::{BufReader, BufWriter};

        let reader_stream = stream.try_clone()?;
        let mut reader = BufReader::new(reader_stream);
        let mut writer = BufWriter::new(stream);

        loop {
            let request = match read_message(&mut reader) {
                Ok(req) => req,
                Err(_) => break, // client disconnected
            };

            let response = match request {
                Request::Query(sql) => self.execute(&sql),
                Request::Disconnect => break,
            };

            let response = match response {
                Ok(result_set) => self.result_set_to_response(result_set),
                Err(e) => Response::Error {
                    message: e.to_string(),
                },
            };

            write_message(&mut writer, &response)
                .map_err(|e| DbError::Internal(
                    format!("write response failed: {}", e)
                ))?;
        }

        Ok(())
    }
}
```

The startup sequence has a clear order: config, recovery, storage, snapshot restore, WAL replay, Raft initialization, listen. Each step depends on the previous one. If any step fails, the server prints an error and exits cleanly — no half-initialized state.

---

## Exercise 4: Integration Tests

**Goal:** Write end-to-end integration tests that execute SQL queries through the complete stack and verify the results.

### Step 1: A test harness

```rust,ignore
// tests/integration.rs

use toydb_server::Server;
use toydb_server::config::Config;
use std::path::PathBuf;

/// Create a server for testing with a temporary data directory.
fn test_server() -> Server {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        node_id: 1,
        listen_addr: "127.0.0.1:0".to_string(), // port 0 = OS picks a free port
        raft_addr: "127.0.0.1:0".to_string(),
        data_dir: dir.path().to_path_buf(),
        peers: vec![],  // single-node cluster for testing
        election_timeout_min: 150,
        election_timeout_max: 300,
        heartbeat_interval: 50,
        snapshot_threshold: 10_000,
    };

    Server::new_from_config(config).expect("server creation failed")
}

/// Execute SQL and assert success, returning the response.
fn exec(server: &mut Server, sql: &str) -> Response {
    match server.execute(sql) {
        Ok(result) => server.result_set_to_response(result),
        Err(e) => panic!("Query failed: {}\nSQL: {}", e, sql),
    }
}

/// Execute SQL and assert it returns rows.
fn query_rows(server: &mut Server, sql: &str) -> Vec<Vec<String>> {
    match exec(server, sql) {
        Response::Rows { rows, .. } => rows,
        other => panic!("Expected rows, got {:?}\nSQL: {}", other, sql),
    }
}

/// Execute SQL and assert it returns Ok.
fn exec_ok(server: &mut Server, sql: &str) -> String {
    match exec(server, sql) {
        Response::Ok { message } => message,
        other => panic!("Expected Ok, got {:?}\nSQL: {}", other, sql),
    }
}
```

### Step 2: Basic SQL tests

```rust,ignore
#[test]
fn test_create_table_and_insert() {
    let mut server = test_server();

    exec_ok(&mut server, "CREATE TABLE users (id INTEGER, name TEXT)");
    exec_ok(&mut server, "INSERT INTO users VALUES (1, 'Alice')");
    exec_ok(&mut server, "INSERT INTO users VALUES (2, 'Bob')");

    let rows = query_rows(&mut server, "SELECT * FROM users ORDER BY id");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["1", "Alice"]);
    assert_eq!(rows[1], vec!["2", "Bob"]);
}

#[test]
fn test_update_and_delete() {
    let mut server = test_server();

    exec_ok(&mut server, "CREATE TABLE items (id INTEGER, name TEXT, price FLOAT)");
    exec_ok(&mut server, "INSERT INTO items VALUES (1, 'Widget', 9.99)");
    exec_ok(&mut server, "INSERT INTO items VALUES (2, 'Gadget', 19.99)");

    exec_ok(&mut server, "UPDATE items SET price = 14.99 WHERE id = 1");

    let rows = query_rows(&mut server, "SELECT name, price FROM items WHERE id = 1");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], vec!["Widget", "14.99"]);

    exec_ok(&mut server, "DELETE FROM items WHERE id = 2");

    let rows = query_rows(&mut server, "SELECT * FROM items");
    assert_eq!(rows.len(), 1);
}

#[test]
fn test_aggregations() {
    let mut server = test_server();

    exec_ok(&mut server, "CREATE TABLE scores (player TEXT, score INTEGER)");
    exec_ok(&mut server, "INSERT INTO scores VALUES ('Alice', 100)");
    exec_ok(&mut server, "INSERT INTO scores VALUES ('Bob', 200)");
    exec_ok(&mut server, "INSERT INTO scores VALUES ('Alice', 150)");

    let rows = query_rows(
        &mut server,
        "SELECT player, SUM(score) FROM scores GROUP BY player ORDER BY player",
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["Alice", "250"]);
    assert_eq!(rows[1], vec!["Bob", "200"]);
}

#[test]
fn test_joins() {
    let mut server = test_server();

    exec_ok(&mut server, "CREATE TABLE users (id INTEGER, name TEXT)");
    exec_ok(&mut server, "CREATE TABLE orders (user_id INTEGER, item TEXT)");

    exec_ok(&mut server, "INSERT INTO users VALUES (1, 'Alice')");
    exec_ok(&mut server, "INSERT INTO users VALUES (2, 'Bob')");
    exec_ok(&mut server, "INSERT INTO orders VALUES (1, 'Widget')");
    exec_ok(&mut server, "INSERT INTO orders VALUES (1, 'Gadget')");
    exec_ok(&mut server, "INSERT INTO orders VALUES (2, 'Gizmo')");

    let rows = query_rows(
        &mut server,
        "SELECT u.name, o.item FROM users u \
         JOIN orders o ON u.id = o.user_id \
         ORDER BY u.name, o.item",
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], vec!["Alice", "Gadget"]);
    assert_eq!(rows[1], vec!["Alice", "Widget"]);
    assert_eq!(rows[2], vec!["Bob", "Gizmo"]);
}
```

### Step 3: Error handling tests

```rust,ignore
#[test]
fn test_syntax_error() {
    let mut server = test_server();

    let response = exec(&mut server, "SELECTT * FROM users");
    match response {
        Response::Error { message } => {
            assert!(
                message.contains("error") || message.contains("Error"),
                "Expected error message, got: {}",
                message,
            );
        }
        _ => panic!("Expected error response for invalid SQL"),
    }
}

#[test]
fn test_table_not_found() {
    let mut server = test_server();

    let response = exec(&mut server, "SELECT * FROM nonexistent");
    match response {
        Response::Error { message } => {
            assert!(
                message.contains("nonexistent"),
                "Error should mention the missing table, got: {}",
                message,
            );
        }
        _ => panic!("Expected error for missing table"),
    }
}

#[test]
fn test_type_mismatch() {
    let mut server = test_server();

    exec_ok(&mut server, "CREATE TABLE typed (id INTEGER, name TEXT)");

    // Inserting wrong number of columns should fail
    let response = exec(&mut server, "INSERT INTO typed VALUES (1)");
    match response {
        Response::Error { message } => {
            assert!(
                message.to_lowercase().contains("column")
                    || message.to_lowercase().contains("value"),
                "Error should mention column/value mismatch, got: {}",
                message,
            );
        }
        _ => panic!("Expected error for column count mismatch"),
    }
}
```

### Step 4: Recovery test

```rust,ignore
#[test]
fn test_recovery_preserves_data() {
    let dir = tempfile::tempdir().unwrap();

    let config = || Config {
        node_id: 1,
        listen_addr: "127.0.0.1:0".to_string(),
        raft_addr: "127.0.0.1:0".to_string(),
        data_dir: dir.path().to_path_buf(),
        peers: vec![],
        election_timeout_min: 150,
        election_timeout_max: 300,
        heartbeat_interval: 50,
        snapshot_threshold: 10_000,
    };

    // First run: create table and insert data
    {
        let mut server = Server::new_from_config(config()).unwrap();
        exec_ok(&mut server, "CREATE TABLE persist_test (id INTEGER, val TEXT)");
        exec_ok(&mut server, "INSERT INTO persist_test VALUES (1, 'hello')");
        exec_ok(&mut server, "INSERT INTO persist_test VALUES (2, 'world')");
        // server is dropped here — simulates shutdown
    }

    // Second run: data should still be there
    {
        let mut server = Server::new_from_config(config()).unwrap();
        let rows = query_rows(
            &mut server,
            "SELECT * FROM persist_test ORDER BY id",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["1", "hello"]);
        assert_eq!(rows[1], vec!["2", "world"]);
    }
}
```

This is the ultimate test: data survives a complete server shutdown and restart. The second `Server::new_from_config` call triggers recovery — it reads the WAL, replays committed entries, and reconstructs the state machine. If the SELECT returns the correct data, every layer is working correctly: persistence, recovery, parsing, planning, and execution.

---

## The Layer Boundary Map

Here is the complete map of types that flow between layers:

```
┌─────────────┐    &str         ┌─────────────┐   Vec<Token>    ┌─────────────┐
│   Client     │───────────────>│   Lexer      │───────────────>│   Parser    │
└─────────────┘                 └─────────────┘                 └──────┬──────┘
                                                                       │
                                                                  Statement
                                                                       │
┌─────────────┐   ResultSet     ┌─────────────┐      Plan       ┌──────▼──────┐
│   Client     │<───────────────│  Executor    │<───────────────│  Planner    │
└─────────────┘                 └──────┬──────┘                 └─────────────┘
                                       │                              │
                                   Row/Value                     Schema info
                                       │                              │
                                ┌──────▼──────┐                 ┌─────▼───────┐
                                │    MVCC      │<───────────────│  Optimizer   │
                                │   Storage    │     (Plan)     └─────────────┘
                                └──────┬──────┘
                                       │
                                   Key/Value bytes
                                       │
                                ┌──────▼──────┐
                                │   BitCask /  │
                                │   Memory KV  │
                                └─────────────┘

Write path adds:
                                ┌─────────────┐
                         ──────>│    Raft      │──────> replicate to followers
                   Plan bytes   │   Node       │
                                └──────┬──────┘
                                       │
                                  WAL + state file
```

Each arrow is a function call with a typed input and output. Each box is a module with a public API. The types at the boundaries (`Token`, `Statement`, `Plan`, `ResultSet`, `Row`, `Value`) are the contracts between layers. As long as these types do not change, each layer can evolve independently.

---

## Rust Gym

Time for reps. These drills focus on modules, workspaces, and error propagation — the spotlight concepts for this chapter.

### Drill 1: EXPLAIN ANALYZE (Medium)

Add timing information to query execution. Wrap the executor to measure time spent in each phase, and return it as part of the response when the query starts with `EXPLAIN ANALYZE`.

```rust
use std::time::{Duration, Instant};

/// Execution timing for each phase.
struct QueryTiming {
    lex_time: Duration,
    parse_time: Duration,
    plan_time: Duration,
    optimize_time: Duration,
    execute_time: Duration,
}

impl QueryTiming {
    fn total(&self) -> Duration {
        self.lex_time + self.parse_time + self.plan_time
            + self.optimize_time + self.execute_time
    }

    fn display(&self) -> String {
        format!(
            "Lex: {:?}, Parse: {:?}, Plan: {:?}, Optimize: {:?}, Execute: {:?}, Total: {:?}",
            self.lex_time, self.parse_time, self.plan_time,
            self.optimize_time, self.execute_time, self.total()
        )
    }
}

/// Simulated query phases (replace with real implementations).
fn lex(sql: &str) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(100));
    sql.split_whitespace().map(|s| s.to_string()).collect()
}

fn parse(tokens: Vec<String>) -> String {
    std::thread::sleep(Duration::from_micros(200));
    tokens.join(" ")
}

fn plan(ast: String) -> String {
    std::thread::sleep(Duration::from_micros(50));
    format!("Plan({})", ast)
}

fn optimize(plan: String) -> String {
    std::thread::sleep(Duration::from_micros(30));
    plan // no-op optimization
}

fn execute(plan: String) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(500));
    vec![format!("Result of {}", plan)]
}

fn execute_with_timing(sql: &str) -> (Vec<String>, QueryTiming) {
    // TODO: Execute each phase and measure its duration
    todo!()
}

fn main() {
    let sql = "SELECT * FROM users WHERE id = 1";
    let (results, timing) = execute_with_timing(sql);
    println!("Results: {:?}", results);
    println!("Timing: {}", timing.display());
    assert!(timing.total() > Duration::from_micros(500));
    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::time::{Duration, Instant};

struct QueryTiming {
    lex_time: Duration,
    parse_time: Duration,
    plan_time: Duration,
    optimize_time: Duration,
    execute_time: Duration,
}

impl QueryTiming {
    fn total(&self) -> Duration {
        self.lex_time + self.parse_time + self.plan_time
            + self.optimize_time + self.execute_time
    }

    fn display(&self) -> String {
        format!(
            "Lex: {:?}, Parse: {:?}, Plan: {:?}, Optimize: {:?}, Execute: {:?}, Total: {:?}",
            self.lex_time, self.parse_time, self.plan_time,
            self.optimize_time, self.execute_time, self.total()
        )
    }
}

fn lex(sql: &str) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(100));
    sql.split_whitespace().map(|s| s.to_string()).collect()
}

fn parse(tokens: Vec<String>) -> String {
    std::thread::sleep(Duration::from_micros(200));
    tokens.join(" ")
}

fn plan(ast: String) -> String {
    std::thread::sleep(Duration::from_micros(50));
    format!("Plan({})", ast)
}

fn optimize(p: String) -> String {
    std::thread::sleep(Duration::from_micros(30));
    p
}

fn execute(p: String) -> Vec<String> {
    std::thread::sleep(Duration::from_micros(500));
    vec![format!("Result of {}", p)]
}

fn time<F, T>(f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    (result, elapsed)
}

fn execute_with_timing(sql: &str) -> (Vec<String>, QueryTiming) {
    let (tokens, lex_time) = time(|| lex(sql));
    let (ast, parse_time) = time(|| parse(tokens));
    let (planned, plan_time) = time(|| plan(ast));
    let (optimized, optimize_time) = time(|| optimize(planned));
    let (results, execute_time) = time(|| execute(optimized));

    let timing = QueryTiming {
        lex_time,
        parse_time,
        plan_time,
        optimize_time,
        execute_time,
    };

    (results, timing)
}

fn main() {
    let sql = "SELECT * FROM users WHERE id = 1";
    let (results, timing) = execute_with_timing(sql);
    println!("Results: {:?}", results);
    println!("Timing: {}", timing.display());
    assert!(timing.total() > Duration::from_micros(500));
    println!("All checks passed!");
}
```

The `time` helper is a generic function that takes any closure, runs it, and returns both the result and the elapsed duration. This is a common pattern for instrumentation — wrap each phase in `time(|| phase())` without changing the phase's implementation. The closure `FnOnce() -> T` captures variables by move, which is why `tokens`, `ast`, etc., are consumed when passed to the next phase.

</details>

### Drill 2: Linearizable Reads (Hard)

Implement a read lease mechanism. The leader tracks the last time a majority of followers confirmed it is still the leader. Reads are only served if the lease has not expired.

```rust
use std::time::{Duration, Instant};

struct ReadLease {
    /// When the lease was last confirmed.
    last_confirmed: Instant,
    /// How long the lease is valid.
    lease_duration: Duration,
    /// Whether this node believes it is the leader.
    is_leader: bool,
}

impl ReadLease {
    fn new(lease_duration: Duration) -> Self {
        // TODO
        todo!()
    }

    /// Called when a majority of followers acknowledge a heartbeat.
    fn confirm(&mut self) {
        // TODO
        todo!()
    }

    /// Check if we can serve reads.
    fn can_serve_read(&self) -> bool {
        // TODO
        todo!()
    }

    /// Called when we lose leadership.
    fn revoke(&mut self) {
        // TODO
        todo!()
    }
}

fn main() {
    let mut lease = ReadLease::new(Duration::from_millis(500));

    // Initially cannot serve reads (no confirmation yet)
    assert!(!lease.can_serve_read());

    // Become leader and confirm
    lease.is_leader = true;
    lease.confirm();
    assert!(lease.can_serve_read());

    // Wait for lease to expire
    std::thread::sleep(Duration::from_millis(600));
    assert!(!lease.can_serve_read());

    // Re-confirm
    lease.confirm();
    assert!(lease.can_serve_read());

    // Lose leadership
    lease.revoke();
    assert!(!lease.can_serve_read());

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::time::{Duration, Instant};

struct ReadLease {
    last_confirmed: Instant,
    lease_duration: Duration,
    is_leader: bool,
}

impl ReadLease {
    fn new(lease_duration: Duration) -> Self {
        ReadLease {
            // Set to a time far in the past so the initial lease is expired
            last_confirmed: Instant::now() - lease_duration - Duration::from_secs(1),
            lease_duration,
            is_leader: false,
        }
    }

    fn confirm(&mut self) {
        self.last_confirmed = Instant::now();
    }

    fn can_serve_read(&self) -> bool {
        self.is_leader && self.last_confirmed.elapsed() < self.lease_duration
    }

    fn revoke(&mut self) {
        self.is_leader = false;
    }
}

fn main() {
    let mut lease = ReadLease::new(Duration::from_millis(500));

    assert!(!lease.can_serve_read());

    lease.is_leader = true;
    lease.confirm();
    assert!(lease.can_serve_read());

    std::thread::sleep(Duration::from_millis(600));
    assert!(!lease.can_serve_read());

    lease.confirm();
    assert!(lease.can_serve_read());

    lease.revoke();
    assert!(!lease.can_serve_read());

    println!("All checks passed!");
}
```

The read lease is a time-bounded assertion: "I was the leader as of time T, and my lease lasts D milliseconds, so I am still the leader until T+D." This is safe because Raft's election timeout is longer than the lease duration — a new leader cannot be elected until the current leader's heartbeats stop, which takes at least one election timeout. By setting the lease duration shorter than the election timeout, we guarantee that the lease expires before a new leader could be elected.

Real systems (etcd, CockroachDB) use this exact mechanism. The tradeoff: clock skew. If the leader's clock runs fast, its lease might expire too early (safe but reduces availability). If a follower's clock runs fast, it might start an election before the leader's lease expires (unsafe if the leader is still serving reads). This is why distributed systems care deeply about clock synchronization (NTP, PTP, or Google's TrueTime).

</details>

### Drill 3: Multi-Statement Transaction (Hard)

Implement a simple transaction that groups multiple SQL statements. All statements succeed or all are rolled back.

```rust
use std::collections::HashMap;

struct SimpleDb {
    data: HashMap<String, String>,
    // Transaction state
    pending: Option<HashMap<String, Option<String>>>, // key -> Some(new_value) or None (delete)
}

impl SimpleDb {
    fn new() -> Self {
        // TODO
        todo!()
    }

    fn begin(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn delete(&mut self, key: &str) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn get(&self, key: &str) -> Option<String> {
        // TODO: should see uncommitted changes within the transaction
        todo!()
    }

    fn commit(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }

    fn rollback(&mut self) -> Result<(), String> {
        // TODO
        todo!()
    }
}

fn main() {
    let mut db = SimpleDb::new();
    db.data.insert("a".into(), "1".into());
    db.data.insert("b".into(), "2".into());

    // Transaction that commits
    db.begin().unwrap();
    db.set("a", "10").unwrap();
    db.set("c", "3").unwrap();
    assert_eq!(db.get("a"), Some("10".to_string())); // sees uncommitted
    db.commit().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("c"), Some("3".to_string()));

    // Transaction that rolls back
    db.begin().unwrap();
    db.set("a", "999").unwrap();
    db.delete("b").unwrap();
    assert_eq!(db.get("a"), Some("999".to_string())); // sees uncommitted
    assert_eq!(db.get("b"), None); // sees the delete
    db.rollback().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string())); // rolled back
    assert_eq!(db.get("b"), Some("2".to_string()));  // rolled back

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct SimpleDb {
    data: HashMap<String, String>,
    pending: Option<HashMap<String, Option<String>>>,
}

impl SimpleDb {
    fn new() -> Self {
        SimpleDb {
            data: HashMap::new(),
            pending: None,
        }
    }

    fn begin(&mut self) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("transaction already in progress".to_string());
        }
        self.pending = Some(HashMap::new());
        Ok(())
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let pending = self.pending.as_mut()
            .ok_or("no active transaction")?;
        pending.insert(key.to_string(), Some(value.to_string()));
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), String> {
        let pending = self.pending.as_mut()
            .ok_or("no active transaction")?;
        pending.insert(key.to_string(), None);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<String> {
        // Check pending changes first (read-your-writes)
        if let Some(pending) = &self.pending {
            if let Some(change) = pending.get(key) {
                return change.clone(); // Some(value) or None (deleted)
            }
        }
        // Fall through to committed data
        self.data.get(key).cloned()
    }

    fn commit(&mut self) -> Result<(), String> {
        let pending = self.pending.take()
            .ok_or("no active transaction")?;

        for (key, value) in pending {
            match value {
                Some(v) => { self.data.insert(key, v); }
                None => { self.data.remove(&key); }
            }
        }

        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        self.pending.take()
            .ok_or("no active transaction")?;
        Ok(()) // just discard the pending changes
    }
}

fn main() {
    let mut db = SimpleDb::new();
    db.data.insert("a".into(), "1".into());
    db.data.insert("b".into(), "2".into());

    db.begin().unwrap();
    db.set("a", "10").unwrap();
    db.set("c", "3").unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    db.commit().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("c"), Some("3".to_string()));

    db.begin().unwrap();
    db.set("a", "999").unwrap();
    db.delete("b").unwrap();
    assert_eq!(db.get("a"), Some("999".to_string()));
    assert_eq!(db.get("b"), None);
    db.rollback().unwrap();
    assert_eq!(db.get("a"), Some("10".to_string()));
    assert_eq!(db.get("b"), Some("2".to_string()));

    println!("All checks passed!");
}
```

The `pending` field is `Option<HashMap<...>>` — `None` means no active transaction, `Some(...)` means a transaction is in progress. The `Option` replaces a boolean flag + separate buffer, and Rust's pattern matching makes the "no active transaction" error check natural. Commit applies all pending changes to the main data; rollback discards them with `.take()`. The `.take()` method moves the value out of the `Option`, replacing it with `None` — a clean ownership transfer that also resets the transaction state.

</details>

### Drill 4: Graceful Cluster Shutdown (Medium)

Implement a shutdown coordinator that waits for in-flight requests to complete before stopping the server.

```rust
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::Duration;

struct ShutdownCoordinator {
    /// Set to true when shutdown is requested.
    shutting_down: Arc<AtomicBool>,
    /// Number of requests currently being processed.
    in_flight: Arc<AtomicUsize>,
}

impl ShutdownCoordinator {
    fn new() -> Self {
        // TODO
        todo!()
    }

    /// Called before processing a request. Returns false if
    /// the server is shutting down (reject the request).
    fn begin_request(&self) -> bool {
        // TODO
        todo!()
    }

    /// Called after a request is complete.
    fn end_request(&self) {
        // TODO
        todo!()
    }

    /// Initiate shutdown. Blocks until all in-flight requests complete
    /// or the timeout expires.
    fn shutdown(&self, timeout: Duration) -> bool {
        // TODO: returns true if clean shutdown, false if timed out
        todo!()
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }
}

fn main() {
    let coord = ShutdownCoordinator::new();

    // Simulate some in-flight requests
    assert!(coord.begin_request());
    assert!(coord.begin_request());
    assert_eq!(coord.in_flight_count(), 2);

    // Start shutdown — new requests should be rejected
    let coord_clone = ShutdownCoordinator {
        shutting_down: coord.shutting_down.clone(),
        in_flight: coord.in_flight.clone(),
    };

    let handle = std::thread::spawn(move || {
        coord_clone.shutdown(Duration::from_secs(5))
    });

    // Give the shutdown thread time to set the flag
    std::thread::sleep(Duration::from_millis(50));

    // New requests should be rejected
    assert!(!coord.begin_request());
    assert!(coord.is_shutting_down());

    // Complete the in-flight requests
    coord.end_request();
    coord.end_request();

    // Shutdown should complete cleanly
    let clean = handle.join().unwrap();
    assert!(clean);

    println!("All checks passed!");
}
```

<details>
<summary>Solution</summary>

```rust
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::{Duration, Instant};

struct ShutdownCoordinator {
    shutting_down: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
}

impl ShutdownCoordinator {
    fn new() -> Self {
        ShutdownCoordinator {
            shutting_down: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn begin_request(&self) -> bool {
        // Check if shutting down BEFORE incrementing
        if self.shutting_down.load(Ordering::SeqCst) {
            return false;
        }
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        // Double-check after incrementing (avoid race with shutdown)
        if self.shutting_down.load(Ordering::SeqCst) {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        true
    }

    fn end_request(&self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.shutting_down.store(true, Ordering::SeqCst);

        let start = Instant::now();
        while self.in_flight.load(Ordering::SeqCst) > 0 {
            if start.elapsed() > timeout {
                return false; // timed out
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }
}

fn main() {
    let coord = ShutdownCoordinator::new();

    assert!(coord.begin_request());
    assert!(coord.begin_request());
    assert_eq!(coord.in_flight_count(), 2);

    let coord_clone = ShutdownCoordinator {
        shutting_down: coord.shutting_down.clone(),
        in_flight: coord.in_flight.clone(),
    };

    let handle = std::thread::spawn(move || {
        coord_clone.shutdown(Duration::from_secs(5))
    });

    std::thread::sleep(Duration::from_millis(50));

    assert!(!coord.begin_request());
    assert!(coord.is_shutting_down());

    coord.end_request();
    coord.end_request();

    let clean = handle.join().unwrap();
    assert!(clean);

    println!("All checks passed!");
}
```

The double-check in `begin_request` is important. Without it, there is a race condition: a request could check `shutting_down` (sees false), then the shutdown thread sets the flag, then the request increments `in_flight`. The shutdown thread would see `in_flight > 0` and wait, but the request was accepted after shutdown started. The double-check closes this race: if shutdown happened between the first check and the increment, the second check catches it and decrements back.

This is a simplified version of the "graceful shutdown" pattern used in production HTTP servers (Hyper, Actix, Axum). The real implementations use `tokio::sync::Notify` or channels instead of polling, but the principle is the same: stop accepting new work, finish existing work, then exit.

</details>

---

## DSA in Context: Module Dependency Graphs

The integration of multiple layers creates a **dependency graph** — a directed graph where nodes are modules and edges are "depends on" relationships.

### Our dependency graph

```
toydb-server
├── toydb-sql
│   └── (no external deps)
├── toydb-storage
│   └── (no external deps)
└── toydb-raft
    └── (no external deps)
```

This is a **tree** — no cycles. `toydb-sql` does not depend on `toydb-raft`. `toydb-storage` does not depend on `toydb-sql`. Only `toydb-server` depends on all three.

### Why cycles are dangerous

If `toydb-sql` depended on `toydb-storage` AND `toydb-storage` depended on `toydb-sql`, you would have a **cycle**:

```
toydb-sql ←→ toydb-storage  (CYCLE — does not compile!)
```

Rust's crate system forbids cyclic dependencies — the compiler rejects them. This is a feature, not a limitation. Cyclic dependencies mean the two crates cannot be understood independently. A change in either one might break the other. Testing requires both to be present. The cycle binds them into a single conceptual unit that should probably be a single crate.

### Topological sort and build order

Cargo builds crates in **topological order** — a crate is compiled only after all its dependencies are compiled. For our workspace:

```
Build order:
1. toydb-sql      (no deps — can build immediately)
2. toydb-storage   (no deps — can build in parallel with toydb-sql)
3. toydb-raft      (no deps — can build in parallel with both above)
4. toydb-server    (depends on all three — must wait for them)
```

Steps 1-3 can run in parallel because they have no dependencies on each other. This is why workspaces with many leaf crates build faster than monolithic crates — the compiler can parallelize.

### Dependency inversion

What if the executor needs to know about storage, but storage should not know about the executor? Use a **trait** (interface) to invert the dependency:

```rust,ignore
// In toydb-sql (no dependency on toydb-storage)
pub trait Storage {
    fn scan_table(&self, table: &str) -> Box<dyn Iterator<Item = Row>>;
    fn insert_row(&mut self, table: &str, row: Row) -> Result<(), String>;
}

// In toydb-storage (no dependency on toydb-sql)
impl Storage for MvccStorage {
    fn scan_table(&self, table: &str) -> Box<dyn Iterator<Item = Row>> {
        // ...
    }
    fn insert_row(&mut self, table: &str, row: Row) -> Result<(), String> {
        // ...
    }
}
```

Wait — this does not work as written. `toydb-storage` would need to depend on `toydb-sql` to implement the `Storage` trait. The solution is to put the trait in a separate crate (`toydb-traits`) or use the `Storage` trait in the server crate where both are available. This is the **dependency inversion principle**: high-level modules define interfaces, low-level modules implement them, and both depend on the abstraction (the trait) rather than on each other.

---

## System Design Corner: Layered Architecture

Our database has a classic **layered architecture** — each layer provides services to the layer above and consumes services from the layer below. This is the dominant pattern in systems software.

### The layers

```
┌─────────────────────────────────────┐
│ Layer 7: Client Interface           │  (REPL, wire protocol)
├─────────────────────────────────────┤
│ Layer 6: Query Processing           │  (lexer, parser, planner, optimizer)
├─────────────────────────────────────┤
│ Layer 5: Execution Engine           │  (Volcano-model executor)
├─────────────────────────────────────┤
│ Layer 4: Transaction Management     │  (MVCC, snapshot isolation)
├─────────────────────────────────────┤
│ Layer 3: Consensus                  │  (Raft leader election + log replication)
├─────────────────────────────────────┤
│ Layer 2: Storage Engine             │  (BitCask, in-memory KV)
├─────────────────────────────────────┤
│ Layer 1: Operating System           │  (files, network, memory)
└─────────────────────────────────────┘
```

### Rules of layered architecture

1. **Each layer depends only on the layer directly below it.** The parser does not know about the storage engine. The executor does not know about Raft. This limits the blast radius of changes.

2. **Each layer has a well-defined interface.** The lexer's interface is `&str → Vec<Token>`. The parser's interface is `Vec<Token> → Statement`. These types are the contracts.

3. **Layers can be replaced independently.** Swap the storage engine from in-memory to disk-based? Only the storage layer changes. Replace the optimizer? Only the optimizer changes. Add a new wire protocol? Only the client interface layer changes.

4. **Skip layers carefully.** Sometimes a higher layer needs to bypass an intermediate layer for performance. For example, read queries skip the Raft layer (Layer 3) and go directly from execution (Layer 5) to storage (Layer 2). This is a deliberate design choice — it violates strict layering but is justified by the performance benefit and the fact that reads do not need consensus.

### Real-world examples

| Database | Layers |
|----------|--------|
| PostgreSQL | Client → Parser → Rewriter → Planner → Executor → Access Methods → Buffer Manager → Storage |
| MySQL | Client → Parser → Optimizer → Handler → Storage Engine (InnoDB/MyISAM) |
| CockroachDB | Client → SQL → Distributed SQL → Transaction → Raft → Pebble (storage) |
| SQLite | Client → Parser → Code Generator → Virtual Machine → B-tree → Pager → OS Interface |

Our toydb has the same shape as CockroachDB — SQL over Raft over a storage engine. The difference is scale: CockroachDB adds distributed SQL execution, range-based sharding, and a production-grade storage engine (Pebble, based on LevelDB). But the architectural pattern is identical.

> **Interview talking point:** *"Our database uses a layered architecture with clear boundaries between the SQL frontend (parser, planner, optimizer), the execution engine, the transaction manager (MVCC), the consensus layer (Raft), and the storage engine. Write queries flow through all layers — the SQL is parsed, planned, proposed to Raft, replicated to followers, committed, and then executed against MVCC storage. Read queries skip the consensus layer entirely, going directly from the executor to MVCC storage. This is safe because we only serve reads on the leader, and we use a read lease mechanism to ensure the leader is still authoritative. The error type hierarchy uses Rust's From trait to propagate errors cleanly across layer boundaries, with the server converting all errors into client-facing error messages."*

---

## Design Insight: Information Hiding

> *"The most important technique for achieving simplicity is to design systems so that developers only need to face a small fraction of the overall complexity at any time."*
> — John Ousterhout, *A Philosophy of Software Design*

Consider what each layer hides from the one above it:

| Layer | What it hides |
|-------|---------------|
| **Lexer** | Character-by-character scanning, whitespace handling, string escape sequences, keyword recognition |
| **Parser** | Token lookahead, operator precedence, recursive descent, error recovery |
| **Planner** | Schema lookup, type validation, plan node construction, table existence checks |
| **Optimizer** | Constant folding rules, filter pushdown logic, cost estimation, plan tree transformations |
| **Executor** | Volcano model iteration, hash table construction for joins, accumulator management for aggregations |
| **MVCC** | Version chains, snapshot timestamps, write conflict detection, garbage collection of old versions |
| **Raft** | Leader election timeouts, heartbeat scheduling, log matching, vote counting, term management |
| **Storage** | File format, fsync timing, compaction, block caching, disk I/O scheduling |

Each layer's hidden complexity is substantial — hundreds or thousands of lines of code. But the interface between layers is tiny: a few types and a few functions. This ratio of hidden complexity to interface surface area is what Ousterhout calls the "deep module" principle. The deepest modules provide the most value because they hide the most complexity behind the simplest interface.

The integration layer (this chapter's `Server` struct) is the opposite — a **shallow module**. It has many dependencies but does little work of its own. Its job is wiring, not computation. Shallow modules are fine at the top of the architecture — someone has to connect the pieces — but the deep modules below are where the real value lives.

---

## What You Built

In this chapter, you:

1. **Connected all layers** — SQL string to tokens to AST to plan to optimized plan to replicated log entry to executed result to client response, through 9 distinct processing stages
2. **Built the Server struct** — a single owner for all layers, routing reads directly to storage and writes through Raft consensus
3. **Designed error propagation** — a unified `DbError` type with `From` implementations for all layer errors, enabling clean `?`-based propagation
4. **Implemented configuration and startup** — recovery from disk, snapshot restoration, WAL replay, Raft initialization, connection acceptance
5. **Wrote integration tests** — end-to-end SQL queries through the complete stack, including recovery tests that verify data survives server restarts
6. **Practiced Rust's module system** — `pub`/`pub(crate)`/private visibility, `use` imports, `mod` declarations, workspace organization, re-exports

Your database is complete. It accepts SQL over TCP, parses it, plans it, replicates writes through Raft, executes queries against MVCC storage, and returns results to the client. Data survives crashes. Writes are replicated to multiple nodes. Reads are served from the leader's local storage.

Chapter 18 adds rigor: testing strategies for each layer, benchmarking to measure performance, and ideas for extending your database with new features.

---

### DS Deep Dive

Our integration uses a synchronous request-response model — the client sends a query and blocks until the response arrives. Production databases support pipelining (send multiple queries before reading any responses), streaming (send results row by row as they are produced), and multiplexing (interleave multiple queries on the same connection). This deep dive explores these advanced protocol patterns, their impact on throughput and latency, and how they interact with connection pooling.
