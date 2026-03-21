# MVCC Version Chains — "Every row remembers its past"

Two bank tellers are working at adjacent windows. Teller A is processing a transfer: deduct $100 from Account 42, add $100 to Account 99. Teller B is running a report: sum up all account balances. If Teller B reads Account 42 after the deduction but Account 99 before the deposit, the report shows $100 less than reality. Money has vanished into thin air.

The traditional fix is locking: Teller A locks both accounts, makes the transfer, then unlocks. But locks mean waiting. Teller B's report freezes until Teller A finishes. With thousands of concurrent transactions, the database grinds to a halt.

MVCC -- Multi-Version Concurrency Control -- solves this without locks. Instead of overwriting data in place, every write creates a new **version**. Readers see the version that was current when their transaction started. Teller B sees the old balances. Teller A sees the new ones. Both work simultaneously, both see consistent data, and nobody waits.

Let's build the version chain from scratch.

---

## The Naive Way

The simplest approach: lock everything.

```rust
fn main() {
    use std::collections::HashMap;

    let mut accounts: HashMap<&str, i64> = HashMap::new();
    accounts.insert("acct_42", 500);
    accounts.insert("acct_99", 300);
    let mut locked = false;

    // Teller A: transfer $100 from acct_42 to acct_99
    // Step 1: acquire lock
    locked = true;

    // Step 2: modify
    *accounts.get_mut("acct_42").unwrap() -= 100;
    // ... imagine a slow network call here ...
    *accounts.get_mut("acct_99").unwrap() += 100;

    // Step 3: release lock
    locked = false;

    // Teller B cannot read ANYTHING during the transfer.
    // Even accounts completely unrelated to the transfer are locked.
    if !locked {
        let total: i64 = accounts.values().sum();
        println!("Total: ${}", total); // 800, correct
    }

    println!("Problem: Teller B waited for the ENTIRE transfer.");
    println!("With 1000 concurrent transactions, this is a bottleneck.");
}
```

Locking works for correctness but kills concurrency. And the granularity problem is real: do you lock the whole table, individual rows, or something in between? Every choice has trade-offs, and all of them involve waiting.

---

## The Insight

Imagine a library with a special rule: books are never erased or overwritten. Instead, every edit creates a new edition. Edition 1 has the original text. Edition 2 has corrections. Edition 3 has more corrections. When a reader checks out the book, they get the latest edition available at that moment. If someone publishes Edition 4 while they are reading, they still see Edition 3 -- their view is frozen in time.

This is MVCC. Each row in the database is not a single value but a **chain of versions**. Each version is tagged with:
- **created_at**: the transaction ID that created this version
- **deleted_at**: the transaction ID that deleted (or replaced) this version, if any

When a transaction reads a row, it does not just grab the latest version. It walks the version chain and finds the version that was **visible** to it -- created before the transaction started, and not yet deleted (or deleted after the transaction started).

The visibility rule is the heart of MVCC:

> A version is visible to transaction T if:
> 1. The version was created by a transaction that committed before T started
> 2. The version was NOT deleted, OR was deleted by a transaction that had not committed when T started

Let's build this.

---

## The Build

### Version and Transaction IDs

Each transaction gets a monotonically increasing ID. Each version records which transaction created it and which (if any) deleted it:

```rust
type TxnId = u64;

#[derive(Debug, Clone)]
struct Version {
    value: i64,          // the actual data (simplified to i64)
    created_by: TxnId,   // transaction that created this version
    deleted_by: Option<TxnId>, // transaction that replaced/deleted it
}
```

### The Version Chain

A row is a vector of versions, ordered from oldest to newest:

```rust
#[derive(Debug, Clone)]
struct VersionedRow {
    key: String,
    versions: Vec<Version>,
}

impl VersionedRow {
    fn new(key: String) -> Self {
        VersionedRow {
            key,
            versions: Vec::new(),
        }
    }

    fn add_version(&mut self, value: i64, txn_id: TxnId) {
        // Mark the current latest version as deleted by this transaction
        if let Some(latest) = self.versions.last_mut() {
            if latest.deleted_by.is_none() {
                latest.deleted_by = Some(txn_id);
            }
        }

        self.versions.push(Version {
            value,
            created_by: txn_id,
            deleted_by: None,
        });
    }
}
```

### The Visibility Check

This is the critical function. Given a version and a reader's transaction ID, determine whether the reader should see this version:

