# Concurrent Data Structures — "Two threads, one HashMap"

Your database is handling 10,000 queries per second across 8 threads. Each thread needs to read and write to a shared in-memory cache -- a HashMap of recently accessed rows. You wrap the HashMap in a `Mutex`, and it works. But now every thread must wait for every other thread to finish its cache operation before it can start its own. Eight threads, one lock, seven of them always waiting. Your 8-core machine performs like a single-core machine. The mutex has serialized all your concurrency away.

The fix is not "remove the lock." The fix is smarter locking -- structures designed from the ground up to let multiple threads work simultaneously without stepping on each other. Let's build one.

---

## The Naive Way

The simplest approach to sharing a HashMap across threads: wrap it in `Arc<Mutex<HashMap>>`. Every access -- read or write -- locks the entire structure:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut handles = Vec::new();

    // Spawn 8 threads, each doing 10,000 operations
    for thread_id in 0..8 {
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            let mut lock_acquisitions = 0u64;
            for i in 0..10_000 {
                let key = format!("key:{}:{}", thread_id, i);

                // Write: lock, insert, unlock
                {
                    let mut map = cache.lock().unwrap();
                    map.insert(key.clone(), format!("value:{}", i));
                    lock_acquisitions += 1;
                }

                // Read: lock, get, unlock
                {
                    let map = cache.lock().unwrap();
                    let _ = map.get(&key);
                    lock_acquisitions += 1;
                }
            }
            lock_acquisitions
        }));
    }

    let total_locks: u64 = handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum();

    println!("Total lock acquisitions: {}", total_locks);
    println!("That is {} operations serialized through one lock.", total_locks);
    println!("With 8 threads, each thread spends ~87.5% of its time waiting.");

    let cache = cache.lock().unwrap();
    println!("Cache size: {} entries", cache.len());
}
```

160,000 lock acquisitions, all serialized through a single mutex. While one thread holds the lock, the other seven are blocked. On average, each thread spends 7/8 of its time waiting. Your 8-core machine delivers the throughput of 1.14 cores.

The problem is not the mutex itself -- mutexes are fast (an uncontended lock takes about 25 nanoseconds on modern hardware). The problem is **contention**: all threads compete for the same lock. As you add more threads, each thread waits longer, and throughput plateaus or even decreases (due to cache line bouncing and context switches).

---

## The Insight

Imagine a library with one checkout desk. Eight people line up, each wanting to check out a book from a different section. They all wait for the same desk, even though their books are in different aisles and their transactions are independent.

Now imagine the library splits into 16 sections, each with its own checkout desk. Person A goes to the Fiction desk, Person B goes to the Science desk. They never interfere with each other. The only time two people wait is if they both need the Fiction desk -- and with 16 desks and 8 people, that is rare.

This is **sharded locking** (also called striped locking). Instead of one lock guarding the entire HashMap, you split the map into N shards, each with its own lock. The key's hash determines which shard it belongs to. Two operations on different shards proceed in parallel. Contention only happens when two threads hit the same shard at the same time.

With 16 shards and 8 threads, the probability of contention drops dramatically. Each shard sees roughly 1/16th of the traffic. The effective parallelism goes from 1x (single mutex) to nearly 8x (one per thread).

Java's `ConcurrentHashMap` has used this approach since JDK 1.5 (originally 16 segments). Go's `sync.Map` uses a different strategy (copy-on-write for reads, dirty map for writes). Rust's `dashmap` crate uses sharded `RwLock`s. Let's build our own.

---

## The Build

### Mutex vs RwLock: Choosing the Right Lock

Before we build the sharded map, we need to pick the right lock type. Rust offers two:

- **`Mutex<T>`**: exclusive access. Only one thread at a time, regardless of whether it is reading or writing.
- **`RwLock<T>`**: reader-writer lock. Multiple readers can hold the lock simultaneously, but a writer needs exclusive access.

For a cache (frequent reads, occasional writes), `RwLock` is the right choice. Multiple threads can read the cache concurrently without blocking each other. Only writes cause serialization.

```rust
use std::sync::RwLock;
use std::collections::HashMap;

