# Chapter 13: Async Networking with Tokio

Your database server works. Open a terminal, connect, send a SQL query, get a result. It feels like magic. But try something: open a second terminal and connect to the same server at the same time. What happens?

The second client just sits there. Frozen. Waiting. It does not crash or give an error -- it simply hangs until the first client disconnects. Only then does the second client come to life.

This is because our server from Chapter 12 handles one connection at a time. It is like a restaurant with a single waiter who takes your order, walks to the kitchen, waits for the food to cook, brings it back, clears your table -- and only then walks over to the next customer. Every other customer is just sitting there, hungry and ignored.

A real database -- PostgreSQL, MySQL, or anything you would use in production -- handles hundreds or thousands of connections simultaneously. This chapter teaches you how to do that using **async programming** with Tokio, Rust's most popular async runtime.

By the end of this chapter, you will have:

- An understanding of what async programming is and why it matters
- Tokio added to your project and `#[tokio::main]` running your server
- An async TCP server that handles many clients at the same time
- Per-connection tasks spawned with `tokio::spawn`
- Graceful shutdown so the server stops cleanly

---

## Spotlight: Async/Await

Every chapter in this book has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **async/await** -- Rust's way of doing many things at once without needing a separate thread for each one.

### The cooking analogy

Imagine you are cooking dinner. You need to:

1. Boil pasta (10 minutes)
2. Make sauce (8 minutes)
3. Toast garlic bread (5 minutes)

**The synchronous way (what our server does now):** You put the pasta on. You stand at the stove and watch it for 10 minutes. When it is done, you start the sauce. You watch that for 8 minutes. Then you toast the bread for 5 minutes. Total time: 23 minutes.

**The async way:** You put the pasta on. While it boils, you start the sauce. While both are cooking, you pop the bread in the oven. You check on each one periodically, giving attention to whichever needs it. Total time: about 10 minutes (the longest single task).

You did not clone yourself. You did not hire extra cooks. You just stopped *waiting around* and used your idle time to work on other things.

That is async programming. When our server is waiting for a client to send data (which might take seconds or minutes), it can use that waiting time to serve other clients.

### Sync vs async: a side-by-side look

Here is what our server does now (synchronous):

```
Client A connects
    Server talks to Client A...
    Client A is typing slowly...
    Server waits... waits... waits...
    Client A sends query
    Server responds
Client A disconnects
Client B connects (finally!)
    Server talks to Client B...
```

Here is what we want (asynchronous):

```
Client A connects
    Server starts talking to Client A
Client B connects
    Server starts talking to Client B (at the same time!)
Client A sends query
    Server responds to Client A
Client C connects
    Server starts talking to Client C
Client B sends query
    Server responds to Client B
```

No client has to wait for another client to finish. The server juggles all of them.

### Why not just use threads?

You might think: "Why not spawn a new thread for each client?" That works, but threads are expensive. Each thread needs its own chunk of memory (typically 2-8 megabytes for its stack). If you have 1,000 connections, that is 2-8 gigabytes of memory just for thread stacks. At 10,000, the operating system starts refusing to create more.

Async tasks are much lighter. A task might use a few hundred bytes. You can have millions of them without breaking a sweat.

```
Threads (one per connection):         Async tasks:
+------------------+                  +------------------+
| Thread 1 (2 MB)  |                  | Thread 1         |
| Client A         |                  |  Task A -+       |
| mostly waiting.. |                  |  Task B -+       |
+------------------+                  |  Task C -+       |
| Thread 2 (2 MB)  |                  |  Task D -+       |
| Client B         |                  +------------------+
| mostly waiting.. |                  | Thread 2         |
+------------------+                  |  Task E -+       |
| Thread 3 (2 MB)  |                  |  Task F -+       |
| Client C         |                  |  Task G -+       |
| mostly waiting.. |                  +------------------+
+------------------+                  2 threads, 7 tasks
| ...1000 more     |                  Uses kilobytes,
| threads (2 GB+)  |                  not gigabytes
+------------------+
```

### What is an `async fn`?

In Rust, `async fn` declares a function that can pause and resume. It does not block the thread while waiting:

```rust,ignore
// Regular function: blocks the thread while reading
fn read_data(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf)?;  // thread is stuck here until data arrives
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

// Async function: pauses, lets other tasks run while waiting
async fn read_data(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await?;  // pauses here, other tasks can run
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}
```

Notice two differences:

1. The function is declared `async fn` instead of `fn`
2. After `stream.read(&mut buf)`, we add `.await`

That `.await` is where the magic happens. It says: "If the data is not ready yet, pause me and go do something else. When the data arrives, come back and continue from here."

> **What Just Happened?**
>
> An `async fn` does not run immediately when you call it. Instead, it creates a **future** -- a description of work to be done. The `.await` keyword is what actually runs the future. Think of it like writing a recipe (calling the async function) versus actually cooking the dish (awaiting it). The recipe is just instructions on paper. Awaiting is when the cooking happens.

### What is `.await`?

Every time you see `.await`, think of it as a potential pause point. The function says: "I need something (data from a socket, a timer to fire, a file to be read). If it is ready, I will continue immediately. If it is not ready, I will step aside and let someone else use this thread."

```rust,ignore
async fn serve_client(stream: &mut TcpStream) {
    // Pause point 1: wait for the client to send data
    let request = read_message(stream).await;

    // Process the request (no pause -- this is CPU work)
    let response = process(request);

    // Pause point 2: wait for the response to be sent
    write_message(stream, &response).await;
}
```

Between pause points, the code runs exactly like normal synchronous code. The async machinery only kicks in at `.await` boundaries.

### What is Tokio?

Here is something that surprises many people: Rust does not include an async runtime. The language gives you `async` and `.await` syntax, but it does not include the machinery that actually schedules and runs async tasks. You need a separate library for that.

**Tokio** is that library. It is the most widely used async runtime in Rust. It provides:

- A **task scheduler** that decides which task runs next
- A **thread pool** (usually one thread per CPU core)
- Async versions of networking types (`TcpListener`, `TcpStream`)
- Timers, channels, and other async utilities

Think of Tokio as the restaurant manager who coordinates the waiter (your code) with the kitchen (the operating system). The waiter knows how to take orders and serve food. The manager makes sure the waiter is always busy -- never standing around waiting for one table when another table needs attention.

### `#[tokio::main]`

To use Tokio, you mark your `main` function with `#[tokio::main]`:

```rust,ignore
#[tokio::main]
async fn main() {
    println!("This runs inside the Tokio runtime!");
}
```

This special attribute does two things:

1. Creates a Tokio runtime (the task scheduler and thread pool)
2. Runs your `async fn main()` inside that runtime

Without `#[tokio::main]`, you would have to create the runtime manually:

```rust,ignore
fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        println!("This runs inside the Tokio runtime!");
    });
}
```

The attribute is just a shortcut. But it is the shortcut everyone uses.

> **What Just Happened?**
>
> `#[tokio::main]` is a **procedural macro** -- it rewrites your code at compile time. It takes your `async fn main()` and wraps it in the boilerplate needed to start the Tokio runtime. You do not need to understand macros deeply right now. Just know that this one line sets up the entire async infrastructure for your program.

---

## Exercise 1: Add Tokio and Convert the Server

**Goal:** Add Tokio as a dependency, convert the TCP server from Chapter 12 to async, and see multiple clients connect at the same time.

### Step 1: Add Tokio to Cargo.toml

Open your `Cargo.toml` and add Tokio to the `[dependencies]` section:

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

Let's break this down:

- `version = "1"` means we want any version 1.x of Tokio. Cargo will pick the latest compatible one.
- `features = ["full"]` enables all of Tokio's features. Tokio is a big library, and by default only a minimal set of features is enabled. `"full"` gives us everything: networking, timers, signal handling, the multi-threaded runtime.

> **Programming Concept: What are Features in Cargo?**
>
> Features are optional parts of a library. A library author can put some code behind a feature flag, and users choose which features they want. This keeps compile times fast when you only need a subset. For learning, `"full"` is fine. In a production project, you would list only the features you actually use, like `features = ["net", "rt-multi-thread", "macros"]`.

### Step 2: Create the async protocol module

