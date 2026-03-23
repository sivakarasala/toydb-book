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