```rust
fn is_visible(
    version: &Version,
    reader_txn: TxnId,
    committed: &std::collections::HashSet<TxnId>,
) -> bool {
    // Rule 1: The creating transaction must have committed
    // AND must have started before the reader
    let creator_committed = committed.contains(&version.created_by);
    let created_before_reader = version.created_by < reader_txn;

    if !creator_committed || !created_before_reader {
        return false;
    }

    // Rule 2: If deleted, the deleting transaction must NOT have
    // committed (from the reader's perspective)
    match version.deleted_by {
        None => true, // not deleted, so visible
        Some(deleter) => {
            // Visible if the deleter hasn't committed yet,
            // or if the deleter started after the reader
            let deleter_committed = committed.contains(&deleter);
            let deleted_before_reader = deleter < reader_txn;
            !(deleter_committed && deleted_before_reader)
        }
    }
}
```

Read this function carefully. It implements snapshot isolation -- the idea that each transaction sees a frozen snapshot of the database as it existed when the transaction started. A version is visible only if its creator had committed before we started, and its deleter (if any) had NOT committed before we started.

### The MVCC Store

Now we build the full store that manages transactions and versioned rows:

```rust
use std::collections::{HashMap, HashSet};

struct MvccStore {
    rows: HashMap<String, VersionedRow>,
    next_txn_id: TxnId,
    active_txns: HashSet<TxnId>,
    committed_txns: HashSet<TxnId>,
}

impl MvccStore {
    fn new() -> Self {
        MvccStore {
            rows: HashMap::new(),
            next_txn_id: 1,
            active_txns: HashSet::new(),
            committed_txns: HashSet::new(),
        }
    }

    fn begin(&mut self) -> TxnId {
        let txn_id = self.next_txn_id;
        self.next_txn_id += 1;
        self.active_txns.insert(txn_id);
        txn_id
    }

    fn commit(&mut self, txn_id: TxnId) {
        self.active_txns.remove(&txn_id);
        self.committed_txns.insert(txn_id);
    }

    fn write(&mut self, txn_id: TxnId, key: &str, value: i64) {
        let row = self.rows
            .entry(key.to_string())
            .or_insert_with(|| VersionedRow::new(key.to_string()));
        row.add_version(value, txn_id);
    }

    fn read(&self, txn_id: TxnId, key: &str) -> Option<i64> {
        let row = self.rows.get(key)?;

        // Walk versions from newest to oldest, return first visible one
        for version in row.versions.iter().rev() {
            if is_visible(version, txn_id, &self.committed_txns) {
                return Some(version.value);
            }
        }
        None
    }

    fn delete(&mut self, txn_id: TxnId, key: &str) {
        if let Some(row) = self.rows.get_mut(key) {
            // Mark the visible version as deleted
            for version in row.versions.iter_mut().rev() {
                if is_visible(version, txn_id, &self.committed_txns) {
                    version.deleted_by = Some(txn_id);
                    break;
                }
            }
        }
    }
}
```

The `read` function walks the version chain from newest to oldest and returns the first visible version. This means newer versions shadow older ones naturally. When we write a new value, the old version gets marked as deleted by the writing transaction, and the new version becomes visible to future readers.

---

## The Payoff

Here is the full, runnable implementation demonstrating the bank transfer scenario:

