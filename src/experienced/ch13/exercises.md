## Exercise 1: Add Tokio and Convert the Server

**Goal:** Add Tokio as a dependency, convert the TCP server from Chapter 12 to async, and handle multiple connections concurrently.

### Step 1: Add Tokio to Cargo.toml

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

The `"full"` feature flag enables all Tokio features — networking, timers, synchronization, signal handling, and the multi-threaded runtime. In production, you would enable only the features you need to reduce compile times.

### Step 2: Convert the protocol to async

The wire protocol from Chapter 12 used `std::io::Read` and `std::io::Write`. We need async equivalents. Create `src/async_protocol.rs`:

```rust,ignore
// src/async_protocol.rs

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024; // 16 MB

/// Write a length-prefixed message to the stream.
pub async fn write_message(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    let len = data.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes (max {})", len, MAX_MESSAGE_SIZE),
        ));
    }
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    stream.flush().await?;
    Ok(())
}

/// Read a length-prefixed message from the stream.
/// Returns None on clean disconnect (EOF on length prefix).
pub async fn read_message(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    // Read the 4-byte length prefix
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_be_bytes(len_buf);
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes (max {})", len, MAX_MESSAGE_SIZE),
        ));
    }

    // Read the message body
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf).await?;
    Ok(Some(buf))
}
```

Notice how similar this is to the synchronous version. The only differences:

1. Functions are `async fn` instead of `fn`
2. Every I/O call has `.await` appended
3. We use `tokio::io` traits instead of `std::io` traits
4. `TcpStream` is `tokio::net::TcpStream`, not `std::net::TcpStream`

The logic is identical. The framing protocol — 4-byte big-endian length prefix, followed by the payload — does not change. Async is about *how* the I/O is scheduled, not *what* the I/O does.

### Step 3: Build the async server

Create `src/bin/toydb-async-server.rs`:

```rust,ignore
// src/bin/toydb-async-server.rs

use tokio::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

mod async_protocol;  // we will adjust this path for your project layout

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb async server listening on {}", addr);

    // Track active connections
    let active_connections = Arc::new(AtomicUsize::new(0));

    loop {
        // Accept a new connection
        let (stream, peer_addr) = listener.accept().await?;
        let count = active_connections.fetch_add(1, Ordering::Relaxed) + 1;
        println!("[{}] connected ({} active)", peer_addr, count);

        // Clone the counter for the task
        let active = Arc::clone(&active_connections);

        // Spawn a task to handle this connection
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("[{}] error: {}", peer_addr, e);
            }
            let count = active.fetch_sub(1, Ordering::Relaxed) - 1;
            println!("[{}] disconnected ({} active)", peer_addr, count);
        });
    }
}
```

The key change from Chapter 12: instead of handling each connection inline (blocking the accept loop), we `tokio::spawn` a new task. The accept loop immediately returns to waiting for the next connection. Every connection runs concurrently.

`Arc<AtomicUsize>` tracks active connections. `Arc` (Atomic Reference Counted) lets multiple tasks share the counter. `AtomicUsize` is a lock-free integer — `fetch_add` and `fetch_sub` are atomic operations that do not need a mutex. This is the right primitive for a simple counter that multiple tasks update independently.

### Step 4: Handle connections asynchronously

```rust,ignore
use tokio::net::TcpStream;
use tokio::io;

async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    loop {
        // Read a request
        let message = match async_protocol::read_message(&mut stream).await? {
            Some(msg) => msg,
            None => return Ok(()),  // clean disconnect
        };

        // Parse the request
        let request = match Request::from_bytes(&message) {
            Ok(req) => req,
            Err(e) => {
                let response = Response::Error {
                    message: format!("invalid request: {}", e),
                };
                async_protocol::write_message(&mut stream, &response.to_bytes()).await?;
                continue;
            }
        };

        // Handle the request
        match request {
            Request::Disconnect => {
                return Ok(());
            }
            Request::Query(sql) => {
                // Execute the query against the database
                let response = execute_query(&sql);
                async_protocol::write_message(&mut stream, &response.to_bytes()).await?;
            }
        }
    }
}
```

This is nearly identical to the synchronous handler. The `loop` reads requests until the client disconnects. The only difference: every I/O call has `.await`, allowing the runtime to service other connections while this one waits.

### Step 5: Test concurrent connections

Start the server:

```
$ cargo run --bin toydb-async-server
toydb async server listening on 127.0.0.1:4000
```

