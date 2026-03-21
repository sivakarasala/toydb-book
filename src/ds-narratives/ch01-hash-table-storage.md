# Hash Table — "The key-value locker room"

Your database has a simple job: store key-value pairs and look them up fast. A client sends `GET user:42` and you need to find the value. Right now your storage is a `Vec` of entries, and every lookup walks the entire list. That worked fine with 100 entries. But your database just hit 100,000 entries, and clients are sending 10,000 GET requests per second. Each request scans up to 100,000 entries. That is up to **one billion comparisons per second**. Your database is drowning.

The hash table brings that down to roughly 10,000 comparisons per second. One per request. Let's build one from scratch and understand why.

---

## The Naive Way

Here is what linear search looks like in a key-value store:

```rust
fn main() {
    // Simulate a database with 100,000 entries
    let entries: Vec<(String, String)> = (0..100_000)
        .map(|i| (format!("key:{}", i), format!("value:{}", i)))
        .collect();

    // Simulate 1,000 lookups (scaled down from 10,000/sec for demo)
    let mut total_comparisons: u64 = 0;
    for i in 0..1_000 {
        let target = format!("key:{}", i * 100); // spread across the keyspace
        for (idx, (key, _)) in entries.iter().enumerate() {
            total_comparisons += 1;
            if key == &target {
                break;
            }
        }
    }

    println!("Total comparisons for 1,000 lookups: {}", total_comparisons);
    // Output: roughly 50,000,000 (50 million)
    // Extrapolate to 10,000 lookups/sec: 500 million comparisons/sec
}
```

Fifty million comparisons for just 1,000 lookups. Each lookup averages 50,000 comparisons -- half the database, on average. Scale that to 10,000 requests per second and your CPU does nothing but compare strings all day. Every other operation starves.

The fundamental problem: we have no way to jump to the right entry. We are reading every name on every locker door, every single time.

---

## The Insight

Picture a locker room at a gym. Each locker has a number painted on the door -- 0, 1, 2, 3, all the way to 127. When you rent a locker, the attendant does not pick a random empty one. They take your name, run it through a formula, and hand you a number. "Smith" becomes locker 47. "Jones" becomes locker 112. When you come back the next day, they run the same formula, get 47, and walk you straight to your locker. No scanning.

That formula is a **hash function**. It takes an arbitrary key -- a string, a number, a byte sequence -- and deterministically maps it to an index in a fixed-size array. The array is the locker room. The index is the locker number. The value is what is inside.

Two people might hash to the same locker. That is a **collision**, and we need a strategy for it. The simplest one: each locker holds a small list. If two keys land in the same bucket, they share it. As long as the hash function spreads keys evenly, each bucket stays short -- one or two entries -- and lookups remain effectively instant.

Let's build the entire locker room.

---

## The Build

### The Hash Function

We need a function that turns a string into a number. The requirements:
- **Deterministic**: same input always produces the same output
- **Uniform**: outputs spread evenly across the array
- **Fast**: computing the hash should be cheaper than the comparisons it replaces

Here is a classic approach using prime multiplication:

```rust
fn hash(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 0;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}
```

Why 31? It is a small prime number. Primes distribute remainders more uniformly than composite numbers. The `wrapping_mul` and `wrapping_add` let the intermediate value overflow without panicking -- we only care about the final modulo result. Java's `String.hashCode()` uses this exact formula. It is not cryptographically secure, but for a hash table index, it is more than good enough.

### The Structure

Each bucket is a `Vec` of key-value pairs. When two keys collide, they sit side by side in the same bucket. This strategy is called **chaining**.

```rust
struct HashTable<V> {
    buckets: Vec<Vec<(String, V)>>,
    count: usize,
}

impl<V> HashTable<V> {
    fn new() -> Self {
        let initial_capacity = 16;
        let mut buckets = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            buckets.push(Vec::new());
        }
        HashTable { buckets, count: 0 }
    }

    fn capacity(&self) -> usize {
        self.buckets.len()
    }

    fn len(&self) -> usize {
        self.count
    }

    fn load_factor(&self) -> f64 {
        self.count as f64 / self.capacity() as f64
    }
}
```

The **load factor** is the ratio of stored entries to total buckets. At load factor 1.0, there are as many entries as buckets -- on average, one entry per bucket. We will resize before it gets that high.

### Insert

Hash the key, find the bucket, check for duplicates, append:

```rust
impl<V> HashTable<V> {
    fn insert(&mut self, key: String, value: V) {
        if self.load_factor() > 0.75 {
            self.resize();
        }

        let index = hash(&key, self.capacity());
        let bucket = &mut self.buckets[index];

        // If key already exists, update in place
        for entry in bucket.iter_mut() {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }

        // New key -- append to bucket
        bucket.push((key, value));
        self.count += 1;
    }
}
```

The duplicate check is important. Without it, inserting the same key twice would create two entries, and `get` would return whichever it finds first -- probably the old one. Databases cannot have that ambiguity.

### Get

Hash the key, find the bucket, scan only that bucket:

```rust
impl<V> HashTable<V> {
    fn get(&self, key: &str) -> Option<&V> {
        let index = hash(key, self.capacity());
        let bucket = &self.buckets[index];

        for entry in bucket {
            if entry.0 == key {
                return Some(&entry.1);
            }
        }
        None
    }
}
```

This is the magic moment. Instead of scanning 100,000 entries, we compute one hash, jump to one bucket, and scan maybe 1-3 entries. The cost went from O(n) to O(1).

### Remove

Hash, find, extract:

```rust
impl<V> HashTable<V> {
    fn remove(&mut self, key: &str) -> Option<V> {
        let index = hash(key, self.capacity());
        let bucket = &mut self.buckets[index];

        let pos = bucket.iter().position(|entry| entry.0 == key);
        if let Some(pos) = pos {
            self.count -= 1;
            Some(bucket.swap_remove(pos).1)
        } else {
            None
        }
    }
}
```

We use `swap_remove` instead of `remove`. Regular `remove` shifts every element after the deleted one left -- O(n) within the bucket. `swap_remove` swaps the target with the last element and pops -- O(1). Since bucket order does not matter for correctness, this is safe and fast.

### Resize: When the Locker Room Gets Crowded

When the load factor exceeds 0.75, collisions start piling up. Buckets grow longer, and those short O(1) scans become O(2), O(3), O(5)... Eventually we are back to linear search territory. The fix: double the number of buckets and rehash every entry.

```rust
impl<V> HashTable<V> {
    fn resize(&mut self) {
        let new_capacity = self.capacity() * 2;
        let mut new_buckets = Vec::with_capacity(new_capacity);
        for _ in 0..new_capacity {
            new_buckets.push(Vec::new());
        }

        // Swap in the new empty bucket array, take ownership of the old one
        let old_buckets = std::mem::replace(&mut self.buckets, new_buckets);
        self.count = 0;

        // Rehash every entry into the new, larger array
        for bucket in old_buckets {
            for (key, value) in bucket {
                let index = hash(&key, self.capacity());
                self.buckets[index].push((key, value));
                self.count += 1;
            }
        }
    }
}
```

The `std::mem::replace` trick is pure Rust elegance. We swap in the new empty buckets, take ownership of the old ones, and drain them into the new layout. No unsafe code, no dangling pointers, no double-free. The borrow checker is happy because we own both the old and new arrays at different times.

Resizing is O(n) -- every entry gets rehashed. But it happens rarely. Each resize doubles the capacity, so after n insertions, resizing has happened at most O(log n) times. The total rehash work across all resizes is O(n), which means the *amortized* cost per insert stays O(1).

---

## The Payoff

Let's solve the original problem. Here is the full, runnable implementation:

```rust
fn hash(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 0;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}

struct HashTable<V> {
    buckets: Vec<Vec<(String, V)>>,
    count: usize,
}

impl<V> HashTable<V> {
    fn new() -> Self {
        let initial_capacity = 16;
        let mut buckets = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            buckets.push(Vec::new());
        }
        HashTable { buckets, count: 0 }
    }

    fn capacity(&self) -> usize {
        self.buckets.len()
    }

    fn len(&self) -> usize {
        self.count
    }

    fn load_factor(&self) -> f64 {
        self.count as f64 / self.capacity() as f64
    }

    fn insert(&mut self, key: String, value: V) {
        if self.load_factor() > 0.75 {
            self.resize();
        }
        let index = hash(&key, self.capacity());
        let bucket = &mut self.buckets[index];
        for entry in bucket.iter_mut() {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }
        bucket.push((key, value));
        self.count += 1;
    }

    fn get(&self, key: &str) -> Option<&V> {
        let index = hash(key, self.capacity());
        let bucket = &self.buckets[index];
        for entry in bucket {
            if entry.0 == key {
                return Some(&entry.1);
            }
        }
        None
    }

    fn remove(&mut self, key: &str) -> Option<V> {
        let index = hash(key, self.capacity());
        let bucket = &mut self.buckets[index];
        let pos = bucket.iter().position(|entry| entry.0 == key);
        if let Some(pos) = pos {
            self.count -= 1;
            Some(bucket.swap_remove(pos).1)
        } else {
            None
        }
    }

    fn resize(&mut self) {
        let new_capacity = self.capacity() * 2;
        let mut new_buckets = Vec::with_capacity(new_capacity);
        for _ in 0..new_capacity {
            new_buckets.push(Vec::new());
        }
        let old_buckets = std::mem::replace(&mut self.buckets, new_buckets);
        self.count = 0;
        for bucket in old_buckets {
            for (key, value) in bucket {
                let index = hash(&key, self.capacity());
                self.buckets[index].push((key, value));
                self.count += 1;
            }
        }
    }
}

fn main() {
    // Build our database's key-value store
    let mut db = HashTable::new();

    // Insert 100,000 entries
    for i in 0..100_000 {
        db.insert(format!("key:{}", i), format!("value:{}", i));
    }

    println!("Stored {} entries in {} buckets", db.len(), db.capacity());
    println!("Load factor: {:.2}", db.load_factor());

    // Simulate 10,000 lookups
    let mut found = 0;
    for i in 0..10_000 {
        if db.get(&format!("key:{}", i)).is_some() {
            found += 1;
        }
    }
    println!("Found {}/10,000 lookups", found);

    // Test removal
    let removed = db.remove("key:42");
    println!("Removed key:42 -> {:?}", removed.map(|v| v));
    println!("key:42 after removal: {:?}", db.get("key:42"));

    // Each lookup: 1 hash computation + scan ~1-2 entries in the bucket
    // 10,000 lookups = ~10,000-20,000 comparisons
    // vs linear scan: ~500,000,000 comparisons
    println!("\nSpeedup: ~25,000x fewer comparisons than linear scan");
}
```

From 500 million comparisons to roughly 10,000. The database goes from unusable to instant. Clients stop timing out. You stop getting paged at 3 AM.

---

## Complexity Table

| Operation | Linear Scan (`Vec`) | Hash Table | Notes |
|-----------|-------------------|------------|-------|
| Lookup | O(n) | O(1) amortized | Hash + short bucket scan |
| Insert | O(1) append / O(n) dedup check | O(1) amortized | Occasional O(n) resize |
| Delete | O(n) find + O(n) shift | O(1) amortized | swap_remove within bucket |
| Build from n items | O(n) | O(n) | Each insert is amortized O(1) |
| Memory overhead | Low | Moderate | Empty buckets + Vec overhead |
| Worst case | O(n) | O(n) | All keys hash to same bucket |

The "amortized" qualifier is critical. Most operations are truly O(1) -- hash the key, index into the array, scan a bucket with 1-2 entries. The rare resize is O(n), but it happens so infrequently (doubling means O(log n) resizes total) that the average cost stays constant.

The worst case -- every key hashing to the same bucket -- is theoretically possible but practically nonexistent with a decent hash function. If someone deliberately crafts keys to cause collisions (a "hash flood" attack), you would need a randomized hash function. Rust's standard `HashMap` uses SipHash for exactly this reason.

---

## Where This Shows Up in Our Database

In Chapter 1, we build a `Database` struct that stores key-value pairs. Under the hood, it uses Rust's standard `HashMap`:

```rust,ignore
use std::collections::HashMap;

pub struct Database {
    data: HashMap<String, Vec<u8>>,
}

impl Database {
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.data.get(key).map(|v| v.as_slice())
    }

    pub fn set(&mut self, key: String, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }
}
```

That `HashMap` is doing exactly what we just built -- hashing keys, indexing into buckets, handling collisions. The standard library version uses SipHash (resistant to hash flooding), Robin Hood hashing (a more cache-friendly collision strategy), and has been optimized for years. But the core idea is identical to our 80-line implementation.

Beyond our toy database, hash tables are everywhere in real database systems:
- **Hash indexes** in PostgreSQL for equality lookups (`WHERE id = 42`)
- **Hash joins** that match rows between tables by hashing join keys
- **Buffer pools** that track which disk pages are cached in memory
- **Lock managers** that map resource identifiers to lock states

