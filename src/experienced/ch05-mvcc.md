# Chapter 5: MVCC — Multi-Version Concurrency Control

Your database can store typed values, serialize them to bytes, and persist them to disk. But it has a dirty secret: it assumes one user at a time. If two transactions run concurrently — one reading a bank balance while another transfers money — the reader might see a half-finished transfer. Account A debited, account B not yet credited. The money vanished into thin air. This is the consistency problem, and every real database must solve it.

This chapter builds a Multi-Version Concurrency Control (MVCC) layer. Instead of locking data so only one transaction can access it at a time, MVCC keeps multiple versions of each value. Readers see a consistent snapshot frozen at the moment their transaction began. Writers create new versions without disturbing readers. No one waits. No one blocks. Everyone sees a consistent world.

By the end of this chapter, you will have:

- A versioned key-value store where each write creates a new `(key, version)` entry
- A `Transaction` struct with `begin()`, `get()`, `set()`, and `commit()`
- Snapshot isolation: each transaction reads only versions that were committed before it started
- Tests proving that concurrent readers see consistent snapshots even during writes
- A clear understanding of Rust lifetimes, references, and borrowing rules

---

## Spotlight: Lifetimes & References

Every chapter has one spotlight concept. This chapter's spotlight is **lifetimes and references** — Rust's mechanism for ensuring that borrowed data is always valid, and the foundation of its memory safety guarantees.

### The problem lifetimes solve

In C, you can return a pointer to a local variable:

```c
int* get_value() {
    int x = 42;
    return &x;  // x is destroyed when the function returns
}
// The caller now has a pointer to garbage — a dangling pointer
```

This compiles, runs, and silently corrupts memory. In Java or Python, the garbage collector prevents this by keeping objects alive as long as any reference exists. Rust takes a third path: the compiler tracks how long every reference is valid and refuses to compile code that would create a dangling reference.

### References: borrowing without owning

A reference lets you access data without taking ownership. There are two kinds:

```rust
let name = String::from("toydb");

let r1 = &name;        // shared reference: can read, cannot modify
let r2 = &name;        // multiple shared references are OK
println!("{} {}", r1, r2);

let r3 = &mut name;    // ERROR: cannot borrow as mutable — r1 and r2 are still in scope
```

The borrowing rules are simple but strict:

1. You can have **any number of shared references** (`&T`), OR
2. You can have **exactly one mutable reference** (`&mut T`)
3. But **never both at the same time**

This is not a limitation — it is a feature. It prevents data races at compile time. If you have a `&mut` reference, no one else can read or write the data. If you have a `&` reference, the data cannot change underneath you.

### What is a lifetime?

A lifetime is the span of code during which a reference is valid. Most of the time, the compiler infers lifetimes automatically. But sometimes you need to tell the compiler: "these two references live for the same duration" or "this returned reference lives as long as this input."

```rust
// The compiler infers: r lives as long as name
let name = String::from("toydb");
let r = &name;  // r's lifetime starts here
println!("{}", r);  // r's lifetime ends here (last use)
```

When a function returns a reference, the compiler needs to know which input the output borrows from:

```rust
// This does NOT compile — the compiler cannot infer the lifetime
fn longest(a: &str, b: &str) -> &str {
    if a.len() > b.len() { a } else { b }
}
```

The return value borrows from either `a` or `b`, but the compiler does not know which. You must annotate:

```rust
// 'a means: the returned reference lives at least as long as both inputs
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}
```

The `'a` is a lifetime parameter. It says: "the returned `&str` is valid for as long as both input `&str`s are valid." If the caller drops one of the inputs, the returned reference becomes invalid — and the compiler will catch it.

### Lifetime annotations on structs

When a struct holds a reference, it needs a lifetime annotation:

```rust
struct Transaction<'a> {
    store: &'a Store,      // borrows from a Store
    version: u64,
}
```

This says: a `Transaction` cannot outlive the `Store` it borrows from. If the `Store` is dropped, every `Transaction` referencing it becomes invalid. The compiler enforces this — you cannot use a `Transaction` after its `Store` is gone.

### When lifetimes get in the way

In practice, lifetimes are most annoying when you want a struct to hold a reference to something. The compiler forces you to thread lifetime parameters through every struct and function that touches the reference. Sometimes the cleanest solution is to avoid references entirely:

```rust
// Instead of borrowing:
struct Transaction<'a> {
    store: &'a Store,     // must track lifetime
}

// Own the data:
struct Transaction {
    store: Store,         // no lifetime needed
}

// Or use shared ownership:
use std::sync::Arc;
struct Transaction {
    store: Arc<Store>,    // reference-counted, no lifetime needed
}
```

