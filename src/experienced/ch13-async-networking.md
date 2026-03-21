# Chapter 13: Async Networking with Tokio

Your database server works. A client connects, sends SQL, receives results. But try opening two terminals and connecting simultaneously — the second client hangs, waiting for the first to disconnect. Your server handles one connection at a time, because `listener.incoming()` blocks on each connection until the client is done. A production database must handle hundreds or thousands of concurrent connections. PostgreSQL forks a new process per connection. MySQL spawns a thread per connection. Modern servers like TiKV and CockroachDB use asynchronous I/O — a single thread (or a small pool of threads) multiplexes across many connections, switching between them whenever one would block.

This chapter converts your blocking TCP server to an async server using Tokio, Rust's dominant async runtime. You will learn async/await syntax, understand how the runtime schedules tasks, manage connections concurrently, and implement graceful shutdown. The spotlight concept is **async/await** — Rust's zero-cost abstraction for concurrent I/O.

By the end of this chapter, you will have:

- An async TCP server using `tokio::net::TcpListener` that handles many clients concurrently
- Per-connection tasks spawned with `tokio::spawn`
- Async read/write using `AsyncReadExt` and `AsyncWriteExt`
- Connection tracking and a maximum connections limit
- Graceful shutdown with `tokio::signal` and a shutdown broadcast channel
- A clear understanding of how Rust's async model differs from JavaScript, Python, and Go

---

## Spotlight: Async/Await

Every chapter has one spotlight concept. This chapter's spotlight is **async/await** — how Rust handles concurrent I/O without threads-per-connection and without a garbage collector.

### The problem: blocking I/O wastes threads

In Chapter 12, your server processes one connection at a time:

```rust,ignore
for stream in listener.incoming() {
    let stream = stream?;
    handle_connection(stream)?;  // blocks until this client disconnects
}
```

To handle multiple connections, the obvious fix is to spawn a thread per connection:

```rust,ignore
for stream in listener.incoming() {
    let stream = stream?;
    std::thread::spawn(move || {
        handle_connection(stream).ok();
    });
}
```

This works, but threads are expensive. Each thread needs a stack (typically 2-8 MB), a kernel data structure, and context switching between threads has measurable overhead. At 1,000 concurrent connections, you are using 2-8 GB of stack space alone. At 10,000, the operating system starts refusing to create more threads.

### Async I/O: many connections, few threads

Async I/O inverts the model. Instead of one thread per connection, you have a small pool of threads (typically equal to the number of CPU cores) and many lightweight **tasks**. When a task would block — waiting for data from a socket, waiting for a timer, waiting for a disk read — it yields control back to the runtime. The runtime picks up another task that is ready to make progress. No thread is ever idle waiting for I/O.

```
Thread-per-connection:              Async:
┌──────────────┐                    ┌──────────────┐
│ Thread 1     │                    │ Thread 1     │
│ Connection A │ (mostly idle)      │ Task A ──┐   │
│ waiting...   │                    │ Task B ──┤   │ (always busy)
├──────────────┤                    │ Task C ──┤   │
│ Thread 2     │                    │ Task D ──┘   │
│ Connection B │ (mostly idle)      ├──────────────┤
│ waiting...   │                    │ Thread 2     │
├──────────────┤                    │ Task E ──┐   │
│ Thread 3     │                    │ Task F ──┤   │ (always busy)
│ Connection C │ (mostly idle)      │ Task G ──┘   │
│ waiting...   │                    └──────────────┘
├──────────────┤
│ ...1000 more │
│ threads      │
└──────────────┘
```

A database connection spends most of its time waiting — waiting for the client to send a query, waiting for disk I/O, waiting for the response to be sent. Async lets you use that idle time to serve other connections.

### `async fn` and `.await`

In Rust, `async fn` declares a function that returns a `Future` instead of executing immediately:

```rust,ignore
// Synchronous: runs to completion, blocks if it needs to wait
fn read_query(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf)?;  // blocks the thread
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

// Async: returns a Future, does not block the thread
async fn read_query(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await?;  // yields, resumes when data arrives
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}
```

The `.await` keyword is where the magic happens. When the runtime hits `.await`, it checks whether the future is ready. If the data is already available, execution continues immediately. If not, the task is suspended and the thread moves on to other work. When the data arrives (the OS signals that the socket has bytes), the runtime resumes the task right where it left off.