In two separate terminals, start two REPL clients:

```
# Terminal 2
$ cargo run --bin toydb-repl
toydb> SELECT * FROM users

# Terminal 3
$ cargo run --bin toydb-repl
toydb> SELECT * FROM orders
```

Both clients should work simultaneously. The server output shows:

```
[127.0.0.1:52001] connected (1 active)
[127.0.0.1:52002] connected (2 active)
```

In Chapter 12, the second client would have waited until the first disconnected. Now they run concurrently.

### Step 6: Understand what changed

The conceptual shift is small — "handle each connection in its own task instead of inline" — but the implications are significant:

| Aspect | Chapter 12 (sync) | This chapter (async) |
|--------|-------------------|---------------------|
| Connections | One at a time | Thousands concurrently |
| Threads | 1 | 1 per CPU core (thread pool) |
| Memory per connection | N/A (sequential) | ~few hundred bytes (task state) |
| Blocking | One slow client blocks everyone | Slow clients only block themselves |
| Dependency | `std` only | `tokio` crate |
| Complexity | Simple | Moderate (async types, lifetimes) |

The async version is more complex — you need `Arc` for shared state, `async move` blocks, and awareness of what can and cannot be held across `.await` points. But the performance difference is enormous: a synchronous server at 10 connections is already painful; an async server at 10,000 connections is routine.

<details>
<summary>Hint: If you get "use of moved value" errors</summary>

When you `tokio::spawn(async move { ... })`, the `move` keyword transfers ownership of all captured variables into the task. If you need the same variable in the next loop iteration (like `active_connections`), you must `Arc::clone()` it before the move block:

```rust,ignore
// WRONG: active_connections is moved into the first task
tokio::spawn(async move {
    // ... uses active_connections
});
// ERROR: active_connections was moved

// RIGHT: clone before moving
let active = Arc::clone(&active_connections);
tokio::spawn(async move {
    // ... uses active (the clone)
});
// active_connections is still available here
```

This pattern — clone-then-move — is idiomatic for sharing `Arc`-wrapped state across spawned tasks.

</details>

<details>
<summary>Hint: If you get "future cannot be sent between threads safely"</summary>

`tokio::spawn` requires the future to be `Send` — it must be safe to move between threads. This fails if you hold a non-`Send` type (like `Rc`, `RefCell`, or `MutexGuard`) across an `.await` point:

```rust,ignore
// WRONG: MutexGuard held across .await
let guard = mutex.lock().unwrap();
some_async_fn().await;  // ERROR: MutexGuard is not Send
drop(guard);

// RIGHT: drop the guard before .await
{
    let guard = mutex.lock().unwrap();
    // ... use guard
}  // guard dropped here
some_async_fn().await;  // OK
```

The fix is always the same: ensure non-`Send` types are dropped before any `.await` point. Use scoping blocks `{ ... }` to make the drop point explicit.

</details>

---

## Exercise 2: Shared Database with Arc<Mutex<>>

**Goal:** Share the database engine across all connection tasks using `Arc<Mutex<Database>>`, so queries from different clients all operate on the same data.

### Step 1: Wrap the database in Arc<Mutex<>>

In Chapter 12, the server owned the database directly. With concurrent tasks, multiple tasks need to access the database simultaneously. Rust's ownership rules prevent sharing a mutable reference across tasks. The solution: wrap the database in `Arc<Mutex<>>`:

```rust,ignore
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the database engine (from previous chapters)
    let database = Database::new();

    // Wrap it for sharing across tasks
    let db = Arc::new(Mutex::new(database));

    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb async server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        println!("[{}] connected", peer_addr);

        // Clone the Arc — creates a new reference, not a new database
        let db = Arc::clone(&db);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, db).await {
                eprintln!("[{}] error: {}", peer_addr, e);
            }
            println!("[{}] disconnected", peer_addr);
        });
    }
}
```

`Arc::clone(&db)` does not clone the database. It increments the reference count and returns a new `Arc` pointing to the same `Mutex<Database>`. When the task finishes, the `Arc` is dropped and the reference count decrements. When the last `Arc` is dropped, the database is cleaned up.

### Step 2: Lock the mutex to execute queries