For our MVCC implementation, we will own the data in the `Transaction` struct and use interior mutability. This avoids lifetime gymnastics while still being safe.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Memory management | Garbage collector | Garbage collector | Garbage collector | Ownership + borrowing |
> | Dangling references | Impossible (GC) | Impossible (GC) | Impossible (GC) | Compile-time error |
> | Data races | Possible (shared state) | GIL prevents (mostly) | Possible (goroutines) | Compile-time error |
> | Shared access | Always allowed | Always allowed | Always allowed | `&T` — read only |
> | Mutable access | Always allowed | Always allowed | Always allowed (mutex optional) | `&mut T` — exclusive |
> | Lifetime annotations | N/A | N/A | N/A | `'a` on references |
>
> The key difference: in GC'd languages, the runtime figures out when to free memory. In Rust, the compiler figures it out at compile time. The cost is that you sometimes need to annotate lifetimes. The benefit is zero runtime overhead, no GC pauses, and data races are impossible.

---

## Exercise 1: See the Problem — Inconsistent Reads

**Goal:** Before building the solution, understand the problem. Simulate two transactions without isolation and observe the anomaly.

### Step 1: A naive concurrent scenario

Create `src/mvcc.rs` and start with a simple versioned store:

```rust
use std::collections::BTreeMap;
use crate::value::Value;

/// A key-value store with no concurrency control.
/// This is intentionally broken — Exercise 2 fixes it.
pub struct NaiveStore {
    data: BTreeMap<String, Value>,
}

impl NaiveStore {
    pub fn new() -> Self {
        NaiveStore {
            data: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }
}
```

### Step 2: Demonstrate the anomaly

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demonstrate_dirty_read_problem() {
        let mut store = NaiveStore::new();

        // Setup: Alice has $1000, Bob has $500
        store.set("alice_balance", Value::Integer(1000));
        store.set("bob_balance", Value::Integer(500));

        // Transaction A: Transfer $200 from Alice to Bob
        // Step 1: Debit Alice
        let alice = store.get("alice_balance").unwrap().as_integer().unwrap();
        store.set("alice_balance", Value::Integer(alice - 200));

        // *** PROBLEM: If Transaction B reads here, it sees: ***
        // Alice: $800 (debited)
        // Bob: $500 (not yet credited)
        // Total: $1300 — $200 vanished!
        let mid_alice = store.get("alice_balance").unwrap().as_integer().unwrap();
        let mid_bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        assert_eq!(mid_alice + mid_bob, 1300); // NOT 1500 — money is missing!

        // Step 2: Credit Bob
        let bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        store.set("bob_balance", Value::Integer(bob + 200));

        // After both steps: Alice $800, Bob $700, total $1500 — correct
        let final_alice = store.get("alice_balance").unwrap().as_integer().unwrap();
        let final_bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        assert_eq!(final_alice + final_bob, 1500);
    }
}
```

This test passes, but it proves the problem: between step 1 and step 2, any observer sees an inconsistent state. In a real concurrent system, another thread could read at that exact moment and make decisions based on wrong data.

The fix is not "read faster" or "write atomically" — it is giving each reader a consistent snapshot that does not change while they are looking at it.

<details>
<summary>Hint: Why not just use a mutex?</summary>

A mutex (mutual exclusion lock) would work: lock the entire store for the duration of a transaction, so no one else can read or write. But this serializes all access. If a long-running report reads millions of rows, every other user waits. MVCC avoids this by letting readers and writers proceed simultaneously — readers see old versions, writers create new versions. No one waits.

</details>

---

## Exercise 2: Implement Versioned Keys

**Goal:** Replace the flat `BTreeMap<String, Value>` with a versioned store where each key has multiple versions. Each write creates a new `(key, version)` entry instead of overwriting the old one.

### Step 1: Define the versioned key

```rust
/// A versioned key: (key, version). The version is a monotonically
/// increasing number assigned by the MVCC layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VersionedKey {
    key: String,
    version: u64,
}

impl VersionedKey {
    fn new(key: &str, version: u64) -> Self {
        VersionedKey {
            key: key.to_string(),
            version,
        }
    }
}
```

The `Ord` derive is critical. It orders `VersionedKey` by `(key, version)` — first alphabetically by key, then numerically by version. This means a `BTreeMap<VersionedKey, Value>` naturally groups all versions of the same key together, ordered by version.

### Step 2: Build the versioned store

```rust
/// The MVCC store. Holds all versions of all keys.
pub struct MvccStore {
    /// All versioned data: (key, version) -> value
    data: BTreeMap<VersionedKey, Option<Value>>,
    /// The next version number to assign
    next_version: u64,
}

impl MvccStore {
    pub fn new() -> Self {
        MvccStore {
            data: BTreeMap::new(),
            next_version: 1,
        }
    }

    /// Allocate a new version number.
    fn next_version(&mut self) -> u64 {
        let v = self.next_version;
        self.next_version += 1;
        v
    }