Our wire protocol from Chapter 12 used `std::io::Read` and `std::io::Write`. We need async versions. Create a new file `src/async_protocol.rs`:

```rust,ignore
// src/async_protocol.rs

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024; // 16 MB

/// Write a length-prefixed message to the stream.
///
/// The format is simple:
///   [4 bytes: message length as big-endian u32] [N bytes: the message]
///
/// This is the same format as Chapter 12, but now using async I/O.
pub async fn write_message(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    let len = data.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes (max {})", len, MAX_MESSAGE_SIZE),
        ));
    }

    // Write the 4-byte length prefix
    stream.write_all(&len.to_be_bytes()).await?;

    // Write the actual message
    stream.write_all(data).await?;

    // Make sure everything is sent
    stream.flush().await?;

    Ok(())
}

/// Read a length-prefixed message from the stream.
/// Returns None if the client disconnected cleanly.
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

Look at how similar this is to the synchronous version from Chapter 12. The logic is identical. The only changes are:

1. Functions are `async fn` instead of `fn`
2. Every I/O call has `.await` appended
3. We use `tokio::io` traits instead of `std::io` traits
4. `TcpStream` is `tokio::net::TcpStream`, not `std::net::TcpStream`

> **What Just Happened?**
>
> We translated our synchronous protocol to async. The meaning did not change -- we still send a 4-byte length prefix followed by the message body. What changed is *how the waiting works*. In the sync version, `stream.read_exact()` blocks the entire thread until the bytes arrive. In the async version, `stream.read_exact().await` pauses just this task, freeing the thread to do other work.

Let's take a moment to understand two new traits:

- **`AsyncReadExt`** -- provides async versions of read methods like `read_exact`. It is the async counterpart of `std::io::Read`.
- **`AsyncWriteExt`** -- provides async versions of write methods like `write_all` and `flush`. It is the async counterpart of `std::io::Write`.

You import them with `use tokio::io::{AsyncReadExt, AsyncWriteExt};`. Once imported, these methods become available on Tokio's `TcpStream`.

### Step 3: Build the async server

Now let's build the server. Create `src/bin/toydb-async-server.rs`:

```rust,ignore
// src/bin/toydb-async-server.rs

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:4000";

    // Bind the TCP listener -- this is async because it might
    // involve DNS resolution
    let listener = TcpListener::bind(addr).await?;
    println!("toydb async server listening on {}", addr);

    // The main loop: accept connections forever
    loop {
        // Wait for a new client to connect
        // .await means: if no one is connecting right now, pause
        // and let other tasks run
        let (stream, peer_addr) = listener.accept().await?;
        println!("[{}] connected", peer_addr);

        // Spawn a new task to handle this client
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("[{}] error: {}", peer_addr, e);
            }
            println!("[{}] disconnected", peer_addr);
        });

        // The loop continues IMMEDIATELY to accept the next client.
        // We do NOT wait for handle_connection to finish!
    }
}
```

This is the key difference from Chapter 12. In Chapter 12, the loop looked like:

```rust,ignore
// Chapter 12 -- synchronous, one client at a time
for stream in listener.incoming() {
    let stream = stream?;
    handle_connection(stream)?;  // blocks until this client disconnects
}
```

Now we use `tokio::spawn` to hand off each client to its own task. The accept loop does not wait for the client to finish -- it immediately goes back to waiting for the next connection.

> **Programming Concept: What is `tokio::spawn`?**
>
> Think of `tokio::spawn` as hiring a helper. When a new customer walks into the restaurant, you (the main loop) say: "Hey, helper, take care of this customer." Then you go back to the door to greet the next customer. You do not follow the helper around -- they work independently.
>
> `tokio::spawn` takes an async block and creates a new **task** for it. The task runs concurrently with your main loop and all other tasks. Tasks are incredibly lightweight -- they use a few hundred bytes of memory, compared to the megabytes a thread needs.

### Step 4: The `async move` block

Look at this part again:

```rust,ignore
tokio::spawn(async move {
    if let Err(e) = handle_connection(stream).await {
        eprintln!("[{}] error: {}", peer_addr, e);
    }
    println!("[{}] disconnected", peer_addr);
});
```

Notice `async move`. The `move` keyword is important. It tells the async block: "Take ownership of `stream` and `peer_addr`. They belong to you now."

Why do we need this? Because the task might run on a different thread than the one that spawned it. Rust's ownership rules say you cannot share data between threads without explicit permission. `move` transfers ownership into the task, so the task owns the data it needs.

> **Common Mistake: Forgetting `move`**
>
> If you write `async { ... }` instead of `async move { ... }`, the async block tries to *borrow* variables from the outer scope. But the outer scope (the loop) continues immediately and might invalidate those borrows. The compiler will give you an error like:
>
> ```
> error: closure may outlive the current function, but it borrows `stream`
> ```
>
> The fix is always the same: add `move` to make `async move { ... }`.

### Step 5: Handle connections

```rust,ignore
use tokio::net::TcpStream;
use tokio::io;