```rust,ignore
async fn handle_connection(
    mut stream: TcpStream,
    db: Arc<Mutex<Database>>,
) -> io::Result<()> {
    loop {
        let message = match read_message(&mut stream).await? {
            Some(msg) => msg,
            None => return Ok(()),
        };

        let request = Request::from_bytes(&message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        match request {
            Request::Disconnect => return Ok(()),
            Request::Query(sql) => {
                // Lock the database, execute the query, release the lock
                let response = {
                    let mut db = db.lock().unwrap();
                    db.execute_query(&sql)
                };
                // Lock is released here (end of block)

                write_message(&mut stream, &response.to_bytes()).await?;
            }
        }
    }
}
```

Critical pattern: lock the mutex, do the work, release the lock **before** any `.await`. The lock scope is the `{ ... }` block. After the block, `db` (the `MutexGuard`) is dropped, releasing the lock. Then we `.await` the write. If we held the lock during the write, no other task could execute queries until the network write completed — defeating the purpose of async.

### Step 3: The std::sync::Mutex vs tokio::sync::Mutex decision

We use `std::sync::Mutex`, not `tokio::sync::Mutex`. This is deliberate:

```rust,ignore
// std::sync::Mutex — blocks the thread while waiting for the lock
let guard = db.lock().unwrap();

// tokio::sync::Mutex — yields to the runtime while waiting for the lock
let guard = db.lock().await;
```

Rule of thumb:
- **Short critical sections** (microseconds): use `std::sync::Mutex`. The lock is held so briefly that blocking is cheaper than the overhead of yielding to the runtime.
- **Long critical sections** (milliseconds+): use `tokio::sync::Mutex`. If the lock might be held while doing I/O, you want other tasks to run while waiting.

Our query execution is CPU-bound and fast (microseconds to milliseconds for in-memory queries). `std::sync::Mutex` is the right choice. PostgreSQL uses a similar approach — fast critical sections use spinlocks, not OS-level locks.

### Step 4: Verify concurrent access

Test with two clients:

```
# Terminal 2
toydb> INSERT INTO users VALUES (10, 'Dave', 40)
OK: inserted 1 row

# Terminal 3
toydb> SELECT * FROM users
id | name  | age
---+-------+----
1  | Alice | 30
2  | Bob   | 25
10 | Dave  | 40
(3 rows)
```

Client 3 sees the data inserted by client 2 — they share the same database.

### Understanding Arc and Mutex

```
                    ┌─────────────────────────┐
                    │    Arc<Mutex<Database>>  │
                    │    reference count: 3    │
                    │    ┌───────────────────┐ │
                    │    │    Mutex           │ │
Task A ────────────►│    │  ┌─────────────┐  │ │◄──────────── Task C
(connection 1)      │    │  │  Database    │  │ │         (connection 3)
                    │    │  │  (the data)  │  │ │
Task B ────────────►│    │  └─────────────┘  │ │
(connection 2)      │    └───────────────────┘ │
                    └─────────────────────────┘

Arc:   "How many tasks reference this?"  → reference counting
Mutex: "Who is currently using this?"    → mutual exclusion
```

- `Arc` solves the *sharing* problem: multiple tasks need a handle to the same database.
- `Mutex` solves the *mutation* problem: only one task can modify the database at a time.
- Together, they provide safe concurrent access without data races, enforced at compile time by Rust's type system.

<details>
<summary>Hint: Why not RwLock?</summary>

`std::sync::RwLock` allows multiple simultaneous readers OR one exclusive writer. This seems ideal — SELECTs can run concurrently, only writes need exclusive access. And for a production database, you would indeed use an `RwLock` (or MVCC, which Chapter 5 covered).

For our toy database, `Mutex` is simpler and sufficient. The locking overhead is not the bottleneck — query execution time dominates. An `RwLock` adds complexity (potential writer starvation, priority policies) without measurable benefit at our scale.

</details>

---

## Exercise 3: Graceful Shutdown

**Goal:** Implement graceful shutdown — when the server receives Ctrl+C (SIGINT), it stops accepting new connections, waits for active connections to finish their current query, and exits cleanly.

### Step 1: Why graceful shutdown matters

Abrupt termination (`kill -9`, Ctrl+C without handling) is dangerous for a database:
- Queries in progress are interrupted mid-execution
- Partially written data may corrupt the database
- Clients receive connection reset errors with no explanation
- Resources (file descriptors, locks) are not cleaned up

Graceful shutdown means: stop accepting new work, finish current work, clean up, exit.

### Step 2: Use tokio::signal to catch Ctrl+C