    /// Write a value at a specific version.
    fn write(&mut self, key: &str, version: u64, value: Value) {
        self.data.insert(
            VersionedKey::new(key, version),
            Some(value),
        );
    }

    /// Delete a key at a specific version (write a tombstone).
    fn delete(&mut self, key: &str, version: u64) {
        self.data.insert(
            VersionedKey::new(key, version),
            None, // tombstone — the key is deleted at this version
        );
    }

    /// Read the latest version of a key that is <= the given version.
    /// Returns None if the key does not exist or was deleted.
    fn read(&self, key: &str, at_version: u64) -> Option<&Value> {
        // Scan all versions of this key in reverse order,
        // find the first one with version <= at_version
        self.data
            .iter()
            .rev()
            .filter(|(vk, _)| vk.key == key && vk.version <= at_version)
            .next()
            .and_then(|(_, val)| val.as_ref())
    }
}
```

### Step 3: Test versioned reads

```rust
#[cfg(test)]
mod versioned_tests {
    use super::*;

    #[test]
    fn write_and_read_at_version() {
        let mut store = MvccStore::new();

        // Version 1: Alice = 1000
        store.write("alice", 1, Value::Integer(1000));

        // Version 2: Alice = 800
        store.write("alice", 2, Value::Integer(800));

        // Version 3: Alice = 900
        store.write("alice", 3, Value::Integer(900));

        // Reading at different versions sees different values
        assert_eq!(
            store.read("alice", 1).unwrap().as_integer().unwrap(),
            1000
        );
        assert_eq!(
            store.read("alice", 2).unwrap().as_integer().unwrap(),
            800
        );
        assert_eq!(
            store.read("alice", 3).unwrap().as_integer().unwrap(),
            900
        );
        // Reading at version 5 still sees the latest (version 3)
        assert_eq!(
            store.read("alice", 5).unwrap().as_integer().unwrap(),
            900
        );
    }

    #[test]
    fn read_nonexistent_key_returns_none() {
        let store = MvccStore::new();
        assert!(store.read("alice", 1).is_none());
    }

    #[test]
    fn delete_makes_key_invisible() {
        let mut store = MvccStore::new();

        store.write("alice", 1, Value::Integer(1000));
        store.delete("alice", 2); // tombstone at version 2

        assert_eq!(
            store.read("alice", 1).unwrap().as_integer().unwrap(),
            1000
        );
        assert!(store.read("alice", 2).is_none()); // deleted
        assert!(store.read("alice", 3).is_none()); // still deleted
    }

    #[test]
    fn delete_then_rewrite() {
        let mut store = MvccStore::new();

        store.write("alice", 1, Value::Integer(1000));
        store.delete("alice", 2);
        store.write("alice", 3, Value::Integer(2000));

        assert!(store.read("alice", 2).is_none());
        assert_eq!(
            store.read("alice", 3).unwrap().as_integer().unwrap(),
            2000
        );
    }
}
```

```
$ cargo test versioned_tests
running 4 tests
test mvcc::versioned_tests::write_and_read_at_version ... ok
test mvcc::versioned_tests::read_nonexistent_key_returns_none ... ok
test mvcc::versioned_tests::delete_makes_key_invisible ... ok
test mvcc::versioned_tests::delete_then_rewrite ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why use BTreeMap instead of HashMap?</summary>

`BTreeMap` keeps keys in sorted order. Since `VersionedKey` sorts by `(key, version)`, all versions of the same key are adjacent and ordered. This makes range scans efficient — to find "all versions of key X with version <= N", we scan backwards from `(X, N)`. A `HashMap` would require checking every entry, which is O(total_entries) instead of O(versions_of_X).

In the real toydb, the underlying storage engine (BitCask or LSM) provides ordered iteration, so the MVCC layer naturally benefits from sorted access.

</details>

---

## Exercise 3: Implement the Transaction Struct

**Goal:** Build a `Transaction` struct that encapsulates a consistent snapshot. It sees only versions committed before it began, buffers its own writes, and applies them atomically on commit.

### Step 1: Define Transaction

```rust
use std::collections::HashMap;

/// The state of a transaction.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TransactionState {
    Active,
    Committed,
    Aborted,
}

/// A transaction with snapshot isolation.
///
/// On begin(), it records the current version as its snapshot version.
/// All reads see only versions <= snapshot_version.
/// All writes are buffered locally until commit().
pub struct Transaction {
    /// The version this transaction reads at (snapshot)
    snapshot_version: u64,
    /// The version this transaction's writes will be stored at
    write_version: u64,
    /// Buffered writes: key -> Some(value) for set, key -> None for delete
    writes: HashMap<String, Option<Value>>,
    /// Current state
    state: TransactionState,
}

impl Transaction {
    /// Create a new transaction against the store.
    /// The snapshot_version is the latest committed version at the time
    /// of begin(). The write_version is the next available version.
    fn new(snapshot_version: u64, write_version: u64) -> Self {
        Transaction {
            snapshot_version,
            write_version,
            writes: HashMap::new(),
            state: TransactionState::Active,
        }
    }
}
```