This is zero-cost in an important sense: if a future completes immediately, there is no allocation, no context switch, no overhead. The state machine generated by the compiler is no larger than the equivalent manual state management code.

### Futures: the trait behind async

Every `async fn` returns a type that implements `Future`:

```rust,ignore
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

You rarely implement `Future` yourself. The compiler transforms `async fn` into a state machine that implements `Future` automatically. Each `.await` point becomes a state transition. The result is a single allocation-free struct that the runtime polls to completion.

### The Tokio runtime

Rust does not include an async runtime in its standard library — it only provides the `Future` trait and `async`/`await` syntax. You need a runtime to actually execute futures. Tokio is the most widely used runtime:

```rust,ignore
// Start the Tokio runtime and run an async main function
#[tokio::main]
async fn main() {
    println!("This runs inside the Tokio runtime");
}
```

`#[tokio::main]` is a macro that creates a multi-threaded runtime and blocks on your async main function. Under the hood, it expands to:

```rust,ignore
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("This runs inside the Tokio runtime");
    });
}
```

The runtime manages a thread pool (by default, one thread per CPU core), a task scheduler, timers, and I/O event notification (using `epoll` on Linux, `kqueue` on macOS, `IOCP` on Windows).

### `tokio::spawn`: lightweight tasks

`tokio::spawn` creates a new task — similar to `std::thread::spawn`, but tasks are much lighter. A task is a future that the runtime schedules cooperatively:

```rust,ignore
// Thread: ~2-8 MB stack, kernel-managed
std::thread::spawn(|| {
    handle_connection(stream);
});

// Task: ~few hundred bytes, runtime-managed
tokio::spawn(async move {
    handle_connection(stream).await;
});
```

Tasks are multiplexed onto the runtime's thread pool. You can spawn millions of tasks — the only limit is memory for their state machines, which are typically a few hundred bytes to a few kilobytes each.

The `async move` block captures variables by ownership (like `move` closures). This is necessary because the task may run on a different thread than the one that spawned it.

### Async networking: `tokio::net`

Tokio provides async versions of the standard library's networking types:

```rust,ignore
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Async server
let listener = TcpListener::bind("127.0.0.1:4000").await?;
loop {
    let (stream, addr) = listener.accept().await?;
    tokio::spawn(async move {
        handle_connection(stream).await;
    });
}

// Async read/write
async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await?;
    stream.write_all(&buf[..n]).await?;
    Ok(())
}
```

`TcpListener::bind().await` is async because DNS resolution might block. `listener.accept().await` suspends until a new connection arrives. `stream.read().await` suspends until data is available. At every `.await` point, the runtime can service other tasks.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust (Tokio) |
> |---------|-----------|--------|-----|------|
> | Async function | `async function f()` | `async def f():` | goroutines (implicit) | `async fn f()` |
> | Await | `await promise` | `await coroutine` | (implicit) | `future.await` |
> | Runtime | V8 event loop (built-in) | `asyncio.run()` | Go runtime (built-in) | `#[tokio::main]` (explicit) |
> | Spawn task | `Promise.resolve().then()` | `asyncio.create_task()` | `go func()` | `tokio::spawn()` |
> | TCP listen | `net.createServer()` | `asyncio.start_server()` | `net.Listen()` | `TcpListener::bind().await` |
> | TCP read | `socket.on('data')` | `reader.read()` | `conn.Read()` | `stream.read().await` |
>
> The biggest differences: (1) Rust's async is **opt-in** — `std::net` is synchronous, you explicitly choose async with Tokio. (2) The runtime is a **library**, not built into the language — you could use a different runtime like `async-std` or `smol`. (3) Rust's async is **zero-cost** — no garbage collector, no hidden allocations, futures compile to state machines. Go achieves similar ergonomics with goroutines, but goroutines have a growable stack (starting at 2-8 KB) and are managed by Go's garbage-collected runtime.

---

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

## Rust Gym

### Drill 1: Basic Async Function

Write an async function that simulates a slow network call and returns a greeting:

```rust,ignore
use tokio::time::{sleep, Duration};

async fn fetch_greeting(name: &str) -> String {
    todo!()
}

#[tokio::main]
async fn main() {
    let start = std::time::Instant::now();

    // These should run concurrently, not sequentially
    let (greeting1, greeting2) = tokio::join!(
        fetch_greeting("Alice"),
        fetch_greeting("Bob"),
    );

    println!("{}", greeting1);
    println!("{}", greeting2);
    println!("Elapsed: {:?}", start.elapsed()); // should be ~100ms, not ~200ms
}
```