async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    loop {
        // Read a request from the client
        let message = match async_protocol::read_message(&mut stream).await? {
            Some(msg) => msg,
            None => return Ok(()),  // client disconnected cleanly
        };

        // Convert the bytes to a string (the SQL query)
        let sql = String::from_utf8_lossy(&message);
        println!("  received query: {}", sql);

        // For now, just echo back a confirmation
        let response = format!("OK: received '{}'\n", sql);
        async_protocol::write_message(
            &mut stream,
            response.as_bytes(),
        ).await?;
    }
}
```

This looks almost identical to the synchronous version. The loop reads messages until the client disconnects. The only visible difference: `.await` after every I/O call.

### Step 6: Test with multiple clients

Start the server:

```
$ cargo run --bin toydb-async-server
toydb async server listening on 127.0.0.1:4000
```

Open a second terminal and connect with a client (or use `nc`/`telnet` for testing):

```
$ cargo run --bin toydb-repl
toydb> SELECT * FROM users
```

Open a **third** terminal and connect another client:

```
$ cargo run --bin toydb-repl
toydb> INSERT INTO orders VALUES (1, 'book')
```

Both clients work at the same time. The server output shows:

```
[127.0.0.1:52001] connected
[127.0.0.1:52002] connected
  received query: SELECT * FROM users
  received query: INSERT INTO orders VALUES (1, 'book')
```

In Chapter 12, the second client would have been frozen until the first disconnected. Now they run concurrently.

> **What Just Happened?**
>
> We converted a single-client server into a multi-client server with surprisingly few changes:
>
> 1. Added `tokio` dependency
> 2. Changed `fn main()` to `async fn main()` with `#[tokio::main]`
> 3. Changed `std::net` types to `tokio::net` types
> 4. Added `.await` to I/O calls
> 5. Used `tokio::spawn` to handle each client in its own task
>
> The business logic (reading messages, processing queries, writing responses) did not change at all. Async is about *how I/O is scheduled*, not *what the I/O does*.

---

## Exercise 2: Shared Database with Arc<Mutex<>>

**Goal:** Share the database engine across all client tasks so that queries from any client operate on the same data.

### The problem

Right now, each client task is independent. If you want all clients to query the same database, the database must be shared between tasks. But Rust's ownership rules say a value can have only one owner. If the main function owns the database, how can multiple tasks use it?

### Step 1: Understand Arc

`Arc` stands for **Atomically Reference Counted**. It is a smart pointer that lets multiple owners share the same data.

Here is an analogy: imagine a library book. Normally, only one person can check it out at a time. But what if the library keeps a counter on the checkout card? Person A checks it out (counter: 1). Person B also wants to read it -- the library gives them access too (counter: 2). When Person A returns it (counter: 1), the book stays available for Person B. When Person B returns it (counter: 0), the library puts it back on the shelf.

`Arc` works the same way:

```rust,ignore
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);

// Arc::clone does NOT copy the data.
// It just increments the counter.
let data_clone = Arc::clone(&data);

// Both `data` and `data_clone` point to the SAME vector.
// When the last Arc is dropped, the vector is freed.
```