### Step 2: Implement transaction operations

```rust
impl Transaction {
    /// Read a key. First checks local writes, then falls back to the store.
    pub fn get(&self, store: &MvccStore, key: &str) -> Result<Option<Value>, String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }

        // Check local writes first — we should see our own uncommitted changes
        if let Some(local_val) = self.writes.get(key) {
            return Ok(local_val.clone());
        }

        // Read from the store at our snapshot version
        Ok(store.read(key, self.snapshot_version).cloned())
    }

    /// Buffer a write. The value is not visible to other transactions
    /// until commit().
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }
        self.writes.insert(key.to_string(), Some(value));
        Ok(())
    }

    /// Buffer a delete.
    pub fn delete(&mut self, key: &str) -> Result<(), String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }
        self.writes.insert(key.to_string(), None);
        Ok(())
    }

    /// Commit: apply all buffered writes to the store at this
    /// transaction's write_version.
    pub fn commit(mut self, store: &mut MvccStore) -> Result<(), String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }

        for (key, value) in self.writes.drain() {
            match value {
                Some(val) => store.write(&key, self.write_version, val),
                None => store.delete(&key, self.write_version),
            }
        }

        self.state = TransactionState::Committed;
        Ok(())
    }

    /// Abort: discard all buffered writes.
    pub fn abort(mut self) -> Result<(), String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }
        self.writes.clear();
        self.state = TransactionState::Aborted;
        Ok(())
    }
}
```

### Step 3: Add a begin() method to MvccStore

```rust
impl MvccStore {
    /// Begin a new transaction. The transaction sees a snapshot at the
    /// current version and will write at the next version.
    pub fn begin(&mut self) -> Transaction {
        let snapshot = self.next_version - 1;
        let write_ver = self.next_version();
        Transaction::new(snapshot, write_ver)
    }
}
```

### Step 4: Test transaction basics

```rust
#[cfg(test)]
mod transaction_tests {
    use super::*;

    #[test]
    fn basic_transaction_flow() {
        let mut store = MvccStore::new();

        // Write initial data outside a transaction (version 1)
        store.write("name", 1, Value::String("Alice".to_string()));
        store.next_version = 2; // advance past the manual write

        // Begin a transaction — it sees version 1
        let mut txn = store.begin();
        assert_eq!(
            txn.get(&store, "name").unwrap().unwrap().as_str().unwrap(),
            "Alice"
        );

        // Write within the transaction (buffered, not yet visible)
        txn.set("name", Value::String("Bob".to_string())).unwrap();

        // The transaction sees its own write
        assert_eq!(
            txn.get(&store, "name").unwrap().unwrap().as_str().unwrap(),
            "Bob"
        );

        // Commit applies the write to the store
        txn.commit(&mut store).unwrap();

        // A new transaction sees the committed value
        let txn2 = store.begin();
        assert_eq!(
            txn2.get(&store, "name").unwrap().unwrap().as_str().unwrap(),
            "Bob"
        );
    }

    #[test]
    fn abort_discards_writes() {
        let mut store = MvccStore::new();
        store.write("name", 1, Value::String("Alice".to_string()));
        store.next_version = 2;

        let mut txn = store.begin();
        txn.set("name", Value::String("Bob".to_string())).unwrap();
        txn.abort().unwrap();

        // A new transaction still sees the original value
        let txn2 = store.begin();
        assert_eq!(
            txn2.get(&store, "name").unwrap().unwrap().as_str().unwrap(),
            "Alice"
        );
    }

    #[test]
    fn transaction_delete() {
        let mut store = MvccStore::new();
        store.write("name", 1, Value::String("Alice".to_string()));
        store.next_version = 2;

        let mut txn = store.begin();
        txn.delete("name").unwrap();

        // Transaction sees the key as deleted
        assert!(txn.get(&store, "name").unwrap().is_none());

        txn.commit(&mut store).unwrap();

        // New transaction also sees it as deleted
        let txn2 = store.begin();
        assert!(txn2.get(&store, "name").unwrap().is_none());
    }

    #[test]
    fn operations_on_committed_txn_fail() {
        let mut store = MvccStore::new();
        let mut txn = store.begin();
        txn.set("x", Value::Integer(1)).unwrap();
        txn.commit(&mut store).unwrap();

        // The transaction consumed itself on commit (moved self),
        // so we cannot call methods on it. This is enforced by the
        // type system — commit() takes `self`, not `&mut self`.
    }
}
```

