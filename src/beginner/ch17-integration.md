# Chapter 17: Integration -- SQL over Raft

You have built a lot of pieces. A SQL lexer that turns text into tokens. A parser that turns tokens into a syntax tree. A query planner and optimizer. An executor that runs queries against storage. An MVCC storage engine with transactions. A client-server protocol. An async networking layer. A Raft consensus system with leader election, log replication, and durability.

Each piece works in isolation. But a database is not a collection of parts -- it is a system. A user types `INSERT INTO users VALUES (1, 'Alice')` and expects the data to appear on three machines, survive crashes, and be immediately visible to the next `SELECT`. This chapter connects every layer into a single executable that does exactly that.

By the end of this chapter, you will have:

- A clear understanding of how a SQL query flows through every layer
- Rust modules (`mod`, `use`, `pub`) explained from scratch
- A `Server` struct that owns and coordinates all layers
- Separate read and write paths (writes go through Raft, reads use local storage)
- An end-to-end test: INSERT then SELECT through the complete stack

---

## Spotlight: Module System & Workspace

Every chapter has one **spotlight concept**. This chapter's spotlight is **module system and workspace** -- how Rust organizes code into pieces and controls what each piece can see.

### The room analogy

Think of your project as a house. Each **module** is a room:

```
Your Database House:
+------------------+------------------+
|     Kitchen      |   Living Room    |
|   (storage)      |    (server)      |
|   - stove (kv)   |   - couch (tcp)  |
|   - fridge(mvcc) |   - tv (handler) |
+------------------+------------------+
|     Bedroom      |    Study         |
|     (sql)        |    (raft)        |
|   - desk(parser) |   - books(wal)   |
|   - lamp(lexer)  |   - chair(node)  |
+------------------+------------------+
```

Each room has its own furniture (functions and types). Doors between rooms (`pub`) control what you can access from where. The kitchen's stove is visible from the living room (because we made it `pub`), but the bedroom's private diary is hidden.

### Creating modules with `mod`

In Rust, you declare modules in your `lib.rs` or `main.rs`:

```rust,ignore
// src/lib.rs

mod storage;   // tells Rust: "load src/storage.rs"
mod sql;       // tells Rust: "load src/sql.rs"
mod raft;      // tells Rust: "load src/raft.rs"
mod server;    // tells Rust: "load src/server.rs"
```

Each `mod` declaration does two things:
1. Creates a namespace (like `storage::MvccStorage`)
2. Tells the compiler to include that file

Modules can be nested. If `sql` is a directory:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;      // loads src/sql/lexer.rs
pub mod parser;     // loads src/sql/parser.rs
pub mod planner;    // loads src/sql/planner.rs
pub mod executor;   // loads src/sql/executor.rs
```

> **Programming Concept: File = Module**
>
> In Rust, the file system structure mirrors the module hierarchy. This is not a convention -- it is a rule. If you write `mod sql;` in `lib.rs`, the compiler looks for `src/sql.rs` or `src/sql/mod.rs`. If neither exists, you get a compile error. This makes it easy to find code -- the module path tells you exactly which file to open.

### Visibility: `pub`, `pub(crate)`, and private

By default, everything in Rust is **private**. You cannot access a function, struct, or field from outside its module unless you explicitly make it visible:

```rust,ignore
// src/sql/parser.rs

// Anyone can see and use this struct
pub struct Parser {
    tokens: Vec<Token>,   // PRIVATE -- only Parser's own code can touch this
    position: usize,      // PRIVATE
}

// Only code in this crate can call this function
pub(crate) fn validate_ast(ast: &Statement) -> Result<(), String> {
    // ...
}

// Only code in this module can call this function
fn consume_token(tokens: &[Token], pos: &mut usize) -> Option<&Token> {
    // ...
}
```

Three levels of visibility:

| Keyword | Who can see it? | Analogy |
|---------|----------------|---------|
| (nothing) | Same module only | Your bedroom -- only you go in |
| `pub(crate)` | Anywhere in this crate | The kitchen -- family only |
| `pub` | Anyone, including external code | The front porch -- the whole neighborhood |

This maps directly to our database layers. The parser's internal state (`tokens`, `position`) is private -- no one outside the parser needs to know about token positions. The `Parser` struct itself is `pub` -- it is the public API that the server uses.

### The `use` keyword: importing names

Without `use`, you would write full paths everywhere:

```rust,ignore
// Without use -- verbose
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