Any time a database needs O(1) lookup by an exact key, a hash table is the answer.

---

## Try It Yourself

### Exercise 1: Collision Statistics

Add a method `collision_stats(&self) -> (usize, usize, usize)` that returns `(empty_buckets, max_bucket_length, total_collisions)`. A collision is any entry that shares a bucket with at least one other entry. Insert 10,000 keys and analyze how evenly the hash function distributes them.

<details>
<summary>Solution</summary>

```rust
fn hash(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 0;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}

struct HashTable<V> {
    buckets: Vec<Vec<(String, V)>>,
    count: usize,
}

impl<V> HashTable<V> {
    fn new() -> Self {
        let initial_capacity = 16;
        let mut buckets = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            buckets.push(Vec::new());
        }
        HashTable { buckets, count: 0 }
    }

    fn capacity(&self) -> usize {
        self.buckets.len()
    }

    fn len(&self) -> usize {
        self.count
    }

    fn load_factor(&self) -> f64 {
        self.count as f64 / self.capacity() as f64
    }

    fn insert(&mut self, key: String, value: V) {
        if self.load_factor() > 0.75 {
            self.resize();
        }
        let index = hash(&key, self.capacity());
        let bucket = &mut self.buckets[index];
        for entry in bucket.iter_mut() {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }
        bucket.push((key, value));
        self.count += 1;
    }

    fn resize(&mut self) {
        let new_capacity = self.capacity() * 2;
        let mut new_buckets = Vec::with_capacity(new_capacity);
        for _ in 0..new_capacity {
            new_buckets.push(Vec::new());
        }
        let old_buckets = std::mem::replace(&mut self.buckets, new_buckets);
        self.count = 0;
        for bucket in old_buckets {
            for (key, value) in bucket {
                let index = hash(&key, self.capacity());
                self.buckets[index].push((key, value));
                self.count += 1;
            }
        }
    }

    fn collision_stats(&self) -> (usize, usize, usize) {
        let mut empty_buckets = 0;
        let mut max_bucket_len = 0;
        let mut total_collisions = 0;

        for bucket in &self.buckets {
            if bucket.is_empty() {
                empty_buckets += 1;
            }
            if bucket.len() > max_bucket_len {
                max_bucket_len = bucket.len();
            }
            // Every entry beyond the first in a bucket is a collision
            if bucket.len() > 1 {
                total_collisions += bucket.len() - 1;
            }
        }

        (empty_buckets, max_bucket_len, total_collisions)
    }
}

fn main() {
    let mut table = HashTable::new();
    for i in 0..10_000 {
        table.insert(format!("user:{}", i), i);
    }

    let (empty, max_len, collisions) = table.collision_stats();
    println!("Buckets: {} total, {} empty", table.capacity(), empty);
    println!("Max bucket length: {}", max_len);
    println!("Total collisions: {}", collisions);
    println!("Load factor: {:.2}", table.load_factor());
    // With a good hash function and load factor <= 0.75,
    // max bucket length should be ~4-6, and most buckets
    // should have 0 or 1 entries.
}
```

</details>

### Exercise 2: Iteration

Implement an `iter()` method that returns all key-value pairs. Then use it to build a `keys()` method and a `values()` method. What order do the keys come out in? (Hint: not insertion order.)

<details>
<summary>Solution</summary>