```
$ cargo test transaction_tests
running 4 tests
test mvcc::transaction_tests::basic_transaction_flow ... ok
test mvcc::transaction_tests::abort_discards_writes ... ok
test mvcc::transaction_tests::transaction_delete ... ok
test mvcc::transaction_tests::operations_on_committed_txn_fail ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

Notice that `commit()` and `abort()` take `self` — they consume the transaction. After calling either, the transaction variable is gone. You cannot accidentally write to a committed transaction because the compiler will not let you use a moved value. This is the ownership system enforcing correctness.

<details>
<summary>Hint: Why buffer writes instead of writing directly?</summary>

Buffering serves two purposes:

1. **Atomicity** — if the transaction aborts (due to a conflict, an error, or the user calling `abort()`), we simply discard the buffer. No cleanup needed. If we had written directly to the store, we would need to undo those writes.

2. **Isolation** — other transactions should not see our uncommitted writes. If we wrote directly, a concurrent reader scanning the store might see a half-finished transfer.

The real toydb uses a similar approach: writes are buffered in memory, and on commit they are written to the underlying storage engine in a single batch.

</details>

---

## Exercise 4: Test Snapshot Isolation

**Goal:** Prove that snapshot isolation works — two transactions started at different times see consistent, non-interfering views of the data. This is the whole point of MVCC.

### Step 1: The bank transfer test

```rust
#[cfg(test)]
mod isolation_tests {
    use super::*;

    #[test]
    fn snapshot_isolation_bank_transfer() {
        let mut store = MvccStore::new();

        // Setup: Alice=$1000, Bob=$500 (version 1)
        store.write("alice", 1, Value::Integer(1000));
        store.write("bob", 1, Value::Integer(500));
        store.next_version = 2;

        // Transaction A (reader): begins now, sees version 1
        let txn_reader = store.begin(); // snapshot=1, write_ver=2

        // Transaction B (writer): transfer $200 Alice -> Bob
        let mut txn_writer = store.begin(); // snapshot=1, write_ver=3

        // Writer reads current balances
        let alice_bal = txn_writer.get(&store, "alice").unwrap()
            .unwrap().as_integer().unwrap();
        let bob_bal = txn_writer.get(&store, "bob").unwrap()
            .unwrap().as_integer().unwrap();

        // Writer performs transfer
        txn_writer.set("alice", Value::Integer(alice_bal - 200)).unwrap();
        txn_writer.set("bob", Value::Integer(bob_bal + 200)).unwrap();
        txn_writer.commit(&mut store).unwrap();

        // CRITICAL: Reader still sees the PRE-TRANSFER balances!
        // Its snapshot was taken before the writer committed.
        let reader_alice = txn_reader.get(&store, "alice").unwrap()
            .unwrap().as_integer().unwrap();
        let reader_bob = txn_reader.get(&store, "bob").unwrap()
            .unwrap().as_integer().unwrap();

        assert_eq!(reader_alice, 1000, "Reader should see original Alice balance");
        assert_eq!(reader_bob, 500, "Reader should see original Bob balance");
        assert_eq!(reader_alice + reader_bob, 1500, "Reader sees consistent total");

        // A NEW transaction sees the post-transfer balances
        let txn_new = store.begin();
        let new_alice = txn_new.get(&store, "alice").unwrap()
            .unwrap().as_integer().unwrap();
        let new_bob = txn_new.get(&store, "bob").unwrap()
            .unwrap().as_integer().unwrap();

        assert_eq!(new_alice, 800, "New reader sees debited Alice");
        assert_eq!(new_bob, 700, "New reader sees credited Bob");
        assert_eq!(new_alice + new_bob, 1500, "New reader sees consistent total");
    }
}
```

This is the test that matters. The reader began before the writer committed, so it sees the old balances. The total is always $1500 — money never vanishes, no matter when you look. This is snapshot isolation: each transaction sees a frozen-in-time view of the database.

### Step 2: Test multiple concurrent writers

```rust
    #[test]
    fn multiple_writers_create_separate_versions() {
        let mut store = MvccStore::new();

        store.write("counter", 1, Value::Integer(0));
        store.next_version = 2;

        // Three transactions, each incrementing the counter
        // (In a real system, we'd need conflict detection — see below)
        let mut txn1 = store.begin();
        let mut txn2 = store.begin();
        let mut txn3 = store.begin();

        txn1.set("counter", Value::Integer(1)).unwrap();
        txn2.set("counter", Value::Integer(2)).unwrap();
        txn3.set("counter", Value::Integer(3)).unwrap();

        // Commit in order: txn1, txn2, txn3
        // Each writes at its own version
        txn1.commit(&mut store).unwrap();
        txn2.commit(&mut store).unwrap();
        txn3.commit(&mut store).unwrap();

        // The latest reader sees the last committed value
        let txn_final = store.begin();
        let val = txn_final.get(&store, "counter").unwrap()
            .unwrap().as_integer().unwrap();
        assert_eq!(val, 3); // txn3 committed last, its version is highest
    }