```rust
use std::collections::{HashMap, HashSet};

type TxnId = u64;

#[derive(Debug, Clone)]
struct Version {
    value: i64,
    created_by: TxnId,
    deleted_by: Option<TxnId>,
}

#[derive(Debug, Clone)]
struct VersionedRow {
    key: String,
    versions: Vec<Version>,
}

impl VersionedRow {
    fn new(key: String) -> Self {
        VersionedRow { key, versions: Vec::new() }
    }

    fn add_version(&mut self, value: i64, txn_id: TxnId) {
        if let Some(latest) = self.versions.last_mut() {
            if latest.deleted_by.is_none() {
                latest.deleted_by = Some(txn_id);
            }
        }
        self.versions.push(Version {
            value,
            created_by: txn_id,
            deleted_by: None,
        });
    }
}

fn is_visible(
    version: &Version,
    reader_txn: TxnId,
    committed: &HashSet<TxnId>,
) -> bool {
    let creator_committed = committed.contains(&version.created_by);
    let created_before_reader = version.created_by < reader_txn;
    if !creator_committed || !created_before_reader {
        return false;
    }
    match version.deleted_by {
        None => true,
        Some(deleter) => {
            let deleter_committed = committed.contains(&deleter);
            let deleted_before_reader = deleter < reader_txn;
            !(deleter_committed && deleted_before_reader)
        }
    }
}

struct MvccStore {
    rows: HashMap<String, VersionedRow>,
    next_txn_id: TxnId,
    active_txns: HashSet<TxnId>,
    committed_txns: HashSet<TxnId>,
}

impl MvccStore {
    fn new() -> Self {
        MvccStore {
            rows: HashMap::new(), next_txn_id: 1,
            active_txns: HashSet::new(), committed_txns: HashSet::new(),
        }
    }

    fn begin(&mut self) -> TxnId {
        let id = self.next_txn_id; self.next_txn_id += 1;
        self.active_txns.insert(id); id
    }

    fn commit(&mut self, txn_id: TxnId) {
        self.active_txns.remove(&txn_id);
        self.committed_txns.insert(txn_id);
    }

    fn write(&mut self, txn_id: TxnId, key: &str, value: i64) {
        let row = self.rows.entry(key.to_string())
            .or_insert_with(|| VersionedRow::new(key.to_string()));
        row.add_version(value, txn_id);
    }

    fn read(&self, txn_id: TxnId, key: &str) -> Option<i64> {
        let row = self.rows.get(key)?;
        for version in row.versions.iter().rev() {
            if is_visible(version, txn_id, &self.committed_txns) {
                return Some(version.value);
            }
        }
        None
    }
}

fn main() {
    let mut store = MvccStore::new();

    // Setup: create accounts with committed values
    let setup = store.begin();
    store.write(setup, "acct_42", 500);
    store.write(setup, "acct_99", 300);
    store.commit(setup);

    println!("=== Initial State ===");
    println!("acct_42: $500, acct_99: $300, total: $800\n");

    // Teller B starts a report BEFORE the transfer
    let report_txn = store.begin();
    println!("Teller B starts report (txn {})", report_txn);

    // Teller A starts a transfer
    let transfer_txn = store.begin();
    println!("Teller A starts transfer (txn {})", transfer_txn);

    // Teller A deducts from acct_42
    let old_42 = store.read(transfer_txn, "acct_42").unwrap();
    store.write(transfer_txn, "acct_42", old_42 - 100);
    println!("Teller A: deducted $100 from acct_42 (now ${})", old_42 - 100);

    // === KEY MOMENT ===
    // Teller B reads DURING the transfer (before commit)
    let b_sees_42 = store.read(report_txn, "acct_42").unwrap();
    let b_sees_99 = store.read(report_txn, "acct_99").unwrap();
    println!("\nTeller B reads (mid-transfer):");
    println!("  acct_42: ${}", b_sees_42); // should see 500 (old value)
    println!("  acct_99: ${}", b_sees_99); // should see 300 (old value)
    println!("  total:   ${}", b_sees_42 + b_sees_99); // should be 800

    // Teller A adds to acct_99 and commits
    let old_99 = store.read(transfer_txn, "acct_99").unwrap();
    store.write(transfer_txn, "acct_99", old_99 + 100);
    store.commit(transfer_txn);
    println!("\nTeller A: committed transfer");

    // Teller B reads AGAIN -- still sees the old snapshot!
    let b_sees_42_again = store.read(report_txn, "acct_42").unwrap();
    let b_sees_99_again = store.read(report_txn, "acct_99").unwrap();
    println!("\nTeller B reads again (after transfer committed):");
    println!("  acct_42: ${}", b_sees_42_again); // still 500!
    println!("  acct_99: ${}", b_sees_99_again); // still 300!
    println!("  total:   ${}", b_sees_42_again + b_sees_99_again); // still 800!

    // A NEW transaction sees the updated values
    let new_txn = store.begin();
    let new_42 = store.read(new_txn, "acct_42").unwrap();
    let new_99 = store.read(new_txn, "acct_99").unwrap();
    println!("\nNew transaction sees:");
    println!("  acct_42: ${}", new_42); // 400
    println!("  acct_99: ${}", new_99); // 400
    println!("  total:   ${}", new_42 + new_99); // 800 -- still consistent!

    // Show version chain
    println!("\n=== Version Chain for acct_42 ===");
    if let Some(row) = store.rows.get("acct_42") {
        for (i, v) in row.versions.iter().enumerate() {
            println!("  v{}: value=${}, created_by=txn{}, deleted_by={:?}",
                i, v.value, v.created_by, v.deleted_by);
        }
    }
}
```