// Much cleaner
fn execute(plan: Plan, storage: &mut MvccStorage) -> ResultSet {
    // ...
}
```

The convention: import types (structs, enums) directly. Import functions through their parent module to avoid confusion about where they come from:

```rust,ignore
use std::io;             // then call: io::Read, io::Write
use std::io::BufReader;  // type imported directly
use std::collections::HashMap;  // type imported directly
```

> **What Just Happened?**
>
> We covered Rust's module system:
> - **`mod`** declares a module and tells the compiler which file to load
> - **`pub`** makes things visible outside their module
> - **`use`** imports names so you do not write full paths
> - The file system structure mirrors the module hierarchy
>
> This is how we organize thousands of lines of database code into manageable pieces, with clear boundaries between layers.

### Re-exports: simplifying the public API

Sometimes your module structure has deep paths. Re-exports flatten them:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;
pub mod parser;
pub mod planner;
pub mod executor;

// Re-export the most commonly used types at the module root
pub use parser::Parser;
pub use planner::{Plan, Planner};
pub use executor::{Executor, ResultSet};
```

Now other code can write `use crate::sql::Parser` instead of `use crate::sql::parser::Parser`. The internal module structure is hidden behind convenient re-exports.

---

## The Big Picture: Request Flow

Before writing code, let's trace a SQL query through every layer. This is the map that connects everything we have built.

A client sends: `INSERT INTO users (id, name) VALUES (1, 'Alice')`

```
Client                          Server
  |                               |
  |  TCP: send SQL string         |
  |------------------------------>|
  |                               |
  |                    +----------v----------+
  |                    | 1. Protocol Layer   |
  |                    |    Deserialize the  |
  |                    |    request          |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 2. Lexer            |
  |                    |    SQL string -->   |
  |                    |    tokens           |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 3. Parser           |
  |                    |    tokens -->       |
  |                    |    syntax tree      |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 4. Planner          |
  |                    |    syntax tree -->  |
  |                    |    execution plan   |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 5. Optimizer        |
  |                    |    plan --> better  |
  |                    |    plan             |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 6. Raft (replicate) |
  |                    |    serialize plan,  |
  |                    |    send to          |
  |                    |    followers, wait  |
  |                    |    for majority     |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 7. Executor         |
  |                    |    run the INSERT   |
  |                    |    against storage  |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 8. MVCC Storage     |
  |                    |    begin txn,       |
  |                    |    write row,       |
  |                    |    commit txn       |
  |                    +----------+----------+
  |                               |
  |                    +----------v----------+
  |                    | 9. Protocol Layer   |
  |                    |    serialize        |
  |                    |    response         |
  |                    +----------+----------+
  |                               |
  |  TCP: receive response        |
  |<------------------------------|
```

Nine layers. Each one takes input from above and produces output for below. The interfaces between layers are the types we defined in earlier chapters: `Token`, `Statement`, `Plan`, `LogEntry`, `Row`, `Response`.

### Write path vs read path

The flow above is the **write path** -- it goes through Raft for replication. The **read path** is shorter:

```
SELECT * FROM users WHERE id = 1

1. Protocol:  deserialize request
2. Lexer:     tokenize
3. Parser:    parse to syntax tree
4. Planner:   create Plan::Select
5. Optimizer: push down filters
6. Executor:  scan storage, apply filters
7. MVCC:      read rows at current snapshot
8. Protocol:  serialize response

Notice: NO RAFT. Reads go directly to local storage.
```

Reads skip Raft because the leader's local storage is always up-to-date. All committed writes have been applied. As long as we serve reads only from the leader, local reads are safe.

> **What Just Happened?**
>
> We traced the complete path of a SQL query through the system. Writes go through all nine layers including Raft replication. Reads skip Raft because the leader's local storage already has all committed data. This separation (write path vs read path) is a common pattern in distributed databases -- it reduces latency for reads.

---

## Exercise 1: The Server Struct

**Goal:** Build a `Server` struct that owns all the layers and coordinates query execution.