> **Programming Concept: Arc vs Rc**
>
> Rust has two reference-counted types:
> - **`Rc`** (Reference Counted) -- for single-threaded use. Cheaper but cannot be sent between threads.
> - **`Arc`** (Atomically Reference Counted) -- for multi-threaded use. Uses atomic operations to safely update the counter from multiple threads.
>
> Since our async tasks might run on different threads, we need `Arc`.

### Step 2: Understand Mutex

`Arc` lets you share data, but it only gives you read access. To *change* shared data, you need a `Mutex`.

`Mutex` stands for "mutual exclusion." It is like a bathroom with a lock. Only one person can be inside at a time. When you want to go in, you lock the door. When you come out, you unlock it. If someone else tries to enter while it is locked, they wait.

```rust,ignore
use std::sync::Mutex;

let counter = Mutex::new(0);

// Lock the mutex to access the data
{
    let mut value = counter.lock().unwrap();
    *value += 1;
    // The lock is released here when `value` goes out of scope
}
// Other threads can now lock the mutex
```

The `lock()` method returns a **guard**. The guard gives you access to the data inside the mutex. When the guard is dropped (goes out of scope), the lock is automatically released. You never need to manually "unlock" -- Rust handles it for you.

### Step 3: Combine them -- `Arc<Mutex<T>>`

To share mutable data across async tasks, you combine both:

```rust,ignore
use std::sync::{Arc, Mutex};

// Create the database
let database = Database::new();

// Wrap it: Arc for sharing, Mutex for mutation
let db = Arc::new(Mutex::new(database));
```

Now `db` can be:
- **Shared** (because of `Arc` -- clone it for each task)
- **Mutated** (because of `Mutex` -- lock it, change the data, unlock)

### Step 4: Update the server