```rust,ignore
use tokio::signal;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb async server listening on {}", addr);

    let db = Arc::new(Mutex::new(Database::new()));

    // Create a broadcast channel for shutdown signaling
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Spawn a task that waits for Ctrl+C
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("failed to listen for Ctrl+C");
        println!("\nShutdown signal received. Stopping...");
        let _ = shutdown_tx_clone.send(());
    });

    // Track active tasks so we can wait for them
    let mut active_tasks = Vec::new();

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer_addr) = result?;
                println!("[{}] connected", peer_addr);

                let db = Arc::clone(&db);
                let mut shutdown_rx = shutdown_tx.subscribe();

                let handle = tokio::spawn(async move {
                    if let Err(e) = handle_connection_with_shutdown(
                        stream, db, &mut shutdown_rx
                    ).await {
                        eprintln!("[{}] error: {}", peer_addr, e);
                    }
                    println!("[{}] disconnected", peer_addr);
                });

                active_tasks.push(handle);
            }

            _ = signal::ctrl_c() => {
                println!("\nShutdown signal received. Stopping...");
                let _ = shutdown_tx.send(());
                break;
            }
        }
    }

    // Wait for all active tasks to complete
    println!("Waiting for {} active connections to finish...", active_tasks.len());
    for handle in active_tasks {
        let _ = handle.await;
    }
    println!("All connections closed. Server stopped.");

    Ok(())
}
```

### Step 3: Understand `tokio::select!`

`tokio::select!` waits on multiple async operations simultaneously and executes the branch of whichever completes first:

```rust,ignore
tokio::select! {
    result = listener.accept() => {
        // A new connection arrived
    }
    _ = signal::ctrl_c() => {
        // Ctrl+C was pressed
    }
}
```

This is similar to Go's `select` statement for channels. Without it, we would be stuck in `listener.accept().await` and could never respond to shutdown signals.

`select!` cancels the other branches when one completes. If `ctrl_c()` fires, the pending `accept()` is cancelled. This is safe because cancellation in Tokio simply drops the future — no resources leak, no undefined behavior.

### Step 4: Make connections shutdown-aware

Each connection task receives a shutdown receiver. When the shutdown signal is sent, the task finishes its current request and exits:

```rust,ignore
use tokio::sync::broadcast;

async fn handle_connection_with_shutdown(
    mut stream: TcpStream,
    db: Arc<Mutex<Database>>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> io::Result<()> {
    loop {
        // Wait for either a message or a shutdown signal
        let message = tokio::select! {
            result = read_message(&mut stream) => {
                match result? {
                    Some(msg) => msg,
                    None => return Ok(()),
                }
            }
            _ = shutdown_rx.recv() => {
                // Shutdown signal received — send a goodbye and close
                let goodbye = Response::Error {
                    message: "server shutting down".to_string(),
                };
                let _ = write_message(&mut stream, &goodbye.to_bytes()).await;
                return Ok(());
            }
        };

        let request = Request::from_bytes(&message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        match request {
            Request::Disconnect => return Ok(()),
            Request::Query(sql) => {
                let response = {
                    let mut db = db.lock().unwrap();
                    db.execute_query(&sql)
                };
                write_message(&mut stream, &response.to_bytes()).await?;
            }
        }
    }
}
```

### Step 5: The broadcast channel pattern

```
                    broadcast::channel
                    ┌─────────────────────┐
                    │  shutdown_tx (Sender)│
                    │         │            │
Ctrl+C handler ───►│    send(())          │
                    │    │    │    │       │
                    │    ▼    ▼    ▼       │
                    │  rx1  rx2  rx3      │
                    └─────────────────────┘
                      │     │     │
                      ▼     ▼     ▼
                    Task  Task  Task
                     A     B     C
```

A broadcast channel sends a value to all subscribers. When the Ctrl+C handler sends `()`, every connection task's `shutdown_rx.recv()` returns, and the task exits gracefully.

Why broadcast and not a simple flag? Because `recv()` is an async operation — we can use it in `tokio::select!` alongside the read operation. A flag would require polling (checking the flag periodically), which adds latency and complexity.

### Step 6: Test graceful shutdown

```
$ cargo run --bin toydb-async-server
toydb async server listening on 127.0.0.1:4000
[127.0.0.1:52001] connected
[127.0.0.1:52002] connected
^C
Shutdown signal received. Stopping...
Waiting for 2 active connections to finish...
[127.0.0.1:52001] disconnected
[127.0.0.1:52002] disconnected
All connections closed. Server stopped.
```