### Step 1: Define the ownership hierarchy

The server is the top-level owner. It owns every layer:

```rust,ignore
// src/server.rs

use crate::sql::{Lexer, Parser, Planner, Optimizer, Executor, ResultSet, Plan};
use crate::storage::MvccStorage;
use crate::raft::RaftNode;
use crate::protocol::{Request, Response};

/// The database server. Owns all layers and coordinates queries.
///
/// Ownership hierarchy:
///   Server
///   +-- RaftNode (consensus, owns WAL and state files)
///   +-- MvccStorage (storage engine, owns the KV store)
///   +-- ServerConfig (configuration)
pub struct Server {
    /// The Raft consensus node.
    raft: RaftNode,
    /// The MVCC storage engine.
    storage: MvccStorage,
    /// Configuration.
    config: ServerConfig,
}

pub struct ServerConfig {
    pub listen_addr: String,
    pub node_id: u64,
    pub data_dir: String,
    pub peers: Vec<String>,
}
```

> **Programming Concept: Why Separate Raft and Storage?**
>
> The server owns `RaftNode` and `MvccStorage` as siblings, not as a parent-child relationship. This is intentional. The read path bypasses Raft entirely and goes straight to storage. If `RaftNode` owned the storage, every read would have to go through the Raft layer, adding unnecessary overhead.
>
> Think of it like a restaurant: the manager (Server) oversees both the dining room (Raft -- coordinates with other restaurants) and the kitchen (Storage -- where the food is made). The dining room handles orders from customers, but sometimes someone just walks into the kitchen directly (reads).

### Step 2: The query router

The server's main job is routing queries to the right path:

```rust,ignore
impl Server {
    pub fn new(config: ServerConfig) -> Result<Self, String> {
        let storage = MvccStorage::new(&config.data_dir)
            .map_err(|e| format!("storage error: {}", e))?;
        let raft = RaftNode::new(config.node_id, vec![])
            .map_err(|e| format!("raft error: {}", e))?;

        Ok(Server { raft, storage, config })
    }

    /// Execute a SQL query.
    /// Reads go directly to storage.
    /// Writes go through Raft for replication.
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

        // Step 5: Route
        if plan.is_read_only() {
            self.execute_read(plan)
        } else {
            self.execute_write(plan)
        }
    }
}
```

Let's walk through each step:

1. **Lex:** turn the SQL string into tokens (`SELECT`, `*`, `FROM`, `users`)
2. **Parse:** turn tokens into a syntax tree (`Statement::Select { ... }`)
3. **Plan:** turn the syntax tree into an execution plan (`Plan::Select { ... }`)
4. **Optimize:** improve the plan (push filters down, fold constants)
5. **Route:** is this a read or a write? Send it to the right path.

Every step returns a `Result`. If any step fails, we immediately return a `Response::Error` to the client. The `match` + `return` pattern is a common way to handle errors one by one.

> **Programming Concept: Early Return for Error Handling**
>
> We use `match` + `return` to handle errors at each step. An alternative is to chain everything with the `?` operator, but that requires all error types to be compatible. The explicit `match` approach gives us control over the error message at each stage. Both patterns are valid Rust.

### Step 3: The read path

```rust,ignore
impl Server {
    /// Execute a read query directly against local storage.
    fn execute_read(&mut self, plan: Plan) -> Response {
        // Only the leader can serve reads
        if !self.raft.is_leader() {
            return Response::Error {
                message: "not the leader".to_string(),
            };
        }

        match Executor::new(&mut self.storage).execute(plan) {
            Ok(result_set) => self.result_to_response(result_set),
            Err(e) => Response::Error {
                message: format!("Execution error: {}", e),
            },
        }
    }
}
```

The read path is simple: check that we are the leader, then execute the plan directly against storage. No Raft, no replication.

### Step 4: The write path