```rust,ignore
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the shared database
    let database = Database::new();
    let db = Arc::new(Mutex::new(database));

    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb async server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        println!("[{}] connected", peer_addr);

        // Clone the Arc for this task.
        // This does NOT copy the database -- it just
        // increments the reference counter.
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

Notice the pattern: **clone the Arc before the `async move` block**, then move the clone into the block. The original `db` stays in the loop for the next iteration.

> **Common Mistake: Cloning Inside the Move Block**
>
> If you try to use `db` directly inside `async move`, the `move` keyword transfers ownership of `db` into the first task. The next loop iteration cannot use `db` because it was moved. The compiler will say:
>
> ```
> error[E0382]: use of moved value: `db`
> ```
>
> The fix: always `Arc::clone(&db)` *before* the `async move` block.

### Step 5: Use the database in the connection handler

```rust,ignore
async fn handle_connection(
    mut stream: TcpStream,
    db: Arc<Mutex<Database>>,
) -> io::Result<()> {
    loop {
        let message = match async_protocol::read_message(&mut stream).await? {
            Some(msg) => msg,
            None => return Ok(()),
        };

        let sql = String::from_utf8_lossy(&message).to_string();

        // Lock the database, execute the query, release the lock
        let response = {
            let mut database = db.lock().unwrap();
            database.execute(&sql)
        };
        // The lock is released here -- the MutexGuard was dropped
        // at the end of the block above.

        let response_bytes = response.to_bytes();
        async_protocol::write_message(&mut stream, &response_bytes).await?;
    }
}
```

The critical pattern: **lock, do fast work, unlock**. We lock the mutex, execute the query (pure computation), and let the guard drop before we do any async I/O. This is important because holding a lock across an `.await` point would block other tasks from accessing the database while we wait for the network.

> **Common Mistake: Holding the Lock Across `.await`**
>
> This is wrong:
>
> ```rust,ignore
> let mut database = db.lock().unwrap();
> database.execute(&sql);
> write_message(&mut stream, &response).await;  // BAD: lock still held!
> ```
>
> While `write_message` is sending data over the network (which might take milliseconds), no other task can access the database. The fix: use a block `{ ... }` to ensure the lock is dropped before any `.await`:
>
> ```rust,ignore
> let response = {
>     let mut database = db.lock().unwrap();
>     database.execute(&sql)
> };  // lock dropped here
> write_message(&mut stream, &response).await;  // OK: lock is free
> ```

---

## Exercise 3: Tracking Active Connections

**Goal:** Keep a count of how many clients are currently connected, and set a maximum connections limit.

### Step 1: Atomic counters

For a simple counter shared across tasks, `Arc<Mutex<usize>>` works but is overkill. There is a lighter tool: **atomic integers**.

An atomic integer is a number that can be safely modified from multiple threads without a lock. It uses special CPU instructions (atomic operations) to ensure that two threads incrementing the counter at the same time do not corrupt it.

```rust,ignore
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Create a shared counter
let active_connections = Arc::new(AtomicUsize::new(0));
```

`AtomicUsize` is an unsigned integer (`usize`) that can be safely modified from multiple threads. The key methods are:

- `fetch_add(1, Ordering::Relaxed)` -- increment by 1, return the old value
- `fetch_sub(1, Ordering::Relaxed)` -- decrement by 1, return the old value
- `load(Ordering::Relaxed)` -- read the current value

> **Programming Concept: What is `Ordering`?**
>
> `Ordering` controls how the CPU handles memory operations across threads. For a simple counter, `Ordering::Relaxed` is fine -- it means "just make the increment/decrement itself safe, don't worry about ordering with other operations." More complex scenarios need stricter orderings, but you will rarely encounter those. When in doubt, `Ordering::SeqCst` (sequentially consistent) is the strictest and safest option.

### Step 2: Track connections in the server

```rust,ignore
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_CONNECTIONS: usize = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Arc::new(Mutex::new(Database::new()));
    let active_connections = Arc::new(AtomicUsize::new(0));

    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb server listening on {} (max {} connections)", addr, MAX_CONNECTIONS);

    loop {
        let (stream, peer_addr) = listener.accept().await?;

        // Check the connection limit
        let count = active_connections.load(Ordering::Relaxed);
        if count >= MAX_CONNECTIONS {
            eprintln!("[{}] rejected: too many connections ({})", peer_addr, count);
            // stream is dropped here, closing the connection
            continue;
        }

        // Increment the counter
        let count = active_connections.fetch_add(1, Ordering::Relaxed) + 1;
        println!("[{}] connected ({} active)", peer_addr, count);

        let db = Arc::clone(&db);
        let active = Arc::clone(&active_connections);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, db).await {
                eprintln!("[{}] error: {}", peer_addr, e);
            }

            // Decrement the counter when the client disconnects
            let count = active.fetch_sub(1, Ordering::Relaxed) - 1;
            println!("[{}] disconnected ({} active)", peer_addr, count);
        });
    }
}
```

> **What Just Happened?**
>
> We added connection tracking with three components:
>
> 1. **`AtomicUsize`** -- a thread-safe counter, shared via `Arc`
> 2. **`fetch_add`** -- safely increments when a client connects
> 3. **`fetch_sub`** -- safely decrements when a client disconnects
>
> The counter is always accurate, even when multiple clients connect or disconnect simultaneously. No lock needed -- atomic operations handle the thread safety internally.

---

## Exercise 4: Graceful Shutdown

**Goal:** Make the server shut down cleanly when it receives Ctrl+C, giving active connections time to finish.

### The problem

Right now, pressing Ctrl+C kills the server instantly. Any client in the middle of a query gets a broken connection. We want the server to:

1. Stop accepting new connections
2. Wait for active connections to finish (with a timeout)
3. Exit cleanly

### Step 1: Catching Ctrl+C with tokio::signal

Tokio provides `tokio::signal::ctrl_c()`, an async function that completes when the user presses Ctrl+C:

```rust,ignore
use tokio::signal;

// This waits until Ctrl+C is pressed
signal::ctrl_c().await?;
println!("Shutdown signal received");
```

But we cannot just put this in our main loop -- we need it to run *alongside* the accept loop. We need both to run at the same time, and whichever one produces a result first wins.

### Step 2: Using `tokio::select!`

`tokio::select!` waits on multiple async operations and runs the first one that completes:

```rust,ignore
use tokio::select;