Teller B sees $800 total throughout the entire report, even though Teller A modified and committed in the middle. No locks, no waiting, no inconsistency. Each transaction gets its own consistent snapshot of the world.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Begin transaction | O(1) | O(1) | Assign ID, add to active set |
| Read (find visible version) | O(v) | O(1) | v = versions for this key |
| Write (add version) | O(1) | O(1) per version | Append to version chain |
| Commit | O(1) | O(1) | Move from active to committed set |
| Visibility check | O(1) | O(1) | Two comparisons + set lookup |
| Garbage collection | O(n * v) | Frees old versions | Remove versions invisible to all |
| Space per row | -- | O(v) | v = number of active versions |

The main cost of MVCC is space. Every update creates a new version instead of overwriting in place. A row updated 1,000 times has 1,000 versions. Without garbage collection, the version chain grows without bound. Real databases run a **vacuum** process that removes versions no longer visible to any active transaction.

---

## Where This Shows Up in Our Database

In Chapter 5, we implement MVCC for our storage engine to support concurrent transactions:

```rust,ignore
// Each key-value pair carries version metadata
pub struct MvccEntry {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub version: u64,
    pub deleted: bool,
}

// The MVCC layer wraps the raw storage engine
pub struct Mvcc<S: Storage> {
    storage: S,
    next_version: u64,
}
```

MVCC is not unique to our toy database. It is the dominant concurrency control mechanism in production:
- **PostgreSQL** stores old versions in the main table and uses VACUUM to clean them up
- **MySQL/InnoDB** stores old versions in a separate "undo log" and links them via rollback pointers
- **Oracle** uses undo tablespaces to reconstruct old versions on demand
- **SQLite** uses a write-ahead log (WAL) to provide snapshot isolation

The core idea is always the same: never destroy old data, let readers see the past, and garbage-collect when nobody needs it anymore.

---

## Try It Yourself

### Exercise 1: Garbage Collection

Implement a `gc(&mut self)` method on `MvccStore` that removes versions that are no longer visible to ANY active transaction. A version can be garbage collected if: (a) it has been deleted, AND (b) the deleting transaction committed, AND (c) no active transaction could possibly need to see it (i.e., all active transactions started after the deleting transaction).

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

type TxnId = u64;

#[derive(Debug, Clone)]
struct Version {
    value: i64,
    created_by: TxnId,
    deleted_by: Option<TxnId>,
}

#[derive(Debug, Clone)]
struct VersionedRow {
    versions: Vec<Version>,
}

struct MvccStore {
    rows: HashMap<String, VersionedRow>,
    next_txn_id: TxnId,
    active_txns: HashSet<TxnId>,
    committed_txns: HashSet<TxnId>,
}

impl MvccStore {
    fn new() -> Self {
        MvccStore {
            rows: HashMap::new(),
            next_txn_id: 1,
            active_txns: HashSet::new(),
            committed_txns: HashSet::new(),
        }
    }

    fn begin(&mut self) -> TxnId {
        let id = self.next_txn_id;
        self.next_txn_id += 1;
        self.active_txns.insert(id);
        id
    }

    fn commit(&mut self, txn_id: TxnId) {
        self.active_txns.remove(&txn_id);
        self.committed_txns.insert(txn_id);
    }

    fn write(&mut self, txn_id: TxnId, key: &str, value: i64) {
        let row = self.rows.entry(key.to_string())
            .or_insert(VersionedRow { versions: Vec::new() });
        if let Some(latest) = row.versions.last_mut() {
            if latest.deleted_by.is_none() {
                latest.deleted_by = Some(txn_id);
            }
        }
        row.versions.push(Version {
            value, created_by: txn_id, deleted_by: None,
        });
    }

    fn gc(&mut self) -> usize {
        // Find the oldest active transaction
        let oldest_active = self.active_txns.iter().copied().min();

        let mut removed = 0;

        for (_key, row) in self.rows.iter_mut() {
            row.versions.retain(|version| {
                // Keep if not deleted
                let Some(deleter) = version.deleted_by else {
                    return true;
                };

                // Keep if deleter hasn't committed
                if !self.committed_txns.contains(&deleter) {
                    return true;
                }

                // Keep if any active transaction might need this version
                if let Some(oldest) = oldest_active {
                    if deleter >= oldest {
                        return true; // an active txn started before the delete
                    }
                }

                // Safe to garbage collect
                removed += 1;
                false
            });
        }

        removed
    }
}