fn main() {
    let map = RwLock::new(HashMap::new());

    // Multiple readers can hold the lock simultaneously
    {
        let _reader1 = map.read().unwrap();
        let _reader2 = map.read().unwrap(); // does NOT block
        println!("Two readers holding the lock at once: OK");
    }

    // A writer gets exclusive access
    {
        let mut writer = map.write().unwrap();
        writer.insert("key", "value");
        println!("Writer had exclusive access");
    }

    // After the writer releases, readers can proceed again
    {
        let reader = map.read().unwrap();
        println!("Reader sees: {:?}", reader.get("key"));
    }
}
```

### The Sharded HashMap

Now let's build the concurrent HashMap. We split the keyspace into N shards, each guarded by its own `RwLock`:

```rust,ignore
use std::collections::HashMap;
use std::hash::{Hash, Hasher, DefaultHasher};
use std::sync::RwLock;

const NUM_SHARDS: usize = 16;

struct ShardedMap<V> {
    shards: Vec<RwLock<HashMap<String, V>>>,
}

impl<V> ShardedMap<V> {
    fn new() -> Self {
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(HashMap::new()));
        }
        ShardedMap { shards }
    }

    /// Determine which shard a key belongs to.
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }
}
```

The `shard_index` method hashes the key and maps it to one of 16 shards. The same key always maps to the same shard, so we get consistent behavior. Different keys will spread across shards, enabling parallelism.

### Get: Read Lock on One Shard

A read operation only locks the shard that contains (or would contain) the key. All other shards remain unlocked:

```rust,ignore
impl<V: Clone> ShardedMap<V> {
    fn get(&self, key: &str) -> Option<V> {
        let idx = self.shard_index(key);
        let shard = self.shards[idx].read().unwrap();
        shard.get(key).cloned()
    }
}
```

We use `.cloned()` to return an owned value rather than a reference. The reference would borrow from the `RwLockReadGuard`, which is dropped at the end of the function. Returning a clone lets the caller use the value after the lock is released. In a real database, you would use `Arc<V>` to avoid the clone cost.

### Insert: Write Lock on One Shard

A write operation takes a write lock on the target shard. Other shards are unaffected:

```rust,ignore
impl<V> ShardedMap<V> {
    fn insert(&self, key: String, value: V) {
        let idx = self.shard_index(&key);
        let mut shard = self.shards[idx].write().unwrap();
        shard.insert(key, value);
    }
}
```

Notice that `insert` takes `&self`, not `&mut self`. The `RwLock` provides interior mutability -- we can modify the contents through a shared reference. This is essential for concurrent use, where multiple threads hold `Arc` references to the same `ShardedMap`.

### Remove and Len

```rust,ignore
impl<V> ShardedMap<V> {
    fn remove(&self, key: &str) -> Option<V> {
        let idx = self.shard_index(key);
        let mut shard = self.shards[idx].write().unwrap();
        shard.remove(key)
    }

    fn len(&self) -> usize {
        // Must lock every shard to get an accurate count.
        // This is inherently expensive -- avoid calling frequently.
        self.shards.iter()
            .map(|shard| shard.read().unwrap().len())
            .sum()
    }
}
```

The `len()` method reveals an important truth about concurrent data structures: **global operations are expensive**. To count all entries, we must read-lock every shard. If we locked them all simultaneously, we would block all writers. Instead, we lock and release each shard one at a time, but the count might be slightly inaccurate (a write to shard 5 could happen between reading shard 4 and shard 6). For a cache, this is acceptable. For a transaction system, it would not be.

### Deadlock Prevention: Lock Ordering

What if an operation needs to lock two shards at once? For example, transferring a value from one key to another. If thread A locks shard 3 then tries to lock shard 7, while thread B locks shard 7 then tries to lock shard 3, both threads wait forever -- a **deadlock**.

The classic prevention strategy: always lock shards in ascending index order:

```rust,ignore
impl<V: Clone> ShardedMap<V> {
    /// Move a value from one key to another, atomically.
    /// Both keys might be in different shards.
    fn rename(&self, from: &str, to: String) -> bool {
        let from_idx = self.shard_index(from);
        let to_idx = self.shard_index(&to);

        if from_idx == to_idx {
            // Same shard -- only one lock needed
            let mut shard = self.shards[from_idx].write().unwrap();
            if let Some(value) = shard.remove(from) {
                shard.insert(to, value);
                return true;
            }
            return false;
        }

        // Different shards -- lock in ascending order to prevent deadlock
        let (first_idx, second_idx) = if from_idx < to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };

        let mut first_shard = self.shards[first_idx].write().unwrap();
        let mut second_shard = self.shards[second_idx].write().unwrap();

        // Now figure out which is source and which is destination
        if from_idx < to_idx {
            if let Some(value) = first_shard.remove(from) {
                second_shard.insert(to, value);
                return true;
            }
        } else {
            if let Some(value) = second_shard.remove(from) {
                first_shard.insert(to, value);
                return true;
            }
        }
        false
    }
}
```

The rule is simple: **every thread that needs multiple locks acquires them in the same order**. If all threads go low-to-high, no circular wait can form, and deadlocks become impossible.

### Channels as an Alternative

Sometimes the best concurrent data structure is no shared data structure at all. Instead of multiple threads accessing a shared HashMap, you can use **channels** to funnel all operations through a single owner thread:

```rust,ignore
use std::collections::HashMap;
use std::sync::mpsc;

enum CacheCommand {
    Get { key: String, reply: mpsc::Sender<Option<String>> },
    Set { key: String, value: String },
}

fn cache_worker(rx: mpsc::Receiver<CacheCommand>) {
    let mut map = HashMap::new();

    for cmd in rx {
        match cmd {
            CacheCommand::Get { key, reply } => {
                let _ = reply.send(map.get(&key).cloned());
            }
            CacheCommand::Set { key, value } => {
                map.insert(key, value);
            }
        }
    }
}
```

This is the "actor model" -- the HashMap is owned by a single thread, and all access goes through message passing. No locks at all. The trade-off is latency: every operation requires sending a message, which is slower than an uncontended lock (microseconds vs nanoseconds). But it eliminates all possibility of deadlock and makes reasoning about correctness much simpler.

This is how Go's philosophy works: "Don't communicate by sharing memory; share memory by communicating." SQLite takes this to the extreme -- it uses a single writer with exclusive locking, and achieves remarkable throughput by keeping the lock hold time minimal.

---

## The Payoff

Let's benchmark the sharded map against a single-mutex map:

```rust
use std::collections::HashMap;
use std::hash::{Hash, Hasher, DefaultHasher};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

const NUM_SHARDS: usize = 16;

struct ShardedMap {
    shards: Vec<RwLock<HashMap<String, String>>>,
}

impl ShardedMap {
    fn new() -> Self {
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(HashMap::new()));
        }
        ShardedMap { shards }
    }

    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }

    fn get(&self, key: &str) -> Option<String> {
        let idx = self.shard_index(key);
        let shard = self.shards[idx].read().unwrap();
        shard.get(key).cloned()
    }

    fn insert(&self, key: String, value: String) {
        let idx = self.shard_index(&key);
        let mut shard = self.shards[idx].write().unwrap();
        shard.insert(key, value);
    }
}

