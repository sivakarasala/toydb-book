## Spotlight: Concurrency — Arc & Mutex

Every chapter has one spotlight concept. This chapter's spotlight is **Arc and Mutex** — Rust's primitives for sharing mutable state across threads and async tasks.

### The problem: shared mutable state

A Raft node runs multiple concurrent activities:
1. Receiving client requests (writes to the log)
2. Sending AppendEntries RPCs to followers
3. Receiving AppendEntries responses and updating match indices
4. Applying committed entries to the state machine
5. Handling election timeouts and heartbeat timers

All of these need access to the same Raft state — the log, the commit index, the current term. In a single-threaded program, this is trivial. In a concurrent program, it is a data race waiting to happen.

Most languages solve this at runtime: Go uses goroutines with channels (or `sync.Mutex`), Java uses `synchronized` blocks, Python uses the GIL (which prevents true parallelism). Rust solves it at compile time — if your code compiles, it has no data races. The tools: `Arc` for shared ownership, `Mutex` for mutual exclusion.

### Arc: shared ownership across threads

`Rc<T>` (Reference Counted) lets multiple owners share the same data, but it is not thread-safe — its reference count uses non-atomic operations. `Arc<T>` (Atomically Reference Counted) is the thread-safe version:

```rust,ignore
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);

let data_clone = Arc::clone(&data);  // increment reference count (atomic)
std::thread::spawn(move || {
    println!("Thread sees: {:?}", data_clone);  // shared read access
});

println!("Main sees: {:?}", data);  // same data, different Arc handle
```

`Arc::clone` does not clone the data — it increments an atomic reference counter and returns a new `Arc` pointing to the same allocation. When the last `Arc` is dropped, the data is deallocated. This is similar to `shared_ptr` in C++, but Rust's type system prevents the use-after-free bugs that plague C++ shared pointers.

### Mutex: mutual exclusion

`Arc<T>` gives shared read access, but it does not allow mutation. To mutate shared data, you need `Mutex<T>`:

```rust,ignore
use std::sync::{Arc, Mutex};

let counter = Arc::new(Mutex::new(0));

let handles: Vec<_> = (0..10).map(|_| {
    let counter = Arc::clone(&counter);
    std::thread::spawn(move || {
        let mut value = counter.lock().unwrap();
        *value += 1;
        // MutexGuard dropped here — lock released
    })
}).collect();

for handle in handles {
    handle.join().unwrap();
}

println!("Final: {}", *counter.lock().unwrap());  // always 10
```

`mutex.lock()` returns a `MutexGuard<T>` — a smart pointer that dereferences to the inner data and releases the lock when dropped. The lock is released automatically when the guard goes out of scope. No manual `unlock()` call needed — RAII handles it.

### The critical insight: lock scope

The most common mistake with `Mutex` in async code: holding the lock across `.await` points.

```rust,ignore
// WRONG: lock held across .await
async fn bad(state: Arc<Mutex<RaftState>>) {
    let mut state = state.lock().unwrap();
    state.log.push(entry);
    network_send(&state).await;  // BLOCKS other tasks from accessing state
    state.commit_index += 1;
}

// RIGHT: lock acquired and released in tight scopes
async fn good(state: Arc<Mutex<RaftState>>) {
    // Scope 1: modify the log
    {
        let mut state = state.lock().unwrap();
        state.log.push(entry);
    }
    // Lock released

    network_send_something().await;  // other tasks can access state

    // Scope 2: update commit index
    {
        let mut state = state.lock().unwrap();
        state.commit_index += 1;
    }
    // Lock released
}
```

The pattern: lock, do fast work, unlock, await, lock again. Keep critical sections as short as possible. If the lock is held during a network call, no other task can read or write the Raft state until the network call completes — this destroys concurrency.

### std::sync::Mutex vs tokio::sync::Mutex

Rust has two `Mutex` implementations:

| | `std::sync::Mutex` | `tokio::sync::Mutex` |
|---|---|---|
| Blocking | Blocks the OS thread | Yields to the Tokio runtime |
| Use when | Lock is held briefly (no .await inside) | Lock is held across .await points |
| Performance | Faster (no runtime overhead) | Slower (runtime coordination) |
| Guard type | `MutexGuard<T>` (not Send) | `MutexGuard<T>` (Send) |

For Raft state: use `std::sync::Mutex`. Our critical sections are pure computation — update a vector, increment a counter, compare terms. No I/O, no awaiting. The lock is held for microseconds.

### Memory ordering (brief)

`Arc` uses atomic operations for its reference count. Atomic operations have **memory orderings** that determine how operations on different variables relate to each other:

```rust,ignore
use std::sync::atomic::{AtomicU64, Ordering};

let counter = AtomicU64::new(0);
counter.fetch_add(1, Ordering::Relaxed);  // no ordering guarantees
counter.fetch_add(1, Ordering::SeqCst);   // strongest guarantees
```

For our purposes:
- `Ordering::Relaxed` is sufficient for counters that are read independently
- `Ordering::SeqCst` (sequentially consistent) is the safe default — use it when in doubt
- `Ordering::Acquire`/`Ordering::Release` are for lock-free data structures (advanced)

We use `Mutex` for our Raft state, which handles memory ordering internally. You only need to think about orderings when using raw atomics without a lock.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Shared ownership | GC handles it | GC handles it | GC handles it | `Arc<T>` (explicit) |
> | Mutual exclusion | N/A (single-threaded) | `threading.Lock()` | `sync.Mutex` | `std::sync::Mutex` |
> | Lock + unlock | N/A | `with lock:` (context manager) | `mu.Lock()` + `defer mu.Unlock()` | `let guard = mutex.lock()` (auto-release) |
> | Data race prevention | N/A (single-threaded) | GIL (accidental) | Runtime race detector | Compile-time (type system) |
> | Async shared state | N/A (single-threaded) | `asyncio.Lock` | channels or `sync.Mutex` | `Arc<Mutex<T>>` or `Arc<tokio::sync::Mutex<T>>` |
>
> The key Rust difference: data races are compile-time errors, not runtime bugs. If you forget to use `Arc`, the compiler rejects your code. If you forget to lock the `Mutex`, the compiler rejects your code. In Go, you can forget to lock a `sync.Mutex` and the program compiles fine — the data race shows up under load in production (or if you run the race detector). In Rust, the type system makes this category of bug impossible.

---