```rust,ignore
impl Server {
    /// Execute a write query through Raft consensus.
    fn execute_write(&mut self, plan: Plan) -> Response {
        // Only the leader can accept writes
        if !self.raft.is_leader() {
            return Response::Error {
                message: "not the leader".to_string(),
            };
        }

        // Serialize the plan for Raft replication
        let command = match plan.serialize() {
            Ok(bytes) => bytes,
            Err(e) => return Response::Error {
                message: format!("Serialization error: {}", e),
            },
        };

        // Replicate through Raft -- waits until a majority confirms
        match self.raft.propose(command) {
            Ok(_) => {
                // Committed! Now execute against local storage
                match Executor::new(&mut self.storage).execute(plan) {
                    Ok(result_set) => self.result_to_response(result_set),
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

    fn result_to_response(&self, result_set: ResultSet) -> Response {
        match result_set {
            ResultSet::Rows { columns, rows } => {
                let string_rows: Vec<Vec<String>> = rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|v| v.to_string()).collect())
                    .collect();
                Response::Rows { columns, rows: string_rows }
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

The write path has two phases:

1. **Replicate:** serialize the plan, send it through Raft, wait for a majority to confirm
2. **Execute:** once committed, run the query against local storage

> **What Just Happened?**
>
> The `execute_write` method is the most complex function in the server because it bridges two worlds: consensus and execution. First it gets agreement from the cluster (Raft), then it does the actual database work (Executor). This two-phase approach ensures that even if the leader crashes after Raft commits but before execution, the next leader will re-execute the committed entries.

---

## Exercise 2: The State Machine

**Goal:** Build the bridge between Raft and the database -- the component that applies committed entries to storage.

### Step 1: What is a state machine?

In Raft, the "state machine" is whatever your application is -- for us, it is the database. Raft does not care what the state machine does. It just ensures that all nodes apply the same entries in the same order.

```rust,ignore
// src/state_machine.rs

use crate::sql::{Lexer, Parser, Planner, Optimizer, Executor};
use crate::storage::MvccStorage;

/// The database state machine.
/// Applies committed Raft entries to the MVCC storage engine.
pub struct SqlStateMachine {
    storage: MvccStorage,
}

impl SqlStateMachine {
    pub fn new(storage: MvccStorage) -> Self {
        SqlStateMachine { storage }
    }

    /// Apply a committed entry.
    /// The entry contains a SQL command that has been agreed upon
    /// by the Raft cluster.
    pub fn apply(&mut self, command: &[u8]) -> Result<(), String> {
        // Convert bytes back to SQL string
        let sql = std::str::from_utf8(command)
            .map_err(|e| format!("invalid UTF-8: {}", e))?;

        // Run the full SQL pipeline
        let tokens = Lexer::new(sql).tokenize()
            .map_err(|e| format!("lex error: {}", e))?;
        let statement = Parser::new(tokens).parse()
            .map_err(|e| format!("parse error: {}", e))?;
        let plan = Planner::new(&self.storage).plan(statement)
            .map_err(|e| format!("plan error: {}", e))?;
        let plan = Optimizer::optimize(plan);

        Executor::new(&mut self.storage).execute(plan)
            .map_err(|e| format!("execute error: {}", e))?;

        Ok(())
    }
}
```

The `apply` method re-parses and re-executes the SQL. On the leader, this means the SQL is processed twice (once for planning/routing, once for application). This is slightly wasteful but simple. A production database would optimize this.

> **Programming Concept: Separation of Concerns**
>
> Notice that the state machine knows about SQL but not about Raft. And Raft knows about log entries but not about SQL. The state machine is the bridge: it converts Raft's opaque byte vectors back into SQL operations. This separation means we could replace the SQL engine with something else (a key-value store, a document database) without changing Raft.

---

## Exercise 3: Error Propagation Across Layers

**Goal:** Handle errors from different layers gracefully using Rust's `From` trait.

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
}

// Raft errors
enum RaftError {
    NotLeader,
    ReplicationFailed(String),
}
```

The server needs to handle all three. Without a unifying type, you end up with deeply nested `match` expressions.

### Step 2: A unified error type

```rust,ignore
// src/error.rs

/// Unified error type for the database server.
#[derive(Debug)]
pub enum DbError {
    /// SQL parsing or planning error (user's fault)
    Sql(String),
    /// Storage engine error (system fault)
    Storage(String),
    /// Raft consensus error (cluster issue)
    Raft(String),
    /// Internal error (bug)
    Internal(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sql(msg) => write!(f, "SQL error: {}", msg),
            DbError::Storage(msg) => write!(f, "Storage error: {}", msg),
            DbError::Raft(msg) => write!(f, "Raft error: {}", msg),
            DbError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}
```