fn benchmark_single_mutex(num_threads: usize, ops_per_thread: usize) -> u128 {
    let map: Arc<Mutex<HashMap<String, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let start = Instant::now();
    let mut handles = Vec::new();

    for t in 0..num_threads {
        let map = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..ops_per_thread {
                let key = format!("key:{}:{}", t, i);
                // 50% writes, 50% reads
                if i % 2 == 0 {
                    let mut m = map.lock().unwrap();
                    m.insert(key, format!("v:{}", i));
                } else {
                    let m = map.lock().unwrap();
                    let _ = m.get(&key);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    start.elapsed().as_millis()
}

fn benchmark_sharded(num_threads: usize, ops_per_thread: usize) -> u128 {
    let map = Arc::new(ShardedMap::new());

    let start = Instant::now();
    let mut handles = Vec::new();

    for t in 0..num_threads {
        let map = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..ops_per_thread {
                let key = format!("key:{}:{}", t, i);
                if i % 2 == 0 {
                    map.insert(key, format!("v:{}", i));
                } else {
                    let _ = map.get(&key);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    start.elapsed().as_millis()
}

fn main() {
    let ops_per_thread = 100_000;

    println!("{:>8} {:>15} {:>15} {:>10}",
             "Threads", "Single Mutex", "Sharded (16)", "Speedup");
    println!("{}", "-".repeat(52));

    for &num_threads in &[1, 2, 4, 8] {
        let single_ms = benchmark_single_mutex(num_threads, ops_per_thread);
        let sharded_ms = benchmark_sharded(num_threads, ops_per_thread);

        let speedup = if sharded_ms > 0 {
            single_ms as f64 / sharded_ms as f64
        } else {
            f64::INFINITY
        };

        println!("{:>8} {:>12}ms {:>12}ms {:>9.1}x",
                 num_threads, single_ms, sharded_ms, speedup);
    }

    let total_ops = 8 * ops_per_thread;
    println!("\nTotal operations per run: {}", total_ops);
    println!("With 1 thread: no contention, both approaches are similar.");
    println!("With 8 threads: sharded map scales nearly linearly,");
    println!("single mutex becomes the bottleneck.");
}
```

With 1 thread, both approaches are similar (no contention). As threads increase, the single mutex approach sees diminishing returns -- adding threads does not speed things up because they all serialize on the lock. The sharded map scales nearly linearly because different threads hit different shards.

---

## Complexity Table

| Operation | Single Mutex | RwLock (global) | Sharded RwLock (N shards) | Channel/Actor |
|-----------|-------------|----------------|--------------------------|---------------|
| Read (uncontended) | O(1) + lock cost | O(1) + lock cost | O(1) + lock cost | O(1) + channel cost |
| Write (uncontended) | O(1) + lock cost | O(1) + lock cost | O(1) + lock cost | O(1) + channel cost |
| Read contention | Serialized | Parallel reads | Parallel reads + shard isolation | Serialized through actor |
| Write contention | Serialized | Serialized | Reduced by factor of N | Serialized through actor |
| Deadlock risk | None (single lock) | None (single lock) | Possible (multi-shard ops) | None |
| Memory overhead | 1 lock | 1 lock | N locks + N HashMaps | Channel buffer |
| Global operations (len, iter) | O(1) under lock | O(1) under lock | O(N) lock all shards | Serialized through actor |
| Implementation complexity | Trivial | Trivial | Moderate | Low-moderate |

The right choice depends on your workload:
- **Read-heavy, low contention**: `RwLock<HashMap>` is simplest and fast enough.
- **Mixed reads/writes, high contention**: sharded `RwLock` gives the best throughput.
- **Complex multi-key operations**: channels/actor model avoids deadlock complexity.
- **Single-threaded with async**: no locking needed; use `RefCell` or just owned data.

---

## Where This Shows Up in Our Database

In Chapter 13, we add multithreaded query execution. The buffer pool -- which caches disk pages in memory -- uses a sharded lock design:

```rust,ignore
pub struct BufferPool {
    // Each shard holds a subset of cached pages
    shards: Vec<RwLock<HashMap<PageId, Arc<Page>>>>,
}

impl BufferPool {
    pub fn get_page(&self, page_id: PageId) -> Option<Arc<Page>> {
        let shard = &self.shards[page_id.0 as usize % self.shards.len()];
        shard.read().unwrap().get(&page_id).cloned()
    }
}
```

Beyond our toydb, concurrent data structures are everywhere:

- **Java's ConcurrentHashMap** pioneered the sharded lock approach. JDK 1.5 used 16 segments with separate locks. JDK 8 moved to a lock-free design using CAS (compare-and-swap) operations for reads and fine-grained locking for writes.
- **Go's sync.Map** uses a dual-map design: a read-only map (no locking for reads) and a dirty map (locked for writes). Reads are wait-free until a miss triggers promotion from the dirty map.
- **Rust's dashmap** crate is a production-quality sharded concurrent HashMap. It uses `RwLock` per shard and provides an API similar to `HashMap`. It is the go-to choice for concurrent maps in Rust.
- **PostgreSQL's buffer manager** uses a combination of shared/exclusive locks on individual buffer frames and a global lock (partitioned into multiple locks) for the buffer table lookup.
- **Lock-free data structures** (used in some high-frequency trading systems) use atomic operations (CAS, fetch-and-add) instead of locks. They are faster under extreme contention but significantly harder to implement correctly.

The general principle: start with the simplest approach (`Mutex<HashMap>`), measure contention, and only add complexity (sharding, lock-free) when the measurements prove it is necessary.

---

## Try It Yourself

### Exercise 1: Read-Write Ratio Analysis

Modify the benchmark to test different read-write ratios: 100% reads, 90/10 read/write, 50/50, 10/90, and 100% writes. For each ratio, compare single mutex vs sharded map. At what read-write ratio does the sharded map stop providing a significant advantage?

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::hash::{Hash, Hasher, DefaultHasher};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

const NUM_SHARDS: usize = 16;

struct ShardedMap {
    shards: Vec<RwLock<HashMap<String, String>>>,
}

impl ShardedMap {
    fn new() -> Self {
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(HashMap::new()));
        }
        ShardedMap { shards }
    }

    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }

    fn get(&self, key: &str) -> Option<String> {
        let idx = self.shard_index(key);
        let shard = self.shards[idx].read().unwrap();
        shard.get(key).cloned()
    }

    fn insert(&self, key: String, value: String) {
        let idx = self.shard_index(&key);
        let mut shard = self.shards[idx].write().unwrap();
        shard.insert(key, value);
    }
}

fn bench(read_pct: usize, num_threads: usize, ops_per_thread: usize, use_sharded: bool) -> u128 {
    let ops = ops_per_thread;
    let start = Instant::now();
    let mut handles = Vec::new();

    if use_sharded {
        let map = Arc::new(ShardedMap::new());
        // Pre-populate
        for i in 0..1000 {
            map.insert(format!("pre:{}", i), format!("v:{}", i));
        }
        for t in 0..num_threads {
            let map = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..ops {
                    let key = format!("pre:{}", (t * ops + i) % 1000);
                    if (i % 100) < read_pct {
                        let _ = map.get(&key);
                    } else {
                        map.insert(key, format!("v:{}", i));
                    }
                }
            }));
        }
    } else {
        let map: Arc<Mutex<HashMap<String, String>>> =
            Arc::new(Mutex::new(HashMap::new()));
        {
            let mut m = map.lock().unwrap();
            for i in 0..1000 {
                m.insert(format!("pre:{}", i), format!("v:{}", i));
            }
        }
        for t in 0..num_threads {
            let map = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..ops {
                    let key = format!("pre:{}", (t * ops + i) % 1000);
                    if (i % 100) < read_pct {
                        let m = map.lock().unwrap();
                        let _ = m.get(&key);
                    } else {
                        let mut m = map.lock().unwrap();
                        m.insert(key, format!("v:{}", i));
                    }
                }
            }));
        }
    }

    for h in handles {
        h.join().unwrap();
    }
    start.elapsed().as_millis()
}

fn main() {
    let num_threads = 8;
    let ops = 50_000;

    println!("Threads: {}, Ops per thread: {}\n", num_threads, ops);
    println!("{:>10} {:>12} {:>12} {:>10}",
             "Read %", "Mutex (ms)", "Sharded (ms)", "Speedup");
    println!("{}", "-".repeat(48));

    for &read_pct in &[100, 90, 50, 10, 0] {
        let mutex_ms = bench(read_pct, num_threads, ops, false);
        let sharded_ms = bench(read_pct, num_threads, ops, true);
        let speedup = if sharded_ms > 0 {
            mutex_ms as f64 / sharded_ms as f64
        } else {
            f64::INFINITY
        };

        println!("{:>9}% {:>10}ms {:>10}ms {:>9.1}x",
                 read_pct, mutex_ms, sharded_ms, speedup);
    }

    println!("\nKey observations:");
    println!("- At 100% reads, sharded RwLock wins big (parallel readers per shard)");
    println!("- At 100% writes, sharding still helps (different shards written in parallel)");
    println!("- The advantage is always present because sharding reduces contention,");
    println!("  but the magnitude depends on how much parallelism is possible");
}
```

</details>

### Exercise 2: LRU Eviction with Concurrent Access

Add LRU (least recently used) eviction to the sharded map. Each shard maintains its own entry count, and when a shard exceeds a capacity limit, the oldest entry is evicted. Use a `VecDeque` as the eviction order tracker within each shard.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher, DefaultHasher};
use std::sync::{Arc, RwLock};
use std::thread;

const NUM_SHARDS: usize = 4;
const MAX_PER_SHARD: usize = 100;

struct LruShard {
    map: HashMap<String, String>,
    order: VecDeque<String>, // front = oldest, back = newest
    capacity: usize,
}

impl LruShard {
    fn new(capacity: usize) -> Self {
        LruShard {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, key: &str) -> Option<String> {
        if let Some(value) = self.map.get(key) {
            // Move to back (most recently used)
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
                self.order.push_back(key.to_string());
            }
            Some(value.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, key: String, value: String) -> Option<String> {
        let mut evicted = None;

        // If key exists, update and move to back
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), value);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
            return None;
        }

        // Evict if at capacity
        if self.map.len() >= self.capacity {
            if let Some(old_key) = self.order.pop_front() {
                evicted = self.map.remove(&old_key);
            }
        }

        self.map.insert(key.clone(), value);
        self.order.push_back(key);
        evicted
    }
}

struct ConcurrentLruCache {
    shards: Vec<RwLock<LruShard>>,
}

impl ConcurrentLruCache {
    fn new(total_capacity: usize) -> Self {
        let per_shard = total_capacity / NUM_SHARDS;
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(LruShard::new(per_shard)));
        }
        ConcurrentLruCache { shards }
    }

    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }

    fn get(&self, key: &str) -> Option<String> {
        let idx = self.shard_index(key);
        // Note: get needs write lock because it updates LRU order
        let mut shard = self.shards[idx].write().unwrap();
        shard.get(key)
    }

    fn insert(&self, key: String, value: String) -> Option<String> {
        let idx = self.shard_index(&key);
        let mut shard = self.shards[idx].write().unwrap();
        shard.insert(key, value)
    }

    fn len(&self) -> usize {
        self.shards.iter()
            .map(|s| s.read().unwrap().map.len())
            .sum()
    }
}

fn main() {
    let cache = Arc::new(ConcurrentLruCache::new(MAX_PER_SHARD * NUM_SHARDS));

    // Fill the cache
    for i in 0..400 {
        cache.insert(format!("key:{}", i), format!("value:{}", i));
    }
    println!("Cache size after 400 inserts: {} (capacity: {})",
             cache.len(), MAX_PER_SHARD * NUM_SHARDS);

    // Now insert more -- should trigger evictions
    let mut evictions = 0;
    for i in 400..800 {
        if cache.insert(format!("key:{}", i), format!("value:{}", i)).is_some() {
            evictions += 1;
        }
    }
    println!("Evictions during next 400 inserts: {}", evictions);
    println!("Cache size: {} (should still be <= {})", cache.len(), MAX_PER_SHARD * NUM_SHARDS);

    // Verify old keys were evicted and new keys are present
    let old_found = (0..100).filter(|i| cache.get(&format!("key:{}", i)).is_some()).count();
    let new_found = (700..800).filter(|i| cache.get(&format!("key:{}", i)).is_some()).count();
    println!("Old keys (0-99) still present: {}", old_found);
    println!("New keys (700-799) present: {}", new_found);

    // Concurrent test
    let cache = Arc::new(ConcurrentLruCache::new(200));
    let mut handles = Vec::new();

    for t in 0..4 {
        let cache = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                let key = format!("t{}:k{}", t, i);
                cache.insert(key.clone(), format!("v:{}", i));
                let _ = cache.get(&key);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    println!("\nConcurrent test complete. Cache size: {} (max 200)", cache.len());
}
```

</details>

### Exercise 3: Atomic Counters

Not all concurrent data structures need locks. Implement a concurrent counter map using `AtomicU64` instead of locks. The map tracks hit counts for cache keys -- each access increments the counter atomically. Use `AtomicU64::fetch_add` for lock-free increments.

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

/// A concurrent counter map using atomic integers for counts
/// and a RwLock only for inserting new keys.
struct AtomicCounterMap {
    // The RwLock protects the structure of the map (adding new keys).
    // Once a key exists, its counter is updated atomically without locking.
    counters: RwLock<HashMap<String, Arc<AtomicU64>>>,
}

impl AtomicCounterMap {
    fn new() -> Self {
        AtomicCounterMap {
            counters: RwLock::new(HashMap::new()),
        }
    }

    /// Increment a counter, creating it if it does not exist.
    fn increment(&self, key: &str) -> u64 {
        // Fast path: read lock, key already exists
        {
            let map = self.counters.read().unwrap();
            if let Some(counter) = map.get(key) {
                return counter.fetch_add(1, Ordering::Relaxed) + 1;
            }
        }

        // Slow path: write lock, insert new key
        let mut map = self.counters.write().unwrap();
        // Double-check: another thread might have inserted it
        let counter = map
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Get the current count for a key.
    fn get(&self, key: &str) -> u64 {
        let map = self.counters.read().unwrap();
        map.get(key)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get all counters as a snapshot.
    fn snapshot(&self) -> Vec<(String, u64)> {
        let map = self.counters.read().unwrap();
        let mut entries: Vec<(String, u64)> = map.iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1)); // sort by count, descending
        entries
    }
}

fn main() {
    let counters = Arc::new(AtomicCounterMap::new());
    let mut handles = Vec::new();

    // 8 threads, each incrementing various counters 100,000 times
    for t in 0..8 {
        let counters = Arc::clone(&counters);
        handles.push(thread::spawn(move || {
            for i in 0..100_000 {
                // Simulate cache access patterns:
                // some keys are "hot" (accessed by all threads)
                let key = if i % 10 == 0 {
                    format!("hot:{}", i % 5)     // 5 hot keys
                } else {
                    format!("cold:{}:{}", t, i % 100) // many cold keys per thread
                };
                counters.increment(&key);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    println!("Top 10 most accessed keys:");
    println!("{:<20} {:>10}", "Key", "Count");
    println!("{}", "-".repeat(32));

    for (key, count) in counters.snapshot().iter().take(10) {
        println!("{:<20} {:>10}", key, count);
    }

    // Verify hot key counts
    let hot_total: u64 = (0..5)
        .map(|i| counters.get(&format!("hot:{}", i)))
        .sum();
    println!("\nTotal hot key accesses: {}", hot_total);
    println!("Expected: {} (8 threads x 10,000 hot ops each)", 8 * 10_000);

    // The key insight: once a counter exists in the map, incrementing it
    // uses fetch_add, which is a single CPU instruction (LOCK XADD on x86).
    // No lock is held. Multiple threads can increment different counters
    // (or even the same counter) simultaneously without any blocking.
    // The RwLock is only needed when adding a NEW key to the map.
}
```

</details>

---

## Recap

Sharing mutable state between threads is the central challenge of concurrent programming. The simplest approach -- a single `Mutex` around a `HashMap` -- works but serializes all access. Under high contention, eight threads perform like one.

Sharded locking splits the data into N independent partitions, each with its own lock. Operations on different shards proceed in parallel. With 16 shards and 8 threads, contention drops by roughly 16x. The trade-off is complexity: global operations (like counting all entries) require locking every shard, and multi-shard operations require careful lock ordering to avoid deadlock.

`RwLock` adds another dimension: multiple readers can proceed in parallel, and only writers need exclusive access. For read-heavy workloads (which describes most database caches), this alone can provide significant speedup.

Channels and the actor model offer a different trade-off: no locks at all, but all operations funnel through a single owner thread. This eliminates deadlock risk entirely and makes correctness easy to reason about, at the cost of added latency per operation.

The right answer depends on your workload. Measure first, optimize second, and always start with the simplest correct solution.