```rust
fn hash(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 0;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}

struct HashTable<V> {
    buckets: Vec<Vec<(String, V)>>,
    count: usize,
}

impl<V> HashTable<V> {
    fn new() -> Self {
        let initial_capacity = 16;
        let mut buckets = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            buckets.push(Vec::new());
        }
        HashTable { buckets, count: 0 }
    }

    fn capacity(&self) -> usize {
        self.buckets.len()
    }

    fn load_factor(&self) -> f64 {
        self.count as f64 / self.capacity() as f64
    }

    fn insert(&mut self, key: String, value: V) {
        if self.load_factor() > 0.75 {
            self.resize();
        }
        let index = hash(&key, self.capacity());
        let bucket = &mut self.buckets[index];
        for entry in bucket.iter_mut() {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }
        bucket.push((key, value));
        self.count += 1;
    }

    fn resize(&mut self) {
        let new_capacity = self.capacity() * 2;
        let mut new_buckets = Vec::with_capacity(new_capacity);
        for _ in 0..new_capacity {
            new_buckets.push(Vec::new());
        }
        let old_buckets = std::mem::replace(&mut self.buckets, new_buckets);
        self.count = 0;
        for bucket in old_buckets {
            for (key, value) in bucket {
                let index = hash(&key, self.capacity());
                self.buckets[index].push((key, value));
                self.count += 1;
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter())
            .map(|(k, v)| (k.as_str(), v))
    }

    fn keys(&self) -> Vec<&str> {
        self.iter().map(|(k, _)| k).collect()
    }

    fn values(&self) -> Vec<&V> {
        self.iter().map(|(_, v)| v).collect()
    }
}

fn main() {
    let mut table = HashTable::new();
    table.insert("alpha".to_string(), 1);
    table.insert("beta".to_string(), 2);
    table.insert("gamma".to_string(), 3);
    table.insert("delta".to_string(), 4);

    println!("Keys (in hash-order, NOT insertion order):");
    for key in table.keys() {
        println!("  {}", key);
    }

    println!("\nAll entries:");
    for (k, v) in table.iter() {
        println!("  {} => {}", k, v);
    }

    // The order depends on hash values, not insertion order.
    // This is why HashMap iteration order is "random" --
    // it is determined by the hash function and bucket layout.
}
```

</details>

### Exercise 3: The DJB2 Hash

Replace the hash function with the DJB2 algorithm: start with `hash = 5381`, then for each byte, compute `hash = hash * 33 + byte`. Insert 10,000 keys using both hash functions and compare collision statistics. Which distributes more evenly?

<details>
<summary>Solution</summary>

```rust
fn hash_prime31(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 0;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}

fn hash_djb2(key: &str, capacity: usize) -> usize {
    let mut hash_value: u64 = 5381;
    for byte in key.bytes() {
        hash_value = hash_value.wrapping_mul(33).wrapping_add(byte as u64);
    }
    (hash_value % capacity as u64) as usize
}

fn analyze_distribution(name: &str, hash_fn: fn(&str, usize) -> usize, keys: &[String]) {
    let capacity = 16384; // fixed capacity, no resizing, to compare raw distribution
    let mut buckets = vec![0usize; capacity];

    for key in keys {
        let index = hash_fn(key, capacity);
        buckets[index] += 1;
    }

    let empty = buckets.iter().filter(|&&c| c == 0).count();
    let max_len = *buckets.iter().max().unwrap();
    let collisions: usize = buckets.iter().filter(|&&c| c > 1).map(|c| c - 1).sum();

    // Standard deviation of bucket lengths
    let mean = keys.len() as f64 / capacity as f64;
    let variance: f64 = buckets.iter()
        .map(|&c| {
            let diff = c as f64 - mean;
            diff * diff
        })
        .sum::<f64>() / capacity as f64;
    let std_dev = variance.sqrt();

    println!("--- {} ---", name);
    println!("  Empty buckets: {} / {}", empty, capacity);
    println!("  Max bucket length: {}", max_len);
    println!("  Total collisions: {}", collisions);
    println!("  Std deviation of bucket sizes: {:.4}", std_dev);
    println!("  (Lower std dev = more uniform distribution)");
}

fn main() {
    let keys: Vec<String> = (0..10_000)
        .map(|i| format!("key:{}", i))
        .collect();

    analyze_distribution("Prime-31", hash_prime31, &keys);
    println!();
    analyze_distribution("DJB2", hash_djb2, &keys);

    // Both should perform similarly on sequential keys.
    // DJB2's magic constant 5381 and multiplier 33 were chosen
    // by Daniel J. Bernstein through empirical testing -- they
    // produce fewer collisions on typical string data (English
    // words, URLs, identifiers). For sequential numeric keys
    // like "key:0", "key:1", ..., the difference is minimal.
}
```

</details>

---

## Recap

A hash table is three things: an array of buckets, a hash function that maps keys to bucket indices, and a collision resolution strategy. We used chaining -- each bucket is a list of entries that hashed to the same index. With a good hash function and a load factor below 0.75, each bucket holds 0-2 entries, making every operation effectively O(1).

The cost of that speed is memory. We maintain an array of buckets, many of them empty. At load factor 0.75 with 100,000 entries, we have roughly 131,072 buckets -- 31,000 of them empty. That is the trade-off: space for time. For a database that needs to serve 10,000 lookups per second, it is a trade-off worth making every single time.
