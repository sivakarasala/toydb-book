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
