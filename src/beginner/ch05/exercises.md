## Exercise 1: See the Problem — Inconsistent Reads

**Goal:** Before building the solution, understand the problem. Simulate two transactions without isolation and observe the anomaly.

### Step 1: Create a naive store

Create `src/mvcc.rs` and start with a simple store that has no concurrency control:

```rust,ignore
use std::collections::BTreeMap;
use crate::value::Value;

/// A key-value store with no concurrency control.
/// This is intentionally broken -- Exercise 2 fixes it.
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

This is a simple wrapper around a `BTreeMap`. Nothing fancy -- just set and get.

### Step 2: Demonstrate the anomaly

```rust,ignore
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
        // Total: $1300 -- $200 vanished!
        let mid_alice = store.get("alice_balance").unwrap().as_integer().unwrap();
        let mid_bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        assert_eq!(mid_alice + mid_bob, 1300); // NOT 1500!

        // Step 2: Credit Bob
        let bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        store.set("bob_balance", Value::Integer(bob + 200));

        // After both steps: Alice $800, Bob $700, total $1500 -- correct
        let final_alice = store.get("alice_balance").unwrap().as_integer().unwrap();
        let final_bob = store.get("bob_balance").unwrap().as_integer().unwrap();
        assert_eq!(final_alice + final_bob, 1500);
    }
}
```

> **What just happened?**
>
> We proved that a simple store shows inconsistent intermediate states. Between debiting Alice and crediting Bob, any observer sees $200 missing. In a real concurrent system, another thread could read at that exact moment and make decisions based on wrong data -- an investment app might think the account is low and reject a purchase, or a fraud detector might flag the missing money.
>
> The fix is not "read faster" or "write atomically" -- it is giving each reader a consistent snapshot that does not change while they are looking at it.

> **Analogy: Two people editing the same Google Doc**
>
> Imagine Alice and Bob are both editing the same Google Doc. Alice is moving a paragraph from page 2 to page 3. At the exact moment when the paragraph has been deleted from page 2 but not yet pasted on page 3, Bob takes a screenshot. His screenshot shows a document with a missing paragraph. The paragraph is not lost -- Alice is still holding it -- but Bob sees an inconsistent view.
>
> MVCC solves this by giving Bob a "frozen" copy of the document as it looked before Alice started editing. Alice's changes are invisible to Bob until she clicks "save" (commits).

### Why not just use a lock?

A lock (mutex) would work: lock the entire store for the duration of a transaction, so no one else can read or write. But this serializes all access. If a long-running report reads millions of rows, every other user waits. MVCC avoids this by letting readers and writers proceed simultaneously -- readers see old versions, writers create new versions. No one waits.

---

## Exercise 2: Implement Versioned Keys

**Goal:** Replace the flat `BTreeMap<String, Value>` with a versioned store where each key has multiple versions. Each write creates a new `(key, version)` entry instead of overwriting the old one.

### Step 1: Define the versioned key

```rust,ignore
/// A versioned key: (key, version). The version is a monotonically
/// increasing number assigned by the MVCC layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VersionedKey {
    key: String,
    version: u64,
}
```

Let's understand the four comparison derives:

- **`PartialEq`** -- enables `==` and `!=` comparisons
- **`Eq`** -- a marker trait that says "equality is total" (every value equals itself). `f64` implements `PartialEq` but not `Eq` because `NaN != NaN`.
- **`PartialOrd`** -- enables `<`, `>`, `<=`, `>=` comparisons
- **`Ord`** -- enables total ordering, which `BTreeMap` requires for its keys

When you derive `Ord` on a struct, Rust compares fields in the order they are declared. So `VersionedKey` sorts first by `key` (alphabetically), then by `version` (numerically). This means a `BTreeMap<VersionedKey, ...>` naturally groups all versions of the same key together, ordered by version.

```rust,ignore
impl VersionedKey {
    fn new(key: &str, version: u64) -> Self {
        VersionedKey {
            key: key.to_string(),
            version,
        }
    }
}
```

> **What just happened?**
>
> Instead of a flat `BTreeMap<String, Value>` where each key has one value, we now have `BTreeMap<VersionedKey, Value>` where each key can have multiple values at different versions. The entry `("alice", version=1)` and `("alice", version=3)` are separate entries in the map. Old versions are preserved, not overwritten.

### Step 2: Build the versioned store

```rust,ignore
/// The MVCC store. Holds all versions of all keys.
pub struct MvccStore {
    /// All versioned data: (key, version) -> value
    data: BTreeMap<VersionedKey, Option<Value>>,
    /// The next version number to assign
    next_version: u64,
}
```

Notice the value type is `Option<Value>`, not `Value`. A `None` value is a **tombstone** -- it marks that the key was deleted at that version. This is how MVCC handles deletes without actually removing old versions.

```rust,ignore
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
            None,  // tombstone -- the key is deleted at this version
        );
    }
}
```

**Why `next_version` returns the current value then increments:** This is a common pattern called "post-increment." The first call returns 1, the second returns 2, etc. Each version number is unique and always increasing.

### Step 3: Implement versioned reads

This is the key method -- reading a value at a specific version:

```rust,ignore
impl MvccStore {
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

Let's break this down piece by piece:

**`self.data.iter()`** -- Creates an iterator over all entries in the BTreeMap, in sorted order.

**`.rev()`** -- Reverses the iterator so we scan from highest version to lowest. We want the *latest* version that is not newer than our target.

**`.filter(|(vk, _)| vk.key == key && vk.version <= at_version)`** -- Keeps only entries that match our key AND have a version number at or before our target version. The `|(vk, _)|` destructures each entry into the key and value (we ignore the value with `_` in the filter).

**`.next()`** -- Takes the first matching entry (which is the latest version, since we reversed).

**`.and_then(|(_, val)| val.as_ref())`** -- If we found an entry, extract the value. `val` is `&Option<Value>`. `.as_ref()` converts `&Option<Value>` to `Option<&Value>`. If the value is `None` (a tombstone), the result is `None` -- the key was deleted.

> **What just happened?**
>
> The `read` method implements the core MVCC concept: time travel. By specifying `at_version`, you can read the state of any key at any point in history. Version 1 might say Alice has $1000, version 3 might say $800. A transaction with snapshot at version 1 always sees $1000, no matter what happens at later versions.

### Step 4: Test versioned reads

```rust,ignore
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

> **What just happened?**
>
> We proved that version chains work. The same key ("alice") has different values at different versions. Reading at version 1 always returns 1000, regardless of what was written at later versions. Deletes insert tombstones (version 2 = None), and new writes after deletes create new versions (version 3 = 2000).

### Why BTreeMap instead of HashMap?

`BTreeMap` keeps keys in sorted order. Since `VersionedKey` sorts by `(key, version)`, all versions of the same key are adjacent and ordered. This makes range scans efficient -- to find "all versions of key X with version <= N", we could scan backwards from `(X, N)`. A `HashMap` would require checking every entry.

---

## Exercise 3: Implement the Transaction Struct

**Goal:** Build a `Transaction` struct that encapsulates a consistent snapshot. It sees only versions committed before it began, buffers its own writes, and applies them atomically on commit.

### Step 1: Define the transaction state

```rust,ignore
use std::collections::HashMap;

/// The state of a transaction.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TransactionState {
    Active,
    Committed,
    Aborted,
}
```

A transaction is always in one of three states:
- **Active** -- currently running, can read and write
- **Committed** -- finished successfully, writes are visible to future transactions
- **Aborted** -- cancelled, writes are discarded

The `Copy` derive is new. It means this type can be copied with a simple memory copy (like an integer), instead of requiring `.clone()`. Only small, simple types can be `Copy` -- enums without heap data qualify.

### Step 2: Define the Transaction struct

```rust,ignore
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
```

Let's understand each field:

**`snapshot_version`** -- The "as of" version. When this transaction reads, it sees data at this version and ignores anything newer. Think of it as a timestamp that freezes the transaction's view of the world.

**`write_version`** -- The version number that will be assigned to all writes when this transaction commits. It is always higher than the snapshot version.

**`writes`** -- A buffer of uncommitted changes. These are invisible to other transactions until commit. The value is `Option<Value>`: `Some(value)` for sets, `None` for deletes.

**`state`** -- Prevents operations on committed or aborted transactions.

```rust,ignore
impl Transaction {
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

> **What just happened?**
>
> We defined a `Transaction` that carries its own snapshot version and a buffer of uncommitted writes. The snapshot version determines what the transaction can *see*. The write buffer determines what the transaction will *change*. Nothing is written to the store until `commit()` is called -- this gives us atomicity (all-or-nothing).

### Step 3: Implement transaction operations

```rust,ignore
impl Transaction {
    /// Read a key. First checks local writes, then falls back to the store.
    pub fn get(&self, store: &MvccStore, key: &str) -> Result<Option<Value>, String> {
        if self.state != TransactionState::Active {
            return Err("Transaction is not active".to_string());
        }

        // Check local writes first -- we should see our own uncommitted changes
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
}
```

Let's trace the `get` method step by step:

1. **State check:** If the transaction is committed or aborted, return an error. You should not be reading from a finished transaction.

2. **Check local writes:** If this transaction has already written to this key (even if not committed yet), return that value. A transaction should see its own writes.

3. **Fall back to the store:** Read from the MVCC store at our `snapshot_version`. The `.cloned()` at the end converts `Option<&Value>` to `Option<Value>` by cloning the value. We need to clone because we are returning an owned value, not a reference.

> **What just happened?**
>
> The `get` method has a two-level lookup: local writes first, then the store. This means a transaction always sees its own uncommitted changes (like a transaction that sets a key and immediately reads it back). But it never sees other transactions' uncommitted changes -- those are in their own write buffers.

### Step 4: Implement commit and abort

```rust,ignore
impl Transaction {
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

**Important: `commit` and `abort` take `self`, not `&mut self`.**

This is a crucial Rust design choice. By taking `self` (not a reference), these methods **consume** the transaction. After calling `commit()`, the `Transaction` variable is gone -- you cannot use it anymore. The compiler enforces this:

```rust,ignore
let txn = store.begin();
txn.commit(&mut store).unwrap();
// txn.set("x", Value::Integer(1));  // ERROR: value moved
```

This prevents a common bug: accidentally writing to a transaction after it has been committed. In other languages, this would be a runtime error. In Rust, it is a compile-time error.

**`self.writes.drain()`** -- Removes all entries from the HashMap and iterates over them. After `drain()`, the HashMap is empty. Each entry is a `(String, Option<Value>)` tuple.

> **What just happened?**
>
> `commit()` takes all the buffered writes and applies them to the store at the transaction's write version. `abort()` simply throws away the buffer. Both methods consume the transaction -- you cannot use it after committing or aborting.
>
> This is **atomicity**: either all writes succeed (commit) or none of them do (abort). There is no partial state.

### Step 5: Add a begin() method to MvccStore

```rust,ignore
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

**`self.next_version - 1`** -- The snapshot sees everything up to (but not including) the current uncommitted version. If `next_version` is 5, the snapshot sees versions 1-4.

**`self.next_version()`** -- Allocates the next version number for this transaction's writes. This also advances the counter, so the next transaction gets a different version.

### Step 6: Test the transaction flow

```rust,ignore
#[cfg(test)]
mod transaction_tests {
    use super::*;

    #[test]
    fn basic_transaction_flow() {
        let mut store = MvccStore::new();

        // Write initial data outside a transaction (version 1)
        store.write("name", 1, Value::String("Alice".to_string()));
        store.next_version = 2; // advance past the manual write

        // Begin a transaction -- it sees version 1
        let mut txn = store.begin();
        assert_eq!(
            txn.get(&store, "name").unwrap().unwrap().as_str().unwrap(),
            "Alice"
        );

        // Write within the transaction (buffered, not yet visible to store)
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
}
```

Let's trace the unwrap chain in the assertions:

```rust,ignore
txn.get(&store, "name")    // Result<Option<Value>, String>
    .unwrap()               // Option<Value> -- unwrap the Result
    .unwrap()               // Value -- unwrap the Option
    .as_str()               // Option<&str> -- extract string if it is a String variant
    .unwrap()               // &str -- unwrap the Option
```

Each `.unwrap()` says "I expect this to succeed; crash if it doesn't." This is fine in tests. In production code, you would use `?` or `match`.

```
$ cargo test transaction_tests
running 3 tests
test mvcc::transaction_tests::basic_transaction_flow ... ok
test mvcc::transaction_tests::abort_discards_writes ... ok
test mvcc::transaction_tests::transaction_delete ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

> **What just happened?**
>
> We tested the complete transaction lifecycle:
> - **Read** existing data through a transaction
> - **Write** new data (buffered in the transaction)
> - **Read back** our own write (local buffer takes priority)
> - **Commit** makes writes visible to future transactions
> - **Abort** discards writes -- the old data is still there
> - **Delete** within a transaction buffers a tombstone

---

## Exercise 4: Test Snapshot Isolation

**Goal:** Prove that snapshot isolation works -- two transactions started at different times see consistent, non-interfering views of the data. This is the whole point of MVCC.

### Step 1: The bank transfer test

This is the test that matters. It proves that MVCC solves the problem from Exercise 1:

```rust,ignore
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

> **What just happened?**
>
> This is the fix for the problem we demonstrated in Exercise 1. Let's walk through the timeline:
>
> 1. **Initial state (version 1):** Alice=$1000, Bob=$500, Total=$1500
> 2. **Reader begins:** Gets snapshot at version 1. It will always see Alice=$1000, Bob=$500.
> 3. **Writer begins:** Gets snapshot at version 1, write version 3.
> 4. **Writer transfers $200:** Buffers alice=$800, bob=$700 in its local writes.
> 5. **Writer commits:** Writes alice=$800 at version 3, bob=$700 at version 3.
> 6. **Reader reads AFTER the commit:** Still sees version 1 data! Alice=$1000, Bob=$500. The total is always $1500. Money never vanishes.
> 7. **New transaction:** Starts after the commit, gets a snapshot that includes version 3. Sees Alice=$800, Bob=$700. Total=$1500.
>
> The reader's view is **frozen in time**. No matter what happens after it begins, it always sees the same consistent data.

### Step 2: Test interleaved reads and writes

```rust,ignore
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

        // Txn A reads x again -- still sees 10 (snapshot isolation!)
        let x2 = txn_a.get(&store, "x").unwrap().unwrap().as_integer().unwrap();
        assert_eq!(x2, 10);
        assert_eq!(x1, x2, "Same transaction, same snapshot, same value");
    }
```

This test proves **repeatable reads**: within a single transaction, reading the same key always returns the same value, even if another transaction modifies and commits that key in between.

### Step 3: Test multiple concurrent writers

```rust,ignore
    #[test]
    fn multiple_writers_create_separate_versions() {
        let mut store = MvccStore::new();

        store.write("counter", 1, Value::Integer(0));
        store.next_version = 2;

        // Three transactions, each writing to the same key
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

    #[test]
    fn read_missing_key_returns_none() {
        let mut store = MvccStore::new();
        let txn = store.begin();
        assert!(txn.get(&store, "nonexistent").unwrap().is_none());
    }
```

```
$ cargo test isolation_tests
running 4 tests
test mvcc::isolation_tests::snapshot_isolation_bank_transfer ... ok
test mvcc::isolation_tests::interleaved_reads_and_writes ... ok
test mvcc::isolation_tests::multiple_writers_create_separate_versions ... ok
test mvcc::isolation_tests::read_missing_key_returns_none ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

> **What just happened?**
>
> Every test proves the same principle: a transaction's snapshot is immutable. No matter what other transactions do after it begins, it always sees the same data. This is the guarantee that makes databases reliable.
>
> The "multiple writers" test shows that when three transactions all write to the same key, each gets its own version. The last to commit wins -- its version is the highest and will be seen by future readers.

### Common mistakes with MVCC

**Mistake: Thinking commits are instantaneous**

In our implementation, commit is indeed instant (a single-threaded loop). In a real distributed database, commit might involve network round trips, disk flushes, and consensus protocols. The isolation guarantee holds regardless -- the snapshot is determined at begin time, not commit time.

**Mistake: Confusing snapshot version with write version**

- **Snapshot version:** What the transaction *sees*. Set at begin time. Never changes.
- **Write version:** Where the transaction's writes will be stored. Also set at begin time. Also never changes.

A transaction with `snapshot=5, write_version=8` reads data from versions 1-5 and writes data at version 8.

**Mistake: Expecting write-write conflict detection**

Our implementation does not detect write-write conflicts. If two transactions both modify the same key and both commit, the last one silently wins. This is called "lost update." A production MVCC system would detect this and abort one of the transactions. We keep it simple here -- the concept of snapshot isolation for reads is the important lesson.

---