The function should sleep for 100ms (simulating a network call) and return `"Hello, {name}!"`.

<details>
<summary>Solution</summary>

```rust,ignore
use tokio::time::{sleep, Duration};

async fn fetch_greeting(name: &str) -> String {
    // Simulate a 100ms network call
    sleep(Duration::from_millis(100)).await;
    format!("Hello, {}!", name)
}

#[tokio::main]
async fn main() {
    let start = std::time::Instant::now();

    // tokio::join! runs both futures concurrently on the same task
    let (greeting1, greeting2) = tokio::join!(
        fetch_greeting("Alice"),
        fetch_greeting("Bob"),
    );

    println!("{}", greeting1);  // Hello, Alice!
    println!("{}", greeting2);  // Hello, Bob!
    println!("Elapsed: {:?}", start.elapsed()); // ~100ms
}
```

Key insight: `tokio::join!` polls both futures concurrently. Both sleeps run in parallel, so the total time is ~100ms (the max of the two), not ~200ms (the sum). This is different from calling them sequentially:

```rust,ignore
// Sequential: ~200ms total
let greeting1 = fetch_greeting("Alice").await;
let greeting2 = fetch_greeting("Bob").await;

// Concurrent: ~100ms total
let (greeting1, greeting2) = tokio::join!(
    fetch_greeting("Alice"),
    fetch_greeting("Bob"),
);
```

`tokio::join!` is for when you need ALL results. `tokio::select!` is for when you need the FIRST result.

</details>

### Drill 2: Select with Timeout

Write a function that reads from a channel with a timeout. If no message arrives within the deadline, return a default value:

```rust,ignore
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

async fn recv_with_timeout(
    rx: &mut mpsc::Receiver<String>,
    deadline: Duration,
) -> String {
    todo!()
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<String>(10);

    // Send a message after 50ms (should arrive in time)
    let tx1 = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx1.send("hello".to_string()).await.unwrap();
    });

    let result = recv_with_timeout(&mut rx, Duration::from_millis(100)).await;
    println!("Got: {}", result); // "hello"

    // No message sent — should timeout
    let result = recv_with_timeout(&mut rx, Duration::from_millis(100)).await;
    println!("Got: {}", result); // "timeout"
}
```

<details>
<summary>Solution</summary>

```rust,ignore
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

async fn recv_with_timeout(
    rx: &mut mpsc::Receiver<String>,
    deadline: Duration,
) -> String {
    match timeout(deadline, rx.recv()).await {
        Ok(Some(msg)) => msg,           // message received in time
        Ok(None) => "channel closed".to_string(),  // sender dropped
        Err(_) => "timeout".to_string(),  // deadline exceeded
    }
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<String>(10);

    let tx1 = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx1.send("hello".to_string()).await.unwrap();
    });

    let result = recv_with_timeout(&mut rx, Duration::from_millis(100)).await;
    println!("Got: {}", result); // "hello"

    let result = recv_with_timeout(&mut rx, Duration::from_millis(100)).await;
    println!("Got: {}", result); // "timeout"
}
```

`tokio::time::timeout` wraps any future with a deadline. If the inner future completes before the deadline, you get `Ok(result)`. If the deadline expires, you get `Err(Elapsed)`. The inner future is cancelled (dropped) on timeout.

This pattern is essential for database servers: you do not want a single slow client to hold resources forever. Connection timeouts, query timeouts, and idle timeouts all use this mechanism.

</details>

### Drill 3: Shared Counter with `Arc<AtomicUsize>`

