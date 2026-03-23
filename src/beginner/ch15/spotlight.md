## Spotlight: Concurrency -- Arc & Mutex

Every chapter has one **spotlight concept**. This chapter's spotlight is **Arc and Mutex** -- Rust's tools for safely sharing data between concurrent tasks.

### The problem: sharing data

In Chapter 13, we used `tokio::spawn` to handle each client in its own task. But a Raft node needs to do many things at once:

1. Receive client requests
2. Send entries to followers
3. Receive confirmations from followers
4. Apply committed entries to the database
5. Send heartbeats on a timer

All of these need access to the same data -- the Raft log, the current term, the commit index. How do you share data between tasks safely?

Most languages let you share data freely and hope for the best. If two threads modify the same variable at the same time, you get a **data race** -- a bug where the result depends on timing. Data races cause corrupted data, crashes, and security vulnerabilities. They are notoriously hard to find because they only happen under specific timing conditions that are hard to reproduce.

Rust takes a different approach: **the compiler prevents data races entirely.** If your code compiles, it has no data races. The tools that make this possible are `Arc` and `Mutex`.

### Arc: sharing ownership (the library book analogy)

Imagine a library book. Normally, one person checks it out, reads it, and returns it. The library tracks who has the book.

Now imagine a magical library where the book can be in multiple people's hands at the same time -- but the library keeps a counter. Person A checks it out (counter: 1). Person B checks it out (counter: 2). Person A returns it (counter: 1). Person B returns it (counter: 0). When the counter reaches zero, the library puts the book back on the shelf.

That is `Arc` -- **Atomically Reference Counted**. It lets multiple owners share the same data. When the last owner drops their `Arc`, the data is freed.

```rust,ignore
use std::sync::Arc;

// Create shared data
let data = Arc::new(vec![1, 2, 3]);

// Clone the Arc -- this does NOT copy the vector!
// It just increments the reference counter.
let data_for_task = Arc::clone(&data);

// Both `data` and `data_for_task` point to the SAME vector.
println!("Main sees: {:?}", data);           // [1, 2, 3]
println!("Task sees: {:?}", data_for_task);  // [1, 2, 3]
```

> **Programming Concept: Arc vs Rc**
>
> Rust has two reference-counted smart pointers:
> - **`Rc`** (Reference Counted) -- for single-threaded code. The counter uses normal (non-atomic) operations, which are faster but not thread-safe.
> - **`Arc`** (Atomically Reference Counted) -- for multi-threaded code. The counter uses atomic CPU operations, which are safe across threads.
>
> If you try to use `Rc` with `tokio::spawn`, the compiler will refuse: "`Rc` cannot be sent between threads safely." This is Rust catching a potential data race at compile time.

### Mutex: exclusive access (the bathroom lock analogy)

`Arc` lets you share data, but only for reading. What if you need to change the data? You need a **Mutex** (mutual exclusion).

Think of a bathroom with a lock on the door. Only one person can be inside at a time. When you want to use it:

1. You check the lock. If it is unlocked, you go in and lock the door.
2. You do your business.
3. You unlock the door and leave.

If someone else tries to enter while it is locked, they wait until you come out.

```rust,ignore
use std::sync::Mutex;

// Create a mutex-protected counter
let counter = Mutex::new(0);

// Lock the mutex to access the data
{
    let mut value = counter.lock().unwrap();
    // `value` is a MutexGuard -- it acts like a mutable reference
    *value += 1;
    println!("Counter is now: {}", *value);
}
// The lock is automatically released here when `value` goes out of scope
```

The `lock()` method returns a **guard** (`MutexGuard`). The guard gives you access to the data inside. When the guard is dropped (goes out of scope), the lock is automatically released. You never need to manually "unlock" -- Rust's RAII (Resource Acquisition Is Initialization) pattern handles it.

> **What Just Happened?**
>
> `Mutex::lock()` does two things:
> 1. Waits until no one else is holding the lock
> 2. Returns a guard that gives you exclusive access to the data
>
> The `.unwrap()` handles the case where a thread panicked while holding the lock (the mutex is "poisoned"). For our purposes, unwrap is fine.

### Combining them: `Arc<Mutex<T>>`

To share mutable data across tasks:

```rust,ignore
use std::sync::{Arc, Mutex};

// The Raft state, shared across tasks
let state = Arc::new(Mutex::new(RaftState {
    log: Vec::new(),
    commit_index: 0,
    current_term: 0,
}));

// Give each task its own Arc (reference to the same data)
let state_for_task = Arc::clone(&state);

tokio::spawn(async move {
    // Lock, modify, unlock
    let mut s = state_for_task.lock().unwrap();
    s.log.push(new_entry);
    s.commit_index += 1;
    // Lock released here
});
```

The pattern is always the same:
1. Wrap your data in `Arc::new(Mutex::new(data))`
2. `Arc::clone` before each `tokio::spawn`
3. `lock().unwrap()` to access the data
4. Let the guard go out of scope to release the lock

### The critical rule: short lock scopes

The most important rule with `Mutex` in async code: **hold the lock for as short a time as possible.**

```rust,ignore
// BAD: lock held across .await
async fn bad_example(state: Arc<Mutex<RaftState>>) {
    let mut s = state.lock().unwrap();
    s.log.push(entry);
    network_send(&s).await;  // other tasks CANNOT access state while we wait!
    s.commit_index += 1;
}

// GOOD: lock acquired and released in tight scopes
async fn good_example(state: Arc<Mutex<RaftState>>) {
    // Scope 1: modify the log
    {
        let mut s = state.lock().unwrap();
        s.log.push(entry);
    }  // lock released

    network_send_something().await;  // other tasks CAN access state

    // Scope 2: update commit index
    {
        let mut s = state.lock().unwrap();
        s.commit_index += 1;
    }  // lock released
}
```

If you hold the lock during a network call, no other task can read or write the Raft state until the network call completes. That destroys concurrency. The pattern: **lock, do fast work, unlock, then await.**

> **Common Mistake: Holding MutexGuard Across `.await`**
>
> The compiler might even warn you about this in some cases. Even when it does not, holding a lock across an await point means other tasks are blocked from accessing the shared state for the entire duration of the await. Always use `{ ... }` blocks to ensure the guard is dropped before any `.await`.

---