### Step 3: Implement `From` for automatic conversion

```rust,ignore
impl From<SqlError> for DbError {
    fn from(e: SqlError) -> Self {
        DbError::Sql(e.to_string())
    }
}

impl From<StorageError> for DbError {
    fn from(e: StorageError) -> Self {
        DbError::Storage(e.to_string())
    }
}

impl From<RaftError> for DbError {
    fn from(e: RaftError) -> Self {
        DbError::Raft(e.to_string())
    }
}
```

Now you can use the `?` operator across layer boundaries:

```rust,ignore
fn execute_query(&mut self, sql: &str) -> Result<Response, DbError> {
    let tokens = Lexer::new(sql).tokenize()?;     // SqlError -> DbError
    let stmt = Parser::new(tokens).parse()?;       // SqlError -> DbError
    let plan = Planner::new(&self.storage).plan(stmt)?;  // SqlError -> DbError

    self.raft.propose(plan.serialize()?)?;          // RaftError -> DbError

    let result = Executor::new(&mut self.storage)
        .execute(plan)?;                            // StorageError -> DbError

    Ok(self.result_to_response(result))
}
```

> **What Just Happened?**
>
> By implementing `From` for each error type, we let the `?` operator automatically convert errors from any layer into our unified `DbError` type. The calling code does not need explicit `match` statements for each error type -- `?` handles the conversion. This makes the code much cleaner while still preserving information about where the error came from.

---

## Exercise 4: End-to-End Test

**Goal:** Write a test that sends SQL through the complete stack and verifies the result.

### Step 1: The test

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_then_select() {
        // Create a server with a temporary data directory
        let dir = tempfile::tempdir().unwrap();
        let config = ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
            node_id: 1,
            data_dir: dir.path().to_str().unwrap().to_string(),
            peers: vec![],  // single-node cluster for testing
        };

        let mut server = Server::new(config).unwrap();

        // Make this node the leader (single-node cluster)
        server.raft.become_leader_for_test();

        // Create a table
        let response = server.execute("CREATE TABLE users (id INT, name TEXT)");
        match &response {
            Response::Ok { message } => {
                assert!(message.contains("created"));
            }
            Response::Error { message } => {
                panic!("CREATE TABLE failed: {}", message);
            }
            _ => panic!("unexpected response"),
        }

        // Insert a row
        let response = server.execute("INSERT INTO users VALUES (1, 'Alice')");
        match &response {
            Response::Ok { message } => {
                assert!(message.contains("1 row"));
            }
            Response::Error { message } => {
                panic!("INSERT failed: {}", message);
            }
            _ => panic!("unexpected response"),
        }

        // Query the row back
        let response = server.execute("SELECT * FROM users WHERE id = 1");
        match &response {
            Response::Rows { columns, rows } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0][0], "1");
                assert_eq!(rows[0][1], "Alice");
            }
            Response::Error { message } => {
                panic!("SELECT failed: {}", message);
            }
            _ => panic!("unexpected response"),
        }

        println!("End-to-end test passed!");
    }
}
```

### Step 2: What this test proves

This single test exercises:

1. **Lexer:** tokenizes three different SQL statements
2. **Parser:** parses CREATE TABLE, INSERT, and SELECT
3. **Planner:** creates plans for all three
4. **Optimizer:** optimizes each plan
5. **Raft:** commits the write operations (CREATE TABLE and INSERT)
6. **Executor:** executes all three plans
7. **MVCC Storage:** stores the table definition and row, reads them back
8. **Error handling:** all error types propagate correctly

If this test passes, every layer of your database is working and integrated correctly.

> **Common Mistake: Testing Without Making the Node Leader**
>
> In a single-node test cluster, the node needs to be explicitly made the leader (or you need to wait for it to elect itself). Otherwise, all writes and reads will fail with "not the leader." Add a `become_leader_for_test()` method that sets the state directly.

---

## Exercise 5: Project Structure Review

**Goal:** Look at the final project structure and understand how everything fits together.

### The file layout

```
toydb/
+-- Cargo.toml
+-- src/
|   +-- lib.rs              # declares all modules
|   +-- error.rs            # unified error types
|   +-- storage/
|   |   +-- mod.rs          # re-exports
|   |   +-- kv.rs           # key-value store
|   |   +-- bitcask.rs      # on-disk storage engine
|   |   +-- mvcc.rs         # MVCC transactions
|   +-- sql/
|   |   +-- mod.rs          # re-exports
|   |   +-- lexer.rs        # tokenizer
|   |   +-- parser.rs       # SQL parser
|   |   +-- planner.rs      # query planner
|   |   +-- optimizer.rs    # query optimizer
|   |   +-- executor.rs     # query executor
|   +-- raft/
|   |   +-- mod.rs          # re-exports
|   |   +-- node.rs         # Raft node state machine
|   |   +-- wal.rs          # write-ahead log
|   |   +-- state.rs        # state persistence
|   +-- server.rs           # the Server struct
|   +-- protocol.rs         # wire protocol
|   +-- state_machine.rs    # bridge between Raft and SQL
+-- src/bin/
|   +-- toydb-server.rs     # main server binary
|   +-- toydb-repl.rs       # interactive client
+-- tests/
|   +-- integration.rs      # end-to-end tests
```

### The dependency graph

```
server
  +-- raft (consensus)
  +-- sql (query processing)
  |   +-- lexer
  |   +-- parser
  |   +-- planner
  |   +-- optimizer
  |   +-- executor
  +-- storage (data)
  |   +-- mvcc
  |   +-- kv / bitcask
  +-- protocol (networking)