Build a concurrent counter that multiple tasks increment simultaneously:

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::main]
async fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    // Spawn 100 tasks, each incrementing the counter 1000 times
    for _ in 0..100 {
        let counter = Arc::clone(&counter);
        let handle = tokio::spawn(async move {
            for _ in 0..1000 {
                // TODO: increment the counter atomically
                todo!()
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    println!("Final count: {}", counter.load(Ordering::Relaxed));
    // Should always be exactly 100_000
}
```

<details>
<summary>Solution</summary>

```rust,ignore
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::main]
async fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..100 {
        let counter = Arc::clone(&counter);
        let handle = tokio::spawn(async move {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("Final count: {}", counter.load(Ordering::Relaxed));
    // Always exactly 100_000
}
```

`fetch_add` is an atomic read-modify-write operation. Even with 100 tasks running concurrently, the final count is always exactly 100,000 — no lost increments, no data races.

`Ordering::Relaxed` is the weakest memory ordering — it guarantees atomicity but not ordering relative to other memory operations. For a simple counter, this is sufficient. Stronger orderings (`SeqCst`, `AcqRel`) add guarantees about how operations on different variables relate to each other, at the cost of potential performance overhead. We cover memory orderings in more depth in Chapter 15.

Why `AtomicUsize` instead of `Mutex<usize>`? For a simple counter, atomics are much faster — no lock acquisition, no potential contention, no risk of deadlock. Use `Mutex` when you need to protect a complex data structure; use atomics when you need to protect a single value.

</details>

### Drill 4: Async Echo Server

Build a minimal async echo server that reads messages and sends them back:

```rust,ignore
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_client(mut stream: TcpStream) -> tokio::io::Result<()> {
    todo!()
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    println!("Echo server on {}", addr);

    // Spawn a client that sends "hello" and reads the echo
    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(b"hello").await.unwrap();

        let mut buf = vec![0u8; 5];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        println!("Echo received: {}", String::from_utf8_lossy(&buf));
    });

    let (stream, _) = listener.accept().await?;
    handle_client(stream).await?;

    Ok(())
}
```

<details>
<summary>Solution</summary>

```rust,ignore
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_client(mut stream: TcpStream) -> tokio::io::Result<()> {
    let mut buf = vec![0u8; 1024];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            // Connection closed
            return Ok(());
        }
        stream.write_all(&buf[..n]).await?;
    }
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    println!("Echo server on {}", addr);

    tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(b"hello").await.unwrap();

        let mut buf = vec![0u8; 5];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        println!("Echo received: {}", String::from_utf8_lossy(&buf));
    });

    let (stream, _) = listener.accept().await?;
    handle_client(stream).await?;

    Ok(())
}
```

The echo server is the "hello world" of network programming. The pattern — read bytes, process them, write bytes — is the foundation of every server. Our database server is just an echo server with SQL parsing and query execution in between the read and write.

Note `TcpListener::bind("127.0.0.1:0")` — port 0 tells the OS to pick a random available port. This is useful for tests: no hardcoded ports, no conflicts with other running programs. `listener.local_addr()` tells you which port was assigned.

</details>

---

## DSA in Context: Concurrent Data Structures

Async networking introduces a fundamental DSA challenge: multiple tasks need to share data safely. This chapter used several concurrent data structures — each with different tradeoffs.

### Channels: producer-consumer communication

Channels are typed, async-safe queues. One task sends, another receives:

```rust,ignore
// mpsc: many producers, single consumer
let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

// Multiple tasks can clone tx and send
tokio::spawn(async move {
    tx.send("hello".to_string()).await.unwrap();
});

// One task receives
while let Some(msg) = rx.recv().await {
    println!("Got: {}", msg);
}
```

Channels decouple producers from consumers. The sender does not need to know how the receiver processes messages, and the receiver does not need to know who sent them. This is the async version of message passing — data flows through channels rather than being shared via locks.

Channel variants:
- **`mpsc`** (multi-producer, single-consumer): the most common. Multiple tasks send, one task receives. Used for work queues, event aggregation, logging.
- **`broadcast`**: one sender, many receivers. Every receiver gets every message. Used for shutdown signals, configuration changes, event notification.
- **`oneshot`**: one sender, one receiver, one message. Used for request-response patterns — "here is a task, send me the result on this channel."
- **`watch`**: one sender, many receivers, but receivers only see the latest value. Used for configuration that changes over time.

### Atomic types: lock-free primitives

Atomic types (`AtomicUsize`, `AtomicBool`, `AtomicI64`) provide lock-free, thread-safe operations on single values:

```rust,ignore
use std::sync::atomic::{AtomicUsize, Ordering};