loop {
    select! {
        // Branch 1: a new client connects
        result = listener.accept() => {
            let (stream, peer_addr) = result?;
            println!("[{}] connected", peer_addr);
            // ... spawn a task to handle the client
        }

        // Branch 2: Ctrl+C is pressed
        _ = signal::ctrl_c() => {
            println!("Shutting down...");
            break;  // exit the loop
        }
    }
}
```

`select!` is like standing at a fork in the road. You wait for something to happen on either path, and you take whichever one gets a signal first.

### Step 3: Notify active connections

We need a way to tell active connections: "The server is shutting down, please finish up." Tokio provides a **broadcast channel** for this -- one sender, many receivers:

```rust,ignore
use tokio::sync::broadcast;

// Create a broadcast channel
let (shutdown_tx, _) = broadcast::channel(1);
```

The sender (`shutdown_tx`) can send a message, and every receiver gets a copy. We give each connection task a receiver:

```rust,ignore
// In the accept loop, before spawning a task:
let mut shutdown_rx = shutdown_tx.subscribe();

tokio::spawn(async move {
    loop {
        select! {
            // Normal work: read from the client
            result = async_protocol::read_message(&mut stream) => {
                match result {
                    Ok(Some(msg)) => { /* handle message */ }
                    Ok(None) => break,  // client disconnected
                    Err(e) => {
                        eprintln!("error: {}", e);
                        break;
                    }
                }
            }

            // Shutdown signal received
            _ = shutdown_rx.recv() => {
                println!("[{}] shutting down connection", peer_addr);
                break;
            }
        }
    }
});
```

### Step 4: Put it all together

```rust,ignore
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::broadcast;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Arc::new(Mutex::new(Database::new()));
    let active_connections = Arc::new(AtomicUsize::new(0));
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let addr = "127.0.0.1:4000";
    let listener = TcpListener::bind(addr).await?;
    println!("toydb server listening on {}", addr);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer_addr) = result?;
                let count = active_connections.fetch_add(1, Ordering::Relaxed) + 1;
                println!("[{}] connected ({} active)", peer_addr, count);

                let db = Arc::clone(&db);
                let active = Arc::clone(&active_connections);
                let mut shutdown_rx = shutdown_tx.subscribe();

                tokio::spawn(async move {
                    handle_client_with_shutdown(
                        stream, db, &mut shutdown_rx, peer_addr
                    ).await;

                    let count = active.fetch_sub(1, Ordering::Relaxed) - 1;
                    println!("[{}] disconnected ({} active)", peer_addr, count);
                });
            }

            _ = signal::ctrl_c() => {
                println!("\nShutdown signal received");
                break;
            }
        }
    }

    // Tell all active connections to shut down
    println!("Notifying active connections...");
    let _ = shutdown_tx.send(());

    // Wait a moment for connections to finish
    let wait_time = std::time::Duration::from_secs(5);
    println!("Waiting up to {:?} for connections to close...", wait_time);
    tokio::time::sleep(wait_time).await;

    let remaining = active_connections.load(Ordering::Relaxed);
    if remaining > 0 {
        println!("{} connections did not close in time", remaining);
    }

    println!("Server stopped.");
    Ok(())
}
```

> **What Just Happened?**
>
> We built graceful shutdown with three pieces:
>
> 1. **`tokio::signal::ctrl_c()`** -- detects when the user presses Ctrl+C
> 2. **`tokio::select!`** -- runs the accept loop and the signal handler in parallel
> 3. **`broadcast::channel`** -- notifies all active connections that shutdown is happening
>
> When Ctrl+C is pressed, the server stops accepting new connections, sends a shutdown signal to all active tasks, waits 5 seconds for them to finish, and then exits. This is how production servers handle shutdown -- no client is left with a silently broken connection.

---

## Exercise 5: Understanding What Changed

**Goal:** Review the full picture of what we built and understand the sync-to-async transformation.

### The comparison

| Aspect | Chapter 12 (sync) | This chapter (async) |
|--------|-------------------|---------------------|
| Connections | One at a time | Thousands concurrently |
| Threads used | 1 | 1 per CPU core (thread pool) |
| Memory per connection | N/A (sequential) | ~few hundred bytes (task state) |
| Blocking | One slow client blocks everyone | Slow clients only block themselves |
| Dependency | `std` only | `tokio` crate |
| Shutdown | Ctrl+C kills immediately | Graceful with notification |

### The mental model

Think of the async server as a well-managed restaurant:

- **The accept loop** is the host at the front door, greeting customers and assigning them to tables.
- **`tokio::spawn`** is like hiring a waiter for each table.
- **`Arc<Mutex<Database>>`** is the shared kitchen -- all waiters bring orders there, but only one can use the stove at a time.
- **`AtomicUsize`** is the occupancy counter at the front door.
- **The shutdown broadcast** is the manager's announcement: "Last call! We are closing soon."

### Common mistakes and how to avoid them

**Mistake 1: Using `std::net` instead of `tokio::net`**

If you accidentally use `std::net::TcpListener` in an async context, the `accept()` call will block the entire thread, defeating the purpose of async. Always use `tokio::net` types inside async functions.

**Mistake 2: Forgetting `.await`**

If you call an async function but forget `.await`, the function is not actually executed. The compiler will warn you:

```
warning: unused implementer of `Future` that must be used
```

This warning means: "You created a future but never ran it." Add `.await` to fix it.

**Mistake 3: Holding `MutexGuard` across `.await`**

We covered this already, but it bears repeating. Lock, do fast work, drop the guard, then await. Never hold the guard while awaiting.

---

## Exercises

These exercises reinforce what you learned. Try them on your own before looking at hints.

### Exercise A: Connection Duration Logging

Add logging that prints how long each client was connected when they disconnect. You will need `std::time::Instant` (or `tokio::time::Instant`).

<details>
<summary>Hint</summary>

Record `Instant::now()` when the client connects. When they disconnect, print `elapsed.as_secs()`.

```rust,ignore
let connected_at = tokio::time::Instant::now();
// ... handle the client ...
let duration = connected_at.elapsed();
println!("[{}] was connected for {:.1}s", peer_addr, duration.as_secs_f64());
```

</details>

### Exercise B: Idle Timeout

Disconnect clients that have not sent any data for 30 seconds. Use `tokio::time::timeout`.

<details>
<summary>Hint</summary>

Wrap the `read_message` call with a timeout:

```rust,ignore
use tokio::time::{timeout, Duration};