The clients receive a "server shutting down" error, and the server waits for them to clean up before exiting.

<details>
<summary>Hint: If the server hangs on shutdown</summary>

If a client is in the middle of sending a large request, `read_message` might block indefinitely. Add a timeout:

```rust,ignore
use tokio::time::{timeout, Duration};

let message = tokio::select! {
    result = timeout(Duration::from_secs(5), read_message(&mut stream)) => {
        match result {
            Ok(Ok(Some(msg))) => msg,
            Ok(Ok(None)) => return Ok(()),     // clean disconnect
            Ok(Err(e)) => return Err(e),        // I/O error
            Err(_) => return Ok(()),            // timeout — client too slow
        }
    }
    _ = shutdown_rx.recv() => {
        return Ok(());
    }
};
```

This bounds the shutdown time: if a client does not respond within 5 seconds, the server drops the connection anyway.

</details>

---

## Exercise 4: Connection Limits and Backpressure

**Goal:** Add a maximum connection limit. When the limit is reached, new connections receive an error message and are closed. Track connection metrics.

### Step 1: Define the connection limit

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ServerState {
    active_connections: AtomicUsize,
    max_connections: usize,
    total_connections: AtomicUsize,  // lifetime counter
    total_queries: AtomicUsize,      // lifetime counter
}

impl ServerState {
    fn new(max_connections: usize) -> Self {
        ServerState {
            active_connections: AtomicUsize::new(0),
            max_connections,
            total_connections: AtomicUsize::new(0),
            total_queries: AtomicUsize::new(0),
        }
    }

    /// Try to acquire a connection slot. Returns false if at capacity.
    fn try_acquire(&self) -> bool {
        let current = self.active_connections.load(Ordering::Relaxed);
        if current >= self.max_connections {
            return false;
        }
        // Note: this is not perfectly atomic (TOCTOU), but close enough
        // for connection limiting. A semaphore would be more precise.
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        true
    }

    fn release(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    fn record_query(&self) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
    }

    fn stats(&self) -> (usize, usize, usize) {
        (
            self.active_connections.load(Ordering::Relaxed),
            self.total_connections.load(Ordering::Relaxed),
            self.total_queries.load(Ordering::Relaxed),
        )
    }
}
```

### Step 2: Enforce the limit in the accept loop

```rust,ignore
const MAX_CONNECTIONS: usize = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    let state = Arc::new(ServerState::new(MAX_CONNECTIONS));
    let db = Arc::new(Mutex::new(Database::new()));

    println!("toydb async server listening on {} (max {} connections)", addr, MAX_CONNECTIONS);

    loop {
        let (mut stream, peer_addr) = listener.accept().await?;

        if !state.try_acquire() {
            // At capacity — reject the connection
            eprintln!("[{}] rejected: at max connections ({})", peer_addr, MAX_CONNECTIONS);
            let response = Response::Error {
                message: format!(
                    "server at capacity ({} connections). Try again later.",
                    MAX_CONNECTIONS
                ),
            };
            let _ = write_message(&mut stream, &response.to_bytes()).await;
            drop(stream);  // close the connection
            continue;
        }

        let (active, total, queries) = state.stats();
        println!(
            "[{}] connected (active: {}, total: {}, queries: {})",
            peer_addr, active, total, queries
        );

        let db = Arc::clone(&db);
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, db, &state).await {
                eprintln!("[{}] error: {}", peer_addr, e);
            }
            state.release();
            let (active, total, queries) = state.stats();
            println!(
                "[{}] disconnected (active: {}, total: {}, queries: {})",
                peer_addr, active, total, queries
            );
        });
    }
}
```

### Step 3: Use a semaphore for precise limiting

The `try_acquire` method above has a time-of-check-to-time-of-use (TOCTOU) race: between checking `current < max` and incrementing, another task might also pass the check. For precise limiting, use Tokio's semaphore:

```rust,ignore
use tokio::sync::Semaphore;

let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