```

Notice: `sql` does not depend on `raft`. `raft` does not depend on `sql`. `storage` does not depend on either. The `server` is the only module that knows about all layers. This clean separation means you can change the storage engine without touching SQL, or replace the SQL parser without touching Raft.

---

## Exercises

### Exercise A: Read-Only Check

Implement `Plan::is_read_only()` that returns `true` for SELECT queries and `false` for INSERT, UPDATE, DELETE, and CREATE TABLE.

<details>
<summary>Hint</summary>

```rust,ignore
impl Plan {
    pub fn is_read_only(&self) -> bool {
        match self {
            Plan::Select { .. } => true,
            Plan::Insert { .. } => false,
            Plan::Update { .. } => false,
            Plan::Delete { .. } => false,
            Plan::CreateTable { .. } => false,
        }
    }
}
```

</details>

### Exercise B: Error Context

Add context to errors so the client knows which layer failed. Instead of "syntax error at position 5", send "SQL Parse Error: syntax error at position 5".

<details>
<summary>Hint</summary>

Use the `DbError` variants to tag errors by layer. In the `Display` implementation, prefix each variant with its layer name.

</details>

### Exercise C: Query Logging

Add a log line that prints every query the server executes, including the time it took and which path (read/write) it used.

<details>
<summary>Hint</summary>

```rust,ignore
let start = std::time::Instant::now();
let response = if plan.is_read_only() {
    self.execute_read(plan)
} else {
    self.execute_write(plan)
};
let elapsed = start.elapsed();
println!(
    "[query] {} | {} | {:.3}ms",
    if plan.is_read_only() { "READ" } else { "WRITE" },
    sql,
    elapsed.as_secs_f64() * 1000.0,
);
```

</details>

---

## Summary

You connected every layer of the database into a working system:

- **Rust modules** (`mod`, `use`, `pub`) organize code into namespaces with controlled visibility
- **The Server struct** owns all layers and coordinates query execution
- **Write path:** SQL -> Lex -> Parse -> Plan -> Optimize -> Raft -> Execute -> Storage
- **Read path:** SQL -> Lex -> Parse -> Plan -> Optimize -> Execute -> Storage (no Raft)
- **The state machine** bridges Raft and SQL, applying committed entries to the database
- **Error propagation** uses `From` traits and `?` to flow errors across layer boundaries
- **End-to-end tests** verify that the entire stack works together

You built a distributed SQL database. From the storage engine to the consensus protocol, every piece is Rust code that you wrote. In the final chapter, we step back to test it thoroughly, measure its performance, and explore where to take it next.