let result = timeout(
    Duration::from_secs(30),
    async_protocol::read_message(&mut stream),
).await;

match result {
    Ok(Ok(Some(msg))) => { /* handle message */ }
    Ok(Ok(None)) => break,       // client disconnected
    Ok(Err(e)) => break,         // I/O error
    Err(_) => {
        println!("Client idle for 30s, disconnecting");
        break;
    }
}
```

</details>

### Exercise C: Connection Counter Display

Every 10 seconds, print a status line showing the number of active connections. Use `tokio::time::interval` to create a periodic timer, and add it as a third branch in the `select!` in your main loop.

<details>
<summary>Hint</summary>

```rust,ignore
let mut status_interval = tokio::time::interval(Duration::from_secs(10));

loop {
    tokio::select! {
        result = listener.accept() => { /* ... */ }
        _ = signal::ctrl_c() => { break; }
        _ = status_interval.tick() => {
            let count = active_connections.load(Ordering::Relaxed);
            println!("[status] {} active connections", count);
        }
    }
}
```

</details>

---

## Summary

You transformed a single-client server into a multi-client async server. Here is what you learned:

- **Async programming** lets one thread handle many connections by pausing tasks that are waiting for I/O
- **`async fn`** and **`.await`** are Rust's syntax for writing async code -- the logic looks almost identical to synchronous code
- **Tokio** is the runtime that schedules and executes async tasks
- **`#[tokio::main]`** sets up the Tokio runtime for your program
- **`tokio::spawn`** creates lightweight tasks for each connection
- **`Arc<Mutex<T>>`** shares mutable data across tasks -- `Arc` for shared ownership, `Mutex` for safe mutation
- **Atomic integers** provide lock-free counters for simple shared state
- **Graceful shutdown** uses `tokio::signal`, `tokio::select!`, and broadcast channels

In the next chapter, we tackle an entirely different problem: what happens when your server crashes? All the data is gone. We need multiple servers, and we need them to agree on the data. That is the problem of **consensus**, and the algorithm we will use is called **Raft**.