fn main() {
    let mut store = MvccStore::new();

    // Create initial data
    let t1 = store.begin();
    store.write(t1, "key1", 100);
    store.commit(t1);

    // Update several times
    for i in 0..5 {
        let t = store.begin();
        store.write(t, "key1", 100 + (i + 1) * 10);
        store.commit(t);
    }

    let versions_before = store.rows["key1"].versions.len();
    println!("Versions before GC: {}", versions_before);

    let removed = store.gc();
    let versions_after = store.rows["key1"].versions.len();
    println!("Removed: {}", removed);
    println!("Versions after GC: {}", versions_after);
    // With no active transactions, all deleted versions can be collected.
    // Only the latest (un-deleted) version remains.
}
```

</details>

### Exercise 2: Write-Write Conflict Detection

Two concurrent transactions both try to update the same key. The second one to commit should be rejected (first-writer-wins). Implement a `write_checked` method that returns `Err` if another active (uncommitted) transaction has already written to this key.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

type TxnId = u64;

#[derive(Debug, Clone)]
struct Version {
    value: i64,
    created_by: TxnId,
    deleted_by: Option<TxnId>,
}

struct MvccStore {
    rows: HashMap<String, Vec<Version>>,
    next_txn_id: TxnId,
    active_txns: HashSet<TxnId>,
    committed_txns: HashSet<TxnId>,
}

impl MvccStore {
    fn new() -> Self {
        MvccStore {
            rows: HashMap::new(),
            next_txn_id: 1,
            active_txns: HashSet::new(),
            committed_txns: HashSet::new(),
        }
    }

    fn begin(&mut self) -> TxnId {
        let id = self.next_txn_id;
        self.next_txn_id += 1;
        self.active_txns.insert(id);
        id
    }

    fn commit(&mut self, txn_id: TxnId) {
        self.active_txns.remove(&txn_id);
        self.committed_txns.insert(txn_id);
    }

    fn abort(&mut self, txn_id: TxnId) {
        self.active_txns.remove(&txn_id);
        // Remove versions created by this transaction
        for versions in self.rows.values_mut() {
            versions.retain(|v| v.created_by != txn_id);
            // Un-delete versions that this txn marked as deleted
            for v in versions.iter_mut() {
                if v.deleted_by == Some(txn_id) {
                    v.deleted_by = None;
                }
            }
        }
    }

    fn write_checked(
        &mut self,
        txn_id: TxnId,
        key: &str,
        value: i64,
    ) -> Result<(), String> {
        // Check: has another ACTIVE transaction written to this key?
        if let Some(versions) = self.rows.get(key) {
            for version in versions {
                if version.created_by != txn_id
                    && self.active_txns.contains(&version.created_by)
                {
                    return Err(format!(
                        "write-write conflict: txn {} already wrote to '{}'",
                        version.created_by, key
                    ));
                }
            }
        }

        // Safe to write
        let versions = self.rows.entry(key.to_string()).or_insert_with(Vec::new);
        if let Some(latest) = versions.last_mut() {
            if latest.deleted_by.is_none() {
                latest.deleted_by = Some(txn_id);
            }
        }
        versions.push(Version {
            value,
            created_by: txn_id,
            deleted_by: None,
        });
        Ok(())
    }
}

fn main() {
    let mut store = MvccStore::new();

    // Setup
    let setup = store.begin();
    store.write_checked(setup, "balance", 500).unwrap();
    store.commit(setup);

    // Two concurrent transactions try to update the same key
    let t1 = store.begin();
    let t2 = store.begin();

    // T1 writes first -- succeeds
    store.write_checked(t1, "balance", 400).unwrap();
    println!("T1 wrote balance=400: OK");

    // T2 tries to write -- should fail
    match store.write_checked(t2, "balance", 600) {
        Ok(_) => println!("T2 wrote: OK (unexpected!)"),
        Err(e) => println!("T2 blocked: {}", e),
    }

    // T1 commits, T2 must abort and retry
    store.commit(t1);
    store.abort(t2);

    // T2 retries with a new transaction
    let t2_retry = store.begin();
    store.write_checked(t2_retry, "balance", 600).unwrap();
    store.commit(t2_retry);
    println!("T2 retry succeeded: balance=600");
}
```