```

### Step 3: Test reading a key that does not exist

```rust
    #[test]
    fn read_missing_key_returns_none() {
        let mut store = MvccStore::new();
        let txn = store.begin();
        assert!(txn.get(&store, "nonexistent").unwrap().is_none());
    }
```

### Step 4: Test interleaved reads and writes

```rust
    #[test]
    fn interleaved_reads_and_writes() {
        let mut store = MvccStore::new();

        // Version 1: initial data
        store.write("x", 1, Value::Integer(10));
        store.write("y", 1, Value::Integer(20));
        store.next_version = 2;

        // Txn A reads x, then Txn B modifies x, then Txn A reads x again
        let txn_a = store.begin();

        let x1 = txn_a.get(&store, "x").unwrap().unwrap().as_integer().unwrap();
        assert_eq!(x1, 10);

        // Txn B modifies x and commits
        let mut txn_b = store.begin();
        txn_b.set("x", Value::Integer(99)).unwrap();
        txn_b.commit(&mut store).unwrap();

        // Txn A reads x again — still sees 10 (snapshot isolation!)
        let x2 = txn_a.get(&store, "x").unwrap().unwrap().as_integer().unwrap();
        assert_eq!(x2, 10);
        assert_eq!(x1, x2, "Same transaction, same snapshot, same value");
    }
```

```
$ cargo test isolation_tests
running 4 tests
test mvcc::isolation_tests::snapshot_isolation_bank_transfer ... ok
test mvcc::isolation_tests::multiple_writers_create_separate_versions ... ok
test mvcc::isolation_tests::read_missing_key_returns_none ... ok
test mvcc::isolation_tests::interleaved_reads_and_writes ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

Every test proves the same principle: a transaction's snapshot is immutable. No matter what other transactions do after it begins, it always sees the same data. This is the guarantee that makes databases reliable.

<details>
<summary>Hint: What about write-write conflicts?</summary>

Our MVCC implementation has a gap: if two transactions both modify the same key and both commit, the last one wins silently. This is called "lost update" — Transaction A's write is overwritten by Transaction B without A knowing.

The real toydb handles this with write conflict detection: on commit, it checks whether any key this transaction wrote was also written by a transaction that committed after our snapshot. If so, the commit is rejected with a serialization error.

We will add conflict detection as an extension exercise. For now, the snapshot isolation for reads is the important concept.

</details>

---

## Rust Gym

### Drill 1: Lifetime Annotations on Struct Fields

This code does not compile. Add the correct lifetime annotations to fix it:

```rust
struct Config {
    name: &str,
    version: &str,
}

fn make_config(name: &str, version: &str) -> Config {
    Config { name, version }
}

fn main() {
    let config = make_config("toydb", "0.1.0");
    println!("{} v{}", config.name, config.version);
}
```

<details>
<summary>Solution</summary>

```rust
struct Config<'a> {
    name: &'a str,
    version: &'a str,
}

fn make_config<'a>(name: &'a str, version: &'a str) -> Config<'a> {
    Config { name, version }
}

fn main() {
    let config = make_config("toydb", "0.1.0");
    println!("{} v{}", config.name, config.version);
}
```

The struct holds references, so it needs a lifetime parameter `'a`. The function signature says: "the returned `Config` lives as long as both inputs." Since we pass string literals (`&'static str`), the config is valid for the entire program.

If instead you had:

```rust
let name = String::from("toydb");
let config = make_config(&name, "0.1.0");
drop(name); // ERROR: cannot drop name while config borrows it
```

The compiler would catch this — `config` borrows `name`, so `name` cannot be dropped first.

</details>

### Drill 2: Return a Reference With Proper Lifetime

This function should return the longer of two string slices. Fix the lifetime annotations:

```rust
fn longest(a: &str, b: &str) -> &str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let result;
    {
        let a = String::from("hello");
        let b = String::from("hi");
        result = longest(&a, &b);
    }
    // Can we use result here?
    // println!("{}", result);
}
```

<details>
<summary>Solution</summary>

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let result;
    {
        let a = String::from("hello");
        let b = String::from("hi");
        result = longest(&a, &b);
        // result is valid here — a and b are still alive
        println!("{}", result); // prints "hello"
    }
    // result is NOT valid here — a and b were dropped
    // println!("{}", result);  // COMPILE ERROR: borrowed value does not live long enough
}
```

The lifetime `'a` constrains `result` to live no longer than the shortest-lived input. Since `a` and `b` are dropped at the end of the inner block, `result` cannot be used after that block.

To fix this, either use `result` inside the block, or move `a` and `b` to the same scope as `result`.

</details>

