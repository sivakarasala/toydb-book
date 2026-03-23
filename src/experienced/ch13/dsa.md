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