loop {
    let (mut stream, peer_addr) = listener.accept().await?;

    // Try to acquire a permit — fails immediately if none available
    let permit = match semaphore.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            eprintln!("[{}] rejected: at capacity", peer_addr);
            let response = Response::Error {
                message: "server at capacity".to_string(),
            };
            let _ = write_message(&mut stream, &response.to_bytes()).await;
            continue;
        }
    };

    let db = Arc::clone(&db);

    tokio::spawn(async move {
        if let Err(e) = handle_connection(stream, db).await {
            eprintln!("[{}] error: {}", peer_addr, e);
        }
        // Permit is dropped here, releasing the semaphore slot
        drop(permit);
        println!("[{}] disconnected", peer_addr);
    });
}
```

The semaphore is the correct primitive for this pattern. `try_acquire_owned()` is atomic — no TOCTOU race. The owned permit moves into the spawned task, and dropping it automatically releases the slot. No manual `release()` call needed — Rust's RAII (Resource Acquisition Is Initialization) handles cleanup.

### Step 4: Add a server status endpoint

Add a special query that returns server metrics:

```rust,ignore
async fn handle_connection(
    mut stream: TcpStream,
    db: Arc<Mutex<Database>>,
    state: &ServerState,
) -> io::Result<()> {
    loop {
        let message = match read_message(&mut stream).await? {
            Some(msg) => msg,
            None => return Ok(()),
        };

        let request = Request::from_bytes(&message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        match request {
            Request::Disconnect => return Ok(()),
            Request::Query(sql) => {
                state.record_query();

                let response = if sql.trim().to_uppercase() == "SHOW STATUS" {
                    // Built-in status command
                    let (active, total, queries) = state.stats();
                    Response::Rows {
                        columns: vec![
                            "metric".to_string(),
                            "value".to_string(),
                        ],
                        rows: vec![
                            vec!["active_connections".to_string(), active.to_string()],
                            vec!["total_connections".to_string(), total.to_string()],
                            vec!["total_queries".to_string(), queries.to_string()],
                        ],
                    }
                } else {
                    let mut db = db.lock().unwrap();
                    db.execute_query(&sql)
                };

                write_message(&mut stream, &response.to_bytes()).await?;
            }
        }
    }
}
```

### Step 5: Test the connection limit

Set `MAX_CONNECTIONS` to 2 for testing:

```
# Terminal 1 (server)
$ cargo run --bin toydb-async-server
toydb async server listening on 127.0.0.1:4000 (max 2 connections)

# Terminal 2 (client 1)
$ cargo run --bin toydb-repl
toydb> SHOW STATUS
metric              | value
--------------------+------
active_connections  | 1
total_connections   | 1
total_queries       | 1

# Terminal 3 (client 2)
$ cargo run --bin toydb-repl
Connected.

# Terminal 4 (client 3 — rejected)
$ cargo run --bin toydb-repl
ERROR: server at capacity (2 connections). Try again later.
```

### Step 6: Performance comparison

Here is a rough comparison of the three approaches for a database server handling many concurrent connections:

```
┌─────────────────────┬───────────────┬───────────────┬───────────────┐
│ Approach            │ Connections   │ Memory/conn   │ Context switch│
├─────────────────────┼───────────────┼───────────────┼───────────────┤
│ Sequential (Ch12)   │ 1             │ N/A           │ None          │
│ Thread-per-conn     │ ~1,000-10,000 │ 2-8 MB (stack)│ Expensive (OS)│
│ Async (Tokio)       │ ~100,000+     │ ~few hundred B│ Cheap (user)  │
└─────────────────────┴───────────────┴───────────────┴───────────────┘
```

The async approach wins overwhelmingly for I/O-bound workloads (which database servers are). CPU-bound workloads benefit less from async — if every connection pegs a CPU core, you need more cores, not more concurrency.

> **Coming from JS/Python/Go?**
>
> Connection limiting is universal:
>
> | Concept | Node.js | Python | Go | Rust (Tokio) |
> |---------|---------|--------|-----|------|
> | Connection limit | `server.maxConnections` | Manual counter | Manual with channels/semaphore | `tokio::sync::Semaphore` |
> | Graceful shutdown | `server.close()` + drain | `signal.signal()` + event | `context.WithCancel()` | `tokio::signal` + broadcast |
> | Server metrics | Middleware/prom-client | Prometheus client | `expvar` package | Manual `AtomicUsize` or metrics crate |
>
> Go's approach is closest to Rust's: goroutines are lightweight tasks (like Tokio tasks), `context.Context` propagates cancellation (like broadcast channels), and `sync.WaitGroup` waits for goroutines to finish (like joining `JoinHandle`s). The key difference: Go's runtime is built-in and opinionated; Rust's async ecosystem gives you choice (Tokio, async-std, smol) but requires explicit setup.

---