### Drill 3: Iterator That Borrows From a Collection

Implement an iterator that yields references to values in a `Vec<i32>` that are above a threshold:

```rust
struct AboveThreshold<'a> {
    data: &'a [i32],
    threshold: i32,
    index: usize,
}

// Implement Iterator for AboveThreshold
// It should yield &'a i32 references to elements above the threshold
```

<details>
<summary>Solution</summary>

```rust
struct AboveThreshold<'a> {
    data: &'a [i32],
    threshold: i32,
    index: usize,
}

impl<'a> AboveThreshold<'a> {
    fn new(data: &'a [i32], threshold: i32) -> Self {
        AboveThreshold {
            data,
            threshold,
            index: 0,
        }
    }
}

impl<'a> Iterator for AboveThreshold<'a> {
    type Item = &'a i32;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.data.len() {
            let val = &self.data[self.index];
            self.index += 1;
            if *val > self.threshold {
                return Some(val);
            }
        }
        None
    }
}

#[test]
fn above_threshold_iterator() {
    let data = vec![1, 5, 3, 8, 2, 9, 4];
    let above_five: Vec<&i32> = AboveThreshold::new(&data, 5).collect();
    assert_eq!(above_five, vec![&8, &9]);
}
```

The lifetime `'a` threads from the `data` slice through the `Iterator::Item` type. This tells the compiler: "the references this iterator yields are valid as long as the original slice is valid." If you dropped `data` while iterating, the compiler would catch the dangling reference.

In practice, you would use `data.iter().filter(|&&x| x > 5)` instead of a custom iterator. But understanding how to build one teaches you what `.filter()` does internally.

</details>

---

## DSA in Context: Version Chains

MVCC is a data structures problem. Each key has a chain of versions — a linked list of `(version, value)` pairs. The fundamental operations are:

### Version chain as a sorted list

```
Key "alice":
  v1 -> 1000
  v3 -> 800     (after transfer)
  v5 -> 900     (after deposit)

Key "bob":
  v1 -> 500
  v3 -> 700     (after transfer)
```

To read "alice" at version 4, we find the latest version <= 4, which is v3 (value 800). This is a binary search on the version chain — O(log V) where V is the number of versions for that key.

### Visibility rules

A version `(key, v)` is visible to transaction T if:

1. `v <= T.snapshot_version` — the version was created before T's snapshot
2. The transaction that created version `v` has committed (not still active, not aborted)
3. There is no newer version `v'` where `v < v' <= T.snapshot_version` (we want the latest visible version)

Our simplified implementation only checks rule 1 and 3 (we do not track transaction commit status). The real toydb maintains an "active transactions" set to implement rule 2.

### Garbage collection

Without cleanup, version chains grow forever. A version is safe to garbage-collect when no active transaction can ever read it — specifically, when its version is older than the oldest active transaction's snapshot.

```
Active transactions: T5 (snapshot=4), T7 (snapshot=6)
Oldest snapshot: 4

Key "alice" versions: v1, v3, v5, v8
  - v1: can be removed (v3 supersedes it, and oldest snapshot is 4, so no one reads v1)
  - v3: KEEP (T5 reads this — it is the latest version <= 4)
  - v5: KEEP (T7 reads this)
  - v8: KEEP (future transactions read this)
```

This is the "MVCC vacuum" — PostgreSQL's `VACUUM` command does exactly this. It scans for dead versions and reclaims their space.

### Time complexity summary

| Operation | Naive (our impl) | Optimized (real DB) |
|-----------|-----------------|---------------------|
| Read | O(N) scan all versions | O(log V) binary search on version chain |
| Write | O(1) insert | O(1) insert + O(1) conflict check |
| Commit | O(W) apply W writes | O(W) apply + O(W) conflict detection |
| Garbage collection | O(N) scan all entries | Background incremental |

Our `BTreeMap` implementation is O(N) for reads because we scan the entire map. A production MVCC engine would use ordered iteration starting from `(key, snapshot_version)` and scanning backward to find the first match — O(log N + V) where N is total entries and V is versions of that key.

---

## System Design Corner: Transaction Isolation Levels

In a system design interview, you should know the four standard isolation levels and which anomalies each prevents.

### The anomaly hierarchy

| Anomaly | Description | Example |
|---------|-------------|---------|
| **Dirty read** | Reading uncommitted data | T1 writes x=5 (not committed), T2 reads x=5 |
| **Non-repeatable read** | Same query, different results | T1 reads x=10, T2 commits x=20, T1 reads x=20 |
| **Phantom read** | New rows appear between queries | T1 counts 5 users, T2 inserts a user, T1 counts 6 |
| **Lost update** | Two transactions overwrite each other | T1 reads x=10, T2 reads x=10, T1 writes x=11, T2 writes x=11 (should be 12) |

### The four isolation levels