</details>

### Exercise 3: Transaction Snapshot

Implement a `snapshot` method that returns a `HashMap<String, i64>` representing all key-value pairs visible to a given transaction. Use this to implement a `scan` operation that returns all visible rows. Test with overlapping transactions that create, update, and delete different keys.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

type TxnId = u64;

#[derive(Debug, Clone)]
struct Version {
    value: i64,
    created_by: TxnId,
    deleted_by: Option<TxnId>,
}

fn is_visible(v: &Version, reader: TxnId, committed: &HashSet<TxnId>) -> bool {
    if !committed.contains(&v.created_by) || v.created_by >= reader {
        return false;
    }
    match v.deleted_by {
        None => true,
        Some(d) => !(committed.contains(&d) && d < reader),
    }
}

struct MvccStore {
    rows: HashMap<String, Vec<Version>>,
    next_txn_id: TxnId,
    active_txns: HashSet<TxnId>,
    committed_txns: HashSet<TxnId>,
}

impl MvccStore {
    fn new() -> Self {
        MvccStore {
            rows: HashMap::new(), next_txn_id: 1,
            active_txns: HashSet::new(), committed_txns: HashSet::new(),
        }
    }
    fn begin(&mut self) -> TxnId {
        let id = self.next_txn_id; self.next_txn_id += 1;
        self.active_txns.insert(id); id
    }
    fn commit(&mut self, id: TxnId) {
        self.active_txns.remove(&id); self.committed_txns.insert(id);
    }
    fn write(&mut self, txn: TxnId, key: &str, value: i64) {
        let vers = self.rows.entry(key.to_string()).or_insert_with(Vec::new);
        if let Some(last) = vers.last_mut() {
            if last.deleted_by.is_none() { last.deleted_by = Some(txn); }
        }
        vers.push(Version { value, created_by: txn, deleted_by: None });
    }
    fn delete(&mut self, txn: TxnId, key: &str) {
        if let Some(vers) = self.rows.get_mut(key) {
            for v in vers.iter_mut().rev() {
                if is_visible(v, txn, &self.committed_txns) {
                    v.deleted_by = Some(txn);
                    break;
                }
            }
        }
    }
    fn snapshot(&self, txn: TxnId) -> HashMap<String, i64> {
        let mut result = HashMap::new();
        for (key, versions) in &self.rows {
            // Find the latest visible version
            for v in versions.iter().rev() {
                if is_visible(v, txn, &self.committed_txns) {
                    result.insert(key.clone(), v.value);
                    break;
                }
            }
        }
        result
    }
}

fn main() {
    let mut store = MvccStore::new();

    // T1: create three accounts
    let t1 = store.begin();
    store.write(t1, "alice", 100);
    store.write(t1, "bob", 200);
    store.write(t1, "carol", 300);
    store.commit(t1);

    // T2: start a long-running report
    let t2 = store.begin();

    // T3: update alice, delete bob, add dave
    let t3 = store.begin();
    store.write(t3, "alice", 150);
    store.delete(t3, "bob");
    store.write(t3, "dave", 400);
    store.commit(t3);

    // T2's snapshot should show the ORIGINAL state
    let snap_t2 = store.snapshot(t2);
    println!("T2 snapshot (pre-T3):");
    let mut keys: Vec<_> = snap_t2.keys().collect();
    keys.sort();
    for k in &keys {
        println!("  {}: ${}", k, snap_t2[*k]);
    }
    // alice=100, bob=200, carol=300 (no dave)

    // New transaction sees T3's changes
    let t4 = store.begin();
    let snap_t4 = store.snapshot(t4);
    println!("\nT4 snapshot (post-T3):");
    let mut keys: Vec<_> = snap_t4.keys().collect();
    keys.sort();
    for k in &keys {
        println!("  {}: ${}", k, snap_t4[*k]);
    }
    // alice=150, carol=300, dave=400 (no bob)
}
```

</details>

---

## Recap

MVCC replaces locks with versions. Every write creates a new version, every read finds the right version for its snapshot, and garbage collection cleans up versions nobody needs. The visibility rule -- two simple checks on transaction IDs and commit status -- is the engine that makes it all work. Readers never block writers. Writers never block readers. The database stays consistent, and nobody waits.