let counter = AtomicUsize::new(0);
counter.fetch_add(1, Ordering::Relaxed);  // atomic increment
let value = counter.load(Ordering::Relaxed);  // atomic read
```

Atomics are faster than mutexes for simple operations (increment, compare-and-swap, load, store) because they use CPU-level atomic instructions instead of OS-level locks. But they only work on single values — you cannot atomically update two counters at once.

### Mutex vs RwLock vs channels

| Structure | Use when | Performance | Complexity |
|-----------|----------|-------------|------------|
| `Mutex` | Exclusive access needed | Good for short critical sections | Low |
| `RwLock` | Many readers, few writers | Better than Mutex for read-heavy | Medium |
| `mpsc` channel | Work distribution | Good for producer-consumer | Low |
| `broadcast` channel | Signaling | Good for one-to-many | Low |
| Atomics | Single-value counters/flags | Best (lock-free) | Low |

For our database:
- **Database access**: `Mutex` (short critical sections, simple)
- **Connection counting**: `AtomicUsize` (single counter, lock-free)
- **Shutdown signaling**: `broadcast` channel (one-to-many notification)
- **Metrics**: `AtomicUsize` (individual counters, lock-free)

The trend: use the simplest primitive that works. Start with `Mutex`, move to `RwLock` or channels only when you have evidence that `Mutex` is a bottleneck.

---

## System Design Corner: Connection Management at Scale

Real database servers manage connections at a level of sophistication far beyond what we have built. Understanding these patterns demonstrates depth in system design interviews.

### Connection pooling

Applications rarely create a new TCP connection per query. They use a **connection pool** — a bounded set of pre-established connections that are borrowed and returned:

```
Application Server                Connection Pool               Database Server
     │                                  │                              │
     │── get_connection() ──────────►   │                              │
     │◄─ Connection #3 ────────────── │                              │
     │                                  │                              │
     │── query("SELECT...") ──────────────────────────────────────►   │
     │◄─ ResultSet ──────────────────────────────────────────────── │
     │                                  │                              │
     │── return_connection(#3) ──────► │                              │
     │                                  │                              │
     │── get_connection() ──────────►   │                              │
     │◄─ Connection #3 (reused) ──── │                              │
```

Connection pools solve several problems:
1. **TCP handshake overhead**: establishing a TCP connection takes 1-3 round trips. Reusing connections amortizes this cost.
2. **Server resource limits**: each connection consumes server memory (buffers, session state, locks). Pooling bounds the maximum.
3. **Authentication overhead**: TLS handshakes and authentication are expensive. Pooling avoids repeating them per query.

### PgBouncer: a connection multiplexer

PgBouncer sits between applications and PostgreSQL, multiplexing many application connections onto fewer database connections:

```
100 app connections ──► PgBouncer ──► 20 PostgreSQL connections
```

Three pooling modes:
- **Session pooling**: one PG connection per client session (least efficient, most compatible)
- **Transaction pooling**: PG connections are released between transactions (good balance)
- **Statement pooling**: PG connections are released between statements (most efficient, but breaks multi-statement transactions)

### Connection lifecycle

Production databases track connections through a lifecycle:

```
CONNECTING ──► AUTHENTICATING ──► IDLE ──► ACTIVE ──► IDLE ──► ... ──► CLOSING
                                   │                    │
                                   │   timeout          │   query timeout
                                   ▼                    ▼
                                CLOSING              CLOSING
```

Timeouts at every stage:
- **Connect timeout**: how long to wait for TCP handshake (typically 5-30s)
- **Authentication timeout**: how long to wait for auth to complete (typically 10s)
- **Idle timeout**: how long an idle connection stays open (typically 5-30 minutes)
- **Query timeout**: how long a single query can run (configurable per query)
- **Statement timeout**: PostgreSQL-specific per-session timeout

### Load shedding

When a server is overloaded, accepting more connections makes things worse — each new connection consumes memory and CPU, slowing down existing queries, causing timeouts, which cause retries, which cause more load. This is a **cascading failure**.

Load shedding means rejecting requests early when the server is overloaded:

```rust,ignore
if active_connections > max_connections {
    // Reject immediately — better than accepting and timing out later
    return Err("server at capacity");
}
```

Our server does this with the semaphore pattern. Production servers use more sophisticated techniques:
- **Adaptive concurrency limiting**: adjust the limit based on response times (Netflix's concurrency-limiter)
- **Circuit breakers**: stop sending requests to a failing backend
- **Priority queues**: serve high-priority queries first, drop low-priority ones under load

> **Interview talking point:** *"Our async server uses Tokio's multi-threaded runtime to handle concurrent connections. Each connection is a lightweight task (~few hundred bytes of state), allowing thousands of concurrent connections on a single machine. We use a semaphore for connection limiting, broadcast channels for graceful shutdown signaling, and Arc<Mutex<>> for shared database access with short critical sections. For production, I would add connection pooling with PgBouncer-style multiplexing, adaptive concurrency limits based on response time percentiles, and connection lifecycle management with timeouts at each stage."*

---

## Design Insight: Complexity Is Incremental

In *A Philosophy of Software Design*, Ousterhout warns that complexity does not arrive all at once. It creeps in, one small addition at a time. No single change seems harmful, but the accumulation is devastating:

> *"Complexity is incremental: it is not one particular thing that makes a system complicated, but the accumulation of dozens or hundreds of small things."*

This chapter is a perfect case study. Compare the Chapter 12 server:

```rust,ignore
for stream in listener.incoming() {
    let stream = stream?;
    handle_connection(stream)?;
}
```

With this chapter's server:

```rust,ignore
let db = Arc::new(Mutex::new(Database::new()));
let (shutdown_tx, _) = broadcast::channel::<()>(1);
let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

loop {
    tokio::select! {
        result = listener.accept() => {
            let (stream, addr) = result?;
            let permit = semaphore.clone().try_acquire_owned()?;
            let db = Arc::clone(&db);
            let mut shutdown_rx = shutdown_tx.subscribe();
            tokio::spawn(async move {
                handle_connection_with_shutdown(stream, db, &mut shutdown_rx).await;
                drop(permit);
            });
        }
        _ = signal::ctrl_c() => {
            let _ = shutdown_tx.send(());
            break;
        }
    }
}
```

Each addition was justified:
- `Arc<Mutex<>>` — needed for shared database access
- `broadcast` channel — needed for shutdown signaling
- `Semaphore` — needed for connection limiting
- `tokio::select!` — needed to listen for both connections and shutdown
- `tokio::spawn` — needed for concurrent connection handling

No single addition was complex. But the accumulation means someone reading this code must understand async/await, `Arc`, `Mutex`, broadcast channels, semaphores, `select!`, and `spawn`. That is a lot of concepts to hold in your head simultaneously.

The lesson: before adding concurrency features, ask whether the simpler version is good enough. A sequential server that handles 100 queries per second might be sufficient for your workload. An async server is necessary at 10,000 queries per second, but the complexity cost is real. Justify it with data, not assumptions.

This principle applies broadly: async adds complexity — each `.await` point is a potential yield point that affects reasoning about state. `Arc<Mutex<>>` adds complexity — you must think about lock ordering and contention. Channels add complexity — you must think about backpressure and deadlocks. Use them when you need them, not because they seem sophisticated.

> *"Complexity is incremental: it is not one particular thing that makes a system complicated, but the accumulation of dozens or hundreds of small things."*
> — John Ousterhout

---

## What You Built

In this chapter, you:

1. **Converted the server to async** — replaced `std::net` with `tokio::net`, added `.await` to I/O calls, used `#[tokio::main]` for the runtime
2. **Spawned concurrent tasks** — `tokio::spawn` for per-connection handling, `async move` for ownership transfer, `Arc::clone` for shared state
3. **Shared the database safely** — `Arc<Mutex<Database>>` for concurrent access, short critical sections, mutex guard scoping
4. **Implemented graceful shutdown** — `tokio::signal::ctrl_c()`, broadcast channels for shutdown notification, `tokio::select!` for multiplexing
5. **Added connection management** — connection counting with `AtomicUsize`, connection limits with `Semaphore`, server metrics, load shedding

Your database is now a concurrent service. Multiple clients connect simultaneously, queries execute concurrently, and the server shuts down gracefully. This is the architecture of every modern database server.

Chapter 14 introduces distributed consensus with Raft — making your database fault-tolerant by replicating it across multiple servers. If one server crashes, the others continue serving queries.

---

### DS Deep Dive

Async I/O is built on operating system primitives — `epoll` (Linux), `kqueue` (macOS), `IOCP` (Windows). These event notification systems tell the runtime which sockets are ready for reading or writing, without blocking. This deep dive explores how Tokio's reactor translates OS events into Rust futures, how the task scheduler decides which future to poll next, and why async Rust compiles to state machines with zero runtime overhead.

**-> [Async Runtimes — "The Air Traffic Controller"](../ds-narratives/ch13-async-runtimes.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/async_protocol.rs` — async wire protocol | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — async message framing |
| `src/bin/toydb-async-server.rs` — async TCP server | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — `Server::serve()` with Tokio |
| Connection management — semaphore, metrics | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — connection lifecycle |
| Graceful shutdown — broadcast, select | [`src/server.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/server.rs) — shutdown handling |