| Level | Dirty Read | Non-repeatable Read | Phantom Read | Lost Update |
|-------|-----------|-------------------|-------------|-------------|
| Read Uncommitted | Possible | Possible | Possible | Possible |
| Read Committed | Prevented | Possible | Possible | Possible |
| Repeatable Read | Prevented | Prevented | Possible | Depends |
| Serializable | Prevented | Prevented | Prevented | Prevented |

### MVCC in real databases

**PostgreSQL** uses MVCC for all isolation levels. Its "Repeatable Read" is actually snapshot isolation (which is between Repeatable Read and Serializable in the hierarchy). Versions are stored in the same table as the data — old versions are called "dead tuples" and cleaned up by VACUUM.

**MySQL (InnoDB)** uses MVCC for reads and locks for writes. At Repeatable Read, it takes "gap locks" to prevent phantoms. This is stricter than PostgreSQL's Repeatable Read but has more lock contention.

**Our toydb** implements snapshot isolation — equivalent to PostgreSQL's Repeatable Read. Reads always see a consistent snapshot. Write conflicts are detected at commit time (in the full implementation).

> **Interview talking point:** *"Our database uses MVCC with snapshot isolation. Each transaction gets a consistent snapshot at begin time and sees no changes from concurrent transactions. Writers do not block readers, and readers do not block writers. Write-write conflicts are detected at commit time — if two transactions modify the same key, the second to commit is aborted and retried. This gives us serializable-equivalent behavior for most workloads without the overhead of full serializable isolation."*

---

## Design Insight: Define Errors Out of Existence

In *A Philosophy of Software Design*, Ousterhout argues that the best error-handling strategy is designing your system so errors cannot occur. MVCC is a perfect example.

Consider the alternative: lock-based concurrency control. Every read locks the data, every write locks the data, and you need to handle:

```
Error: lock timeout after 30s
Error: deadlock detected — aborting transaction A
Error: lock escalation from row to table (unexpected)
Error: lock held by crashed process — orphaned lock
```

Each of these is a runtime error that the application must handle. Deadlocks require retry logic. Timeouts require configuration tuning. Orphaned locks require a cleanup process. The error surface is enormous.

MVCC eliminates most of these errors by design:

- **No lock timeouts** — readers never lock, so they never wait
- **No deadlocks** — there are no locks to deadlock on
- **No orphaned locks** — there are no locks
- **No lock escalation** — there are no locks

The only remaining "error" is the write-write conflict, which is a clean, well-defined condition: "two transactions tried to modify the same key." The fix is equally clean: abort one and retry. One error case instead of five.

This is the power of defining errors out of existence. By choosing MVCC over locks, we did not just pick a "better" concurrency strategy — we removed entire categories of errors from the system. The code is simpler, the error handling is simpler, and the system is more reliable.

The lesson applies broadly: before writing error handling, ask whether you can redesign the API or the data model so the error is impossible. Type systems help (Rust's `Option` prevents null pointer errors), but design choices help more (MVCC prevents deadlock errors).

---

## What You Built

In this chapter, you:

1. **Understood the problem** — demonstrated that a naive store shows inconsistent intermediate states during multi-step operations
2. **Built versioned keys** — `(key, version)` entries in a `BTreeMap` that preserve the full history of every key
3. **Implemented transactions** — `begin()`, `get()`, `set()`, `delete()`, `commit()`, `abort()` with buffered writes and snapshot reads
4. **Proved snapshot isolation** — tests showing that a reader's view is frozen at begin time, unaffected by concurrent commits

Your database now supports concurrent readers with consistent snapshots. Writers buffer their changes and apply them atomically on commit. This is the same mechanism that PostgreSQL, MySQL, and CockroachDB use to serve thousands of concurrent connections.

But users do not want to call `set("name", Value::String("Alice"))`. They want to write `INSERT INTO users (name) VALUES ('Alice')`. Chapter 6 begins the SQL journey with a lexer that breaks SQL strings into tokens.

---

### DS Deep Dive

MVCC snapshot isolation prevents most anomalies but not all. This deep dive explores write skew (the anomaly that snapshot isolation misses), serializable snapshot isolation (SSI), and how PostgreSQL and CockroachDB achieve full serializability without locks.

**-> [MVCC Anomalies & Serializable Snapshot Isolation -- "The Time Travel Paradox"](../ds-narratives/ch05-mvcc-anomalies.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/mvcc.rs` — `MvccStore` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `MVCC` struct |
| `src/mvcc.rs` — `Transaction` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `Transaction` struct |
| `VersionedKey` | [`src/storage/mvcc.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) — `Key` enum with version encoding |
| Snapshot isolation tests | [`src/storage/mvcc.rs` tests](https://github.com/erikgrinaker/toydb/blob/master/src/storage/mvcc.rs) |
