# Snapshot Isolation — "Reading yesterday's newspaper today"

Your database has a table with 10 million rows. A user starts a long-running analytics query: `SELECT SUM(balance) FROM accounts`. The query takes 30 seconds to scan all rows. Meanwhile, another user transfers $1,000 from account A to account B. The transfer is two operations: `UPDATE accounts SET balance = balance - 1000 WHERE id = 'A'` and `UPDATE accounts SET balance = balance + 1000 WHERE id = 'B'`.

The analytics query has already scanned past account A (seeing the original balance) but has not yet reached account B. The transfer completes, subtracting from A and adding to B. When the query reaches account B, it sees the new, higher balance. The result: the sum is $1,000 too high. Money appeared from nowhere. The query read a state that never existed -- half before the transfer and half after.

This is a **read anomaly**, and it violates the fundamental guarantee of a transaction: seeing a consistent snapshot of the data. Snapshot isolation fixes this by giving each transaction its own frozen view of the database, as if the entire database were photographed at the moment the transaction began.

---

## The Naive Way

The simplest way to prevent read anomalies: lock the entire database for every transaction. While the analytics query runs, nobody else can write. While the transfer runs, nobody else can read:

```rust
use std::collections::HashMap;

fn main() {
    let mut db: HashMap<String, i64> = HashMap::new();
    db.insert("Alice".to_string(), 5000);
    db.insert("Bob".to_string(), 3000);
    db.insert("Carol".to_string(), 7000);

    // Transaction 1: Analytics query (exclusive lock on entire DB)
    println!("=== With Global Lock ===\n");
    println!("Transaction 1 (analytics) acquires EXCLUSIVE lock on entire DB");
    let sum: i64 = db.values().sum();
    println!("  SUM(balance) = {}", sum);
    println!("  Lock held for entire scan duration (~30 seconds for large tables)");
    println!("  ALL other transactions blocked during this time");
    println!("Transaction 1 releases lock\n");

    // Only now can the transfer happen
    println!("Transaction 2 (transfer) can finally start");
    *db.get_mut("Alice").unwrap() -= 1000;
    *db.get_mut("Bob").unwrap() += 1000;
    println!("  Transferred $1000 from Alice to Bob");

    let sum: i64 = db.values().sum();
    println!("  New SUM(balance) = {} (unchanged, transfer is balanced)\n", sum);

    // The result is correct, but...
    println!("Problem: every reader blocks every writer, and vice versa.");
    println!("A 30-second analytics query blocks ALL writes for 30 seconds.");
    println!("In a production database with 10,000 transactions/second,");
    println!("that is 300,000 transactions forced to wait.");
}
```

The global lock gives correct results but destroys concurrency. Readers block writers. Writers block readers. A single long-running query can bring the entire database to a standstill. This is not theoretical -- early database systems actually worked this way, and it was a constant source of production outages.

---

## The Insight

Think about reading a newspaper. When you pick up the morning edition, it is a **snapshot** of the world at press time -- say, 5 AM. While you read through it over the next hour, new things happen in the world: stocks change, news breaks, weather shifts. But your newspaper does not change. You read a consistent view of the world as of 5 AM.

If someone asks "what was the weather when the paper was printed?", you can give a correct answer. It does not matter that the weather has changed since. Your snapshot is internally consistent -- all the articles reflect the same moment in time.

**Snapshot isolation** gives each database transaction its own newspaper. When a transaction starts, it gets a **read timestamp** that identifies the version of the database it sees. All reads within that transaction return data as of that timestamp, regardless of what other transactions write in the meantime.

The trick: instead of overwriting data in place, every update creates a **new version** of the row, tagged with the writing transaction's timestamp. Old versions are kept around (temporarily) so that concurrent readers can still see them. This is called **Multi-Version Concurrency Control (MVCC)**.

The key properties:
1. **Readers never block writers**: a reader just sees an older version.
2. **Writers never block readers**: a writer creates a new version; old versions are still visible.
3. **Each transaction sees a consistent snapshot**: all reads return data as of the transaction's start time.

This is how PostgreSQL, Oracle, MySQL (InnoDB), and CockroachDB work. Let's build it.

---

## The Build

### Versioned Values

Instead of storing one value per key, we store a chain of versions. Each version has a timestamp (the transaction ID that created it) and the value:

```rust,ignore
#[derive(Debug, Clone)]
struct Version {
    created_by: u64,    // transaction ID that wrote this version
    value: Option<i64>, // None means "deleted" (tombstone)
}

#[derive(Debug)]
struct VersionedValue {
    versions: Vec<Version>, // newest first
}

impl VersionedValue {
    fn new() -> Self {
        VersionedValue { versions: Vec::new() }
    }

    /// Write a new version of this value.
    fn write(&mut self, txn_id: u64, value: Option<i64>) {
        // Insert at the front (newest first)
        self.versions.insert(0, Version {
            created_by: txn_id,
            value,
        });
    }
}
```

### Visibility Rules

A version is visible to a transaction if it was created by a committed transaction that started before the reader. This is the heart of snapshot isolation -- the visibility check:

```rust,ignore
use std::collections::HashSet;

impl VersionedValue {
    /// Find the version visible to a transaction with the given read timestamp.
    /// A version is visible if:
    /// 1. It was created by a transaction that committed before our read timestamp
    /// 2. OR it was created by our own transaction
    fn read(&self, txn_id: u64, read_ts: u64, committed: &HashSet<u64>) -> Option<i64> {
        for version in &self.versions {
            // Our own writes are always visible
            if version.created_by == txn_id {
                return version.value;
            }

            // Other transactions' writes are visible only if:
            // - The writing transaction has committed
            // - The writing transaction started before our read timestamp
            if version.created_by <= read_ts && committed.contains(&version.created_by) {
                return version.value;
            }
        }

        None // no visible version exists
    }
}
```

This is the crucial function. It scans the version chain from newest to oldest, looking for the first version that is (a) committed, and (b) created at or before our read timestamp. Uncommitted versions are invisible. Versions from transactions that started after us are invisible. We see exactly the state of the database as of our start time.

### The Transaction Manager

The transaction manager assigns timestamps, tracks active transactions, and manages commits:

```rust,ignore
struct TransactionManager {
    next_txn_id: u64,
    active: HashSet<u64>,    // currently running transactions
    committed: HashSet<u64>, // transactions that have committed
    aborted: HashSet<u64>,   // transactions that were rolled back
}

impl TransactionManager {
    fn new() -> Self {
        TransactionManager {
            next_txn_id: 1,
            active: HashSet::new(),
            committed: HashSet::new(),
            aborted: HashSet::new(),
        }
    }

    /// Start a new transaction. Returns (txn_id, read_timestamp).
    /// The read timestamp is the current "time" -- all committed
    /// transactions up to this point are visible.
    fn begin(&mut self) -> (u64, u64) {
        let txn_id = self.next_txn_id;
        self.next_txn_id += 1;
        let read_ts = txn_id; // simplified: read timestamp = txn ID
        self.active.insert(txn_id);
        (txn_id, read_ts)
    }

    fn commit(&mut self, txn_id: u64) -> bool {
        if self.active.remove(&txn_id) {
            self.committed.insert(txn_id);
            true
        } else {
            false
        }
    }

    fn abort(&mut self, txn_id: u64) -> bool {
        if self.active.remove(&txn_id) {
            self.aborted.insert(txn_id);
            true
        } else {
            false
        }
    }

    fn is_committed(&self, txn_id: u64) -> bool {
        self.committed.contains(&txn_id)
    }
}
```

### The MVCC Database

Putting it all together -- a database that uses versioned values and snapshot reads:

```rust,ignore
struct MvccDatabase {
    data: HashMap<String, VersionedValue>,
    txn_mgr: TransactionManager,
}

struct Transaction {
    id: u64,
    read_ts: u64,
    writes: Vec<(String, Option<i64>)>, // buffered writes
}

impl MvccDatabase {
    fn new() -> Self {
        MvccDatabase {
            data: HashMap::new(),
            txn_mgr: TransactionManager::new(),
        }
    }

    fn begin(&mut self) -> Transaction {
        let (id, read_ts) = self.txn_mgr.begin();
        Transaction {
            id,
            read_ts,
            writes: Vec::new(),
        }
    }

    fn read(&self, txn: &Transaction, key: &str) -> Option<i64> {
        match self.data.get(key) {
            Some(versioned) => versioned.read(
                txn.id,
                txn.read_ts,
                &self.txn_mgr.committed,
            ),
            None => None,
        }
    }

    fn write(&mut self, txn: &mut Transaction, key: &str, value: i64) {
        // Buffer the write
        txn.writes.push((key.to_string(), Some(value)));
        // Also apply immediately so the transaction can read its own writes
        self.data
            .entry(key.to_string())
            .or_insert_with(VersionedValue::new)
            .write(txn.id, Some(value));
    }

    fn delete(&mut self, txn: &mut Transaction, key: &str) {
        txn.writes.push((key.to_string(), None));
        self.data
            .entry(key.to_string())
            .or_insert_with(VersionedValue::new)
            .write(txn.id, None);
    }

    fn commit(&mut self, txn: Transaction) -> bool {
        self.txn_mgr.commit(txn.id)
    }

    fn abort(&mut self, txn: Transaction) {
        self.txn_mgr.abort(txn.id);
        // In a real implementation, we would also remove the
        // uncommitted versions from the version chains.
    }
}
```

### Write Conflict Detection

Snapshot isolation prevents read anomalies, but what about write-write conflicts? If two transactions both try to update the same key, we have a conflict. The **first committer wins** rule: the first transaction to commit succeeds; the second must abort:

```rust,ignore
impl MvccDatabase {
    /// Check for write-write conflicts before committing.
    /// A conflict exists if another transaction wrote to the same key
    /// and committed after our transaction began.
    fn check_write_conflicts(&self, txn: &Transaction) -> bool {
        for (key, _) in &txn.writes {
            if let Some(versioned) = self.data.get(key) {
                for version in &versioned.versions {
                    if version.created_by != txn.id
                        && version.created_by > txn.read_ts
                        && self.txn_mgr.is_committed(version.created_by)
                    {
                        // Another transaction committed a write to this key
                        // after our transaction started. Conflict!
                        return true;
                    }
                }
            }
        }
        false
    }

    fn commit_with_conflict_check(&mut self, txn: Transaction) -> Result<(), String> {
        if self.check_write_conflicts(&txn) {
            self.abort(txn);
            Err("write-write conflict: transaction aborted".to_string())
        } else {
            self.commit(txn);
            Ok(())
        }
    }
}
```

### Garbage Collection

Old versions consume memory. Once no active transaction can possibly need a version (because all transactions that started before that version was superseded have finished), the version can be garbage collected:

```rust,ignore
impl MvccDatabase {
    /// Remove versions that are no longer needed.
    /// A version can be removed if there is a newer committed version
    /// and no active transaction could still read the old version.
    fn garbage_collect(&mut self) {
        let oldest_active = self.txn_mgr.active.iter().min().copied();

        for versioned in self.data.values_mut() {
            if versioned.versions.len() <= 1 {
                continue; // keep at least one version
            }

            // Find the newest committed version visible to the oldest active txn
            let mut found_visible = false;
            versioned.versions.retain(|version| {
                if !found_visible {
                    // Keep everything until we find a version visible to
                    // the oldest active transaction
                    if self.txn_mgr.is_committed(version.created_by) {
                        match oldest_active {
                            Some(oldest) if version.created_by <= oldest => {
                                found_visible = true;
                                true // keep this version
                            }
                            None => {
                                found_visible = true;
                                true
                            }
                            _ => true, // newer than oldest active, keep
                        }
                    } else {
                        true // uncommitted, keep for now
                    }
                } else {
                    false // older than what anyone needs -- remove
                }
            });
        }
    }
}
```

---

## The Payoff

Let's demonstrate the original problem -- a long-running reader coexisting with a concurrent writer -- and show how snapshot isolation gives both correct results:

```rust
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct Version {
    created_by: u64,
    value: Option<i64>,
}

struct VersionedValue {
    versions: Vec<Version>,
}

impl VersionedValue {
    fn new() -> Self {
        VersionedValue { versions: Vec::new() }
    }

    fn write(&mut self, txn_id: u64, value: Option<i64>) {
        self.versions.insert(0, Version { created_by: txn_id, value });
    }

    fn read(&self, txn_id: u64, read_ts: u64, committed: &HashSet<u64>) -> Option<i64> {
        for version in &self.versions {
            if version.created_by == txn_id {
                return version.value;
            }
            if version.created_by <= read_ts && committed.contains(&version.created_by) {
                return version.value;
            }
        }
        None
    }
}

struct MvccDb {
    data: HashMap<String, VersionedValue>,
    next_id: u64,
    committed: HashSet<u64>,
}

impl MvccDb {
    fn new() -> Self {
        MvccDb {
            data: HashMap::new(),
            next_id: 1,
            committed: HashSet::new(),
        }
    }

    fn setup_write(&mut self, key: &str, value: i64) {
        let txn_id = self.next_id;
        self.next_id += 1;
        self.data
            .entry(key.to_string())
            .or_insert_with(VersionedValue::new)
            .write(txn_id, Some(value));
        self.committed.insert(txn_id);
    }

    fn begin_txn(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn read(&self, txn_id: u64, read_ts: u64, key: &str) -> Option<i64> {
        self.data.get(key)
            .and_then(|v| v.read(txn_id, read_ts, &self.committed))
    }

    fn write(&mut self, txn_id: u64, key: &str, value: i64) {
        self.data
            .entry(key.to_string())
            .or_insert_with(VersionedValue::new)
            .write(txn_id, Some(value));
    }

    fn commit(&mut self, txn_id: u64) {
        self.committed.insert(txn_id);
    }
}

fn main() {
    println!("=== Snapshot Isolation Demo ===\n");

    let mut db = MvccDb::new();

    // Setup: initial balances (committed before any transaction starts)
    db.setup_write("Alice", 5000);
    db.setup_write("Bob", 3000);
    db.setup_write("Carol", 7000);

    let expected_sum = 5000 + 3000 + 7000;
    println!("Initial balances: Alice=5000, Bob=3000, Carol=7000");
    println!("Expected SUM = {}\n", expected_sum);

    // Transaction T1: Long-running analytics query (starts first)
    let t1_id = db.begin_txn();
    let t1_read_ts = t1_id;
    println!("T1 (analytics) starts at timestamp {}", t1_read_ts);

    // T1 reads Alice's balance
    let alice_balance = db.read(t1_id, t1_read_ts, "Alice").unwrap();
    println!("T1 reads Alice: {}", alice_balance);

    // Now T2 starts and transfers $1000 from Alice to Bob
    let t2_id = db.begin_txn();
    println!("\nT2 (transfer) starts at timestamp {}", t2_id);
    db.write(t2_id, "Alice", 4000); // Alice: 5000 -> 4000
    db.write(t2_id, "Bob", 4000);   // Bob:   3000 -> 4000
    db.commit(t2_id);
    println!("T2 transfers $1000: Alice=4000, Bob=4000");
    println!("T2 commits\n");

    // T1 continues reading -- it should still see the OLD values
    let bob_balance = db.read(t1_id, t1_read_ts, "Bob").unwrap();
    let carol_balance = db.read(t1_id, t1_read_ts, "Carol").unwrap();
    println!("T1 reads Bob: {} (sees pre-transfer value!)", bob_balance);
    println!("T1 reads Carol: {}", carol_balance);

    let t1_sum = alice_balance + bob_balance + carol_balance;
    println!("\nT1's SUM = {} (expected {})", t1_sum, expected_sum);
    assert_eq!(t1_sum, expected_sum);
    println!("CORRECT! T1 sees a consistent snapshot from timestamp {}\n", t1_read_ts);

    // T3 starts after T2 committed -- it sees the NEW values
    let t3_id = db.begin_txn();
    let t3_read_ts = t3_id;
    let t3_alice = db.read(t3_id, t3_read_ts, "Alice").unwrap();
    let t3_bob = db.read(t3_id, t3_read_ts, "Bob").unwrap();
    let t3_carol = db.read(t3_id, t3_read_ts, "Carol").unwrap();
    let t3_sum = t3_alice + t3_bob + t3_carol;

    println!("T3 starts at timestamp {} (after T2 committed)", t3_read_ts);
    println!("T3 reads: Alice={}, Bob={}, Carol={}", t3_alice, t3_bob, t3_carol);
    println!("T3's SUM = {} (expected {})", t3_sum, expected_sum);
    assert_eq!(t3_sum, expected_sum);
    println!("CORRECT! T3 sees T2's committed writes\n");

    // Demonstrate version chains
    println!("=== Version Chain for 'Alice' ===");
    if let Some(versioned) = db.data.get("Alice") {
        for (i, version) in versioned.versions.iter().enumerate() {
            let committed = if db.committed.contains(&version.created_by) {
                "committed"
            } else {
                "active"
            };
            println!("  Version {}: value={:?}, created_by=txn{}, {}",
                     i, version.value, version.created_by, committed);
        }
    }

    println!("\nKey insight: readers NEVER blocked writers.");
    println!("T1's 30-second scan did not prevent T2's transfer.");
    println!("T2's transfer did not corrupt T1's sum.");
    println!("Both transactions completed correctly, concurrently.");
}
```

The analytics query sees a consistent snapshot from its start time. The transfer completes without waiting. Neither blocks the other. Both get correct results. This is the magic of snapshot isolation.

---

## Complexity Table

| Operation | Global Lock | Snapshot Isolation (MVCC) | Notes |
|-----------|------------|--------------------------|-------|
| Read | O(1) but blocks writers | O(v) version chain scan | v = versions per key (typically 1-3) |
| Write | O(1) but blocks readers | O(1) append new version | Old versions retained |
| Read-write concurrency | Serialized | Fully concurrent | Readers never block writers |
| Write-write conflict | Serialized (no conflict) | Detected at commit time | First committer wins |
| Space overhead | None | O(k * v) | k = keys, v = avg versions |
| Garbage collection | Not needed | O(n) periodic | Remove old versions |
| Consistency level | Serializable | Snapshot isolation | SI allows write skew anomaly |
| Implementation complexity | Trivial | Moderate | Version chains, visibility rules |

Snapshot isolation is slightly weaker than full serializability. It prevents most anomalies (dirty reads, non-repeatable reads, phantom reads) but allows **write skew**: two transactions read overlapping data, make decisions based on those reads, and write to different keys. The combined result is inconsistent even though neither transaction saw the other's writes. PostgreSQL calls its strongest level "serializable," and it extends snapshot isolation with additional checks to prevent write skew.

---

## Where This Shows Up in Our Database

In Chapter 16, we add transaction support to our database using MVCC:

```rust,ignore
pub struct MvccStorage {
    versions: HashMap<String, Vec<Version>>,
    txn_manager: TransactionManager,
}

impl MvccStorage {
    pub fn begin(&mut self) -> Transaction {
        // Assign a read timestamp
        // ...
    }

    pub fn get(&self, txn: &Transaction, key: &str) -> Option<Vec<u8>> {
        // Walk the version chain, find the newest visible version
        // ...
    }

    pub fn set(&mut self, txn: &mut Transaction, key: &str, value: Vec<u8>) {
        // Create a new version tagged with this transaction's ID
        // ...
    }
}
```

Beyond our toydb, MVCC and snapshot isolation are the dominant concurrency control mechanism in production databases:

- **PostgreSQL** implements MVCC by storing multiple row versions in the heap. Each row has `xmin` (creating transaction) and `xmax` (deleting transaction) fields. The visibility check compares these against the reader's snapshot. VACUUM garbage-collects dead row versions.
- **Oracle** stores undo records in rollback segments. When a reader needs an old version, the database reconstructs it by applying undo records backward from the current version. This is "undo-based MVCC" versus PostgreSQL's "append-based MVCC."
- **MySQL/InnoDB** uses a combination: the clustered index stores the current version, and undo logs store previous versions. Readers reconstruct old versions from undo logs on the fly.
- **CockroachDB** implements MVCC at the key-value layer. Each key has a timestamp suffix, and multiple versions coexist in the storage engine. The MVCC layer provides snapshot isolation, and an additional serializable snapshot isolation (SSI) layer detects write skew.
- **SQLite** takes a simpler approach: it uses a write-ahead log (WAL) where readers see the database as of the start of their transaction, and writers append to the WAL. This gives snapshot isolation semantics without version chains per row.

The principle is universal: instead of making transactions wait for each other, give each transaction its own view of the database. The cost is extra storage for old versions and the complexity of visibility checks. The payoff is that readers and writers can proceed concurrently, which is the foundation of every high-throughput database system.

---

## Try It Yourself

### Exercise 1: Write Skew Detection

Snapshot isolation allows a "write skew" anomaly. Example: two doctors are on call. The rule is "at least one doctor must be on call." Both check: "is the other doctor on call? Yes." Both decide to go off call. Now nobody is on call. Implement a check that detects this write skew at commit time by verifying that the transaction's read set has not been modified by another committed transaction.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct Version {
    created_by: u64,
    value: String,
}

struct MvccDb {
    data: HashMap<String, Vec<Version>>,
    next_id: u64,
    committed: HashSet<u64>,
}

struct Txn {
    id: u64,
    read_ts: u64,
    read_set: HashSet<String>,   // keys we read
    write_set: HashMap<String, String>, // keys we wrote
}

impl MvccDb {
    fn new() -> Self {
        MvccDb { data: HashMap::new(), next_id: 1, committed: HashSet::new() }
    }

    fn setup(&mut self, key: &str, value: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.data.entry(key.to_string())
            .or_insert_with(Vec::new)
            .insert(0, Version { created_by: id, value: value.to_string() });
        self.committed.insert(id);
    }

    fn begin(&mut self) -> Txn {
        let id = self.next_id;
        self.next_id += 1;
        Txn { id, read_ts: id, read_set: HashSet::new(), write_set: HashMap::new() }
    }

    fn read(&self, txn: &mut Txn, key: &str) -> Option<String> {
        txn.read_set.insert(key.to_string());
        if let Some(versions) = self.data.get(key) {
            for v in versions {
                if v.created_by == txn.id {
                    return Some(v.value.clone());
                }
                if v.created_by <= txn.read_ts && self.committed.contains(&v.created_by) {
                    return Some(v.value.clone());
                }
            }
        }
        None
    }

    fn write(&mut self, txn: &mut Txn, key: &str, value: &str) {
        txn.write_set.insert(key.to_string(), value.to_string());
        self.data.entry(key.to_string())
            .or_insert_with(Vec::new)
            .insert(0, Version { created_by: txn.id, value: value.to_string() });
    }

    /// Commit with Serializable Snapshot Isolation (SSI) check.
    /// Detects write skew by checking if any key in the read set
    /// was modified by another transaction that committed after we started.
    fn commit_ssi(&mut self, txn: Txn) -> Result<(), String> {
        // Check: was anything we READ modified by a concurrent committed txn?
        for key in &txn.read_set {
            if let Some(versions) = self.data.get(key) {
                for v in versions {
                    if v.created_by != txn.id
                        && v.created_by > txn.read_ts
                        && self.committed.contains(&v.created_by)
                    {
                        return Err(format!(
                            "write skew detected: key '{}' was modified by txn {} after our start",
                            key, v.created_by
                        ));
                    }
                }
            }
        }

        self.committed.insert(txn.id);
        Ok(())
    }

    fn commit_si(&mut self, txn: &Txn) {
        self.committed.insert(txn.id);
    }
}

fn main() {
    println!("=== Write Skew Detection ===\n");

    // Scenario: Two doctors on call
    let mut db = MvccDb::new();
    db.setup("doctor_alice", "on_call");
    db.setup("doctor_bob", "on_call");

    // Without SSI: write skew allowed
    println!("--- Without SSI (plain Snapshot Isolation) ---");
    {
        let mut db = MvccDb::new();
        db.setup("doctor_alice", "on_call");
        db.setup("doctor_bob", "on_call");

        let mut t1 = db.begin();
        let mut t2 = db.begin();

        // T1: "Is Bob on call? Yes, so I can go off call."
        let bob = db.read(&mut t1, "doctor_bob").unwrap();
        println!("T1 reads doctor_bob: {}", bob);
        db.write(&mut t1, "doctor_alice", "off_call");
        println!("T1 writes doctor_alice = off_call");

        // T2: "Is Alice on call? Yes, so I can go off call."
        let alice = db.read(&mut t2, "doctor_alice").unwrap();
        println!("T2 reads doctor_alice: {} (snapshot: still on_call!)", alice);
        db.write(&mut t2, "doctor_bob", "off_call");
        println!("T2 writes doctor_bob = off_call");

        db.commit_si(&t1);
        db.commit_si(&t2);
        println!("Both committed under SI: NOBODY IS ON CALL! (write skew)\n");
    }

    // With SSI: write skew detected
    println!("--- With SSI (Serializable Snapshot Isolation) ---");
    {
        let mut db = MvccDb::new();
        db.setup("doctor_alice", "on_call");
        db.setup("doctor_bob", "on_call");

        let mut t1 = db.begin();
        let mut t2 = db.begin();

        let bob = db.read(&mut t1, "doctor_bob").unwrap();
        println!("T1 reads doctor_bob: {}", bob);
        db.write(&mut t1, "doctor_alice", "off_call");

        let alice = db.read(&mut t2, "doctor_alice").unwrap();
        println!("T2 reads doctor_alice: {}", alice);
        db.write(&mut t2, "doctor_bob", "off_call");

        // T1 commits first -- succeeds
        match db.commit_ssi(t1) {
            Ok(()) => println!("T1 commits: OK"),
            Err(e) => println!("T1 aborted: {}", e),
        }

        // T2 tries to commit -- SSI detects that doctor_alice (which T2 read)
        // was modified by T1 (which committed after T2 started)
        match db.commit_ssi(t2) {
            Ok(()) => println!("T2 commits: OK (BUG!)"),
            Err(e) => println!("T2 aborted: {}", e),
        }

        println!("\nSSI prevented the write skew. At least one doctor stays on call.");
    }
}
```

</details>

### Exercise 2: Version Garbage Collection

Implement a garbage collector that removes old versions that no active transaction could still need. Track the minimum read timestamp across all active transactions. Any version older than the newest version visible to that minimum timestamp can be safely removed.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct Version {
    created_by: u64,
    value: i64,
}

struct MvccDb {
    data: HashMap<String, Vec<Version>>, // newest version first
    next_id: u64,
    active_txns: HashSet<u64>,
    committed: HashSet<u64>,
}

impl MvccDb {
    fn new() -> Self {
        MvccDb {
            data: HashMap::new(),
            next_id: 1,
            active_txns: HashSet::new(),
            committed: HashSet::new(),
        }
    }

    fn write_committed(&mut self, key: &str, value: i64) {
        let id = self.next_id;
        self.next_id += 1;
        self.data.entry(key.to_string())
            .or_insert_with(Vec::new)
            .insert(0, Version { created_by: id, value });
        self.committed.insert(id);
    }

    fn begin(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.active_txns.insert(id);
        id
    }

    fn commit(&mut self, txn_id: u64) {
        self.active_txns.remove(&txn_id);
        self.committed.insert(txn_id);
    }

    fn count_versions(&self) -> usize {
        self.data.values().map(|v| v.len()).sum()
    }

    /// Garbage collect old versions.
    /// The low-water mark is the minimum read timestamp among all active transactions.
    /// For each key, keep:
    /// - All versions newer than the low-water mark (active txns might need them)
    /// - The newest version at or below the low-water mark (the "visible" version)
    /// - Remove everything older than that
    fn garbage_collect(&mut self) -> usize {
        let low_water_mark = self.active_txns.iter().min().copied()
            .unwrap_or(self.next_id); // if no active txns, everything can be cleaned

        let mut removed = 0;

        for versions in self.data.values_mut() {
            if versions.len() <= 1 {
                continue;
            }

            // Find the boundary: the first committed version at or below the low-water mark
            let mut keep_up_to = versions.len(); // keep everything by default
            let mut found_visible = false;

            for (i, version) in versions.iter().enumerate() {
                if !found_visible
                    && version.created_by <= low_water_mark
                    && self.committed.contains(&version.created_by)
                {
                    found_visible = true;
                    keep_up_to = i + 1; // keep this version, remove everything after
                }
            }

            if found_visible && keep_up_to < versions.len() {
                let to_remove = versions.len() - keep_up_to;
                versions.truncate(keep_up_to);
                removed += to_remove;
            }
        }

        removed
    }
}

fn main() {
    let mut db = MvccDb::new();

    // Create many versions of the same keys
    println!("=== Version Garbage Collection ===\n");

    for i in 0..10 {
        db.write_committed("counter", i);
        db.write_committed("name", i * 100);
    }

    println!("After 10 updates each to 'counter' and 'name':");
    println!("  Total versions: {}", db.count_versions());

    // Start a long-running transaction at this point
    let long_txn = db.begin();
    println!("\nStarted long-running transaction (id={})", long_txn);

    // More updates happen while the long transaction is active
    for i in 10..20 {
        db.write_committed("counter", i);
        db.write_committed("name", i * 100);
    }
    println!("After 10 more updates:");
    println!("  Total versions: {}", db.count_versions());

    // GC while the long transaction is active
    let removed = db.garbage_collect();
    println!("\nGC with active transaction (low-water mark = {}):", long_txn);
    println!("  Removed: {} versions", removed);
    println!("  Remaining: {} versions", db.count_versions());
    println!("  (Must keep versions visible to the long-running transaction)");

    // Complete the long transaction
    db.commit(long_txn);
    println!("\nLong-running transaction committed.");

    // GC again -- now we can clean up more
    let removed = db.garbage_collect();
    println!("GC after long transaction completed:");
    println!("  Removed: {} additional versions", removed);
    println!("  Remaining: {} versions", db.count_versions());
    println!("  (Only latest committed versions remain)");

    // Show final version chains
    println!("\nFinal version chains:");
    for (key, versions) in &db.data {
        println!("  {}: {} version(s)", key, versions.len());
        for v in versions {
            println!("    txn={}, value={}", v.created_by, v.value);
        }
    }
}
```

</details>

### Exercise 3: Read-Only Transaction Optimization

Read-only transactions do not write anything, so they can never conflict with other transactions. They also do not need to appear in the active transaction set (because they create no versions that other transactions need to track). Implement a `begin_readonly()` method that returns a lightweight read-only transaction that skips conflict checking and does not appear in the active set. This allows garbage collection to proceed without waiting for long-running read-only queries.

<details>
<summary>Solution</summary>

```rust
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct Version {
    created_by: u64,
    value: i64,
}

enum TxnType {
    ReadWrite { writes: Vec<(String, i64)> },
    ReadOnly,
}

struct Transaction {
    id: u64,
    read_ts: u64,
    txn_type: TxnType,
}

struct MvccDb {
    data: HashMap<String, Vec<Version>>,
    next_id: u64,
    active_rw_txns: HashSet<u64>, // only read-write transactions
    committed: HashSet<u64>,
}

impl MvccDb {
    fn new() -> Self {
        MvccDb {
            data: HashMap::new(),
            next_id: 1,
            active_rw_txns: HashSet::new(),
            committed: HashSet::new(),
        }
    }

    fn write_initial(&mut self, key: &str, value: i64) {
        let id = self.next_id;
        self.next_id += 1;
        self.data.entry(key.to_string())
            .or_insert_with(Vec::new)
            .insert(0, Version { created_by: id, value });
        self.committed.insert(id);
    }

    /// Begin a read-write transaction.
    /// Added to the active set (affects GC low-water mark).
    fn begin_rw(&mut self) -> Transaction {
        let id = self.next_id;
        self.next_id += 1;
        self.active_rw_txns.insert(id);
        Transaction {
            id,
            read_ts: id,
            txn_type: TxnType::ReadWrite { writes: Vec::new() },
        }
    }

    /// Begin a read-only transaction.
    /// NOT added to the active set -- does not block GC.
    fn begin_readonly(&mut self) -> Transaction {
        let id = self.next_id;
        self.next_id += 1;
        // Note: NOT inserted into active_rw_txns
        Transaction {
            id,
            read_ts: id,
            txn_type: TxnType::ReadOnly,
        }
    }

    fn read(&self, txn: &Transaction, key: &str) -> Option<i64> {
        if let Some(versions) = self.data.get(key) {
            for v in versions {
                if v.created_by == txn.id {
                    return Some(v.value);
                }
                if v.created_by <= txn.read_ts && self.committed.contains(&v.created_by) {
                    return Some(v.value);
                }
            }
        }
        None
    }

    fn write(&mut self, txn: &mut Transaction, key: &str, value: i64) -> Result<(), &'static str> {
        match &mut txn.txn_type {
            TxnType::ReadOnly => Err("cannot write in a read-only transaction"),
            TxnType::ReadWrite { writes } => {
                writes.push((key.to_string(), value));
                self.data.entry(key.to_string())
                    .or_insert_with(Vec::new)
                    .insert(0, Version { created_by: txn.id, value });
                Ok(())
            }
        }
    }

    fn commit(&mut self, txn: Transaction) {
        match &txn.txn_type {
            TxnType::ReadWrite { .. } => {
                self.active_rw_txns.remove(&txn.id);
                self.committed.insert(txn.id);
            }
            TxnType::ReadOnly => {
                // Nothing to do -- read-only txns have no side effects
                // and were never in the active set
            }
        }
    }

    fn gc_low_water_mark(&self) -> u64 {
        // Only read-write transactions affect the low-water mark
        self.active_rw_txns.iter().min().copied()
            .unwrap_or(self.next_id)
    }

    fn count_versions(&self) -> usize {
        self.data.values().map(|v| v.len()).sum()
    }

    fn garbage_collect(&mut self) -> usize {
        let lwm = self.gc_low_water_mark();
        let mut removed = 0;

        for versions in self.data.values_mut() {
            if versions.len() <= 1 { continue; }
            let mut found_visible = false;
            let mut keep_up_to = versions.len();

            for (i, version) in versions.iter().enumerate() {
                if !found_visible
                    && version.created_by <= lwm
                    && self.committed.contains(&version.created_by)
                {
                    found_visible = true;
                    keep_up_to = i + 1;
                }
            }

            if found_visible && keep_up_to < versions.len() {
                removed += versions.len() - keep_up_to;
                versions.truncate(keep_up_to);
            }
        }
        removed
    }
}

fn main() {
    let mut db = MvccDb::new();

    // Setup
    for i in 0..5 {
        db.write_initial(&format!("key{}", i), i * 100);
    }

    // Create many versions
    for round in 1..=5 {
        for i in 0..5 {
            let mut txn = db.begin_rw();
            db.write(&mut txn, &format!("key{}", i), round * 100 + i).unwrap();
            db.commit(txn);
        }
    }

    println!("=== Read-Only Transaction Optimization ===\n");
    println!("Total versions: {}", db.count_versions());

    // Start a long-running READ-ONLY transaction
    let ro_txn = db.begin_readonly();
    println!("Started read-only transaction (id={})", ro_txn.id);
    println!("Active RW transactions: {:?}", db.active_rw_txns);
    println!("GC low-water mark: {} (read-only txn does NOT affect it)",
             db.gc_low_water_mark());

    // More writes happen
    for i in 0..5 {
        let mut txn = db.begin_rw();
        db.write(&mut txn, &format!("key{}", i), 999 + i).unwrap();
        db.commit(txn);
    }

    // GC can proceed because the read-only txn is not in the active set
    let removed = db.garbage_collect();
    println!("\nGC while read-only txn is active:");
    println!("  Removed: {} versions", removed);
    println!("  Remaining: {}", db.count_versions());

    // The read-only transaction can still read its snapshot
    println!("\nRead-only transaction reads (snapshot from id={}):", ro_txn.read_ts);
    for i in 0..5 {
        let key = format!("key{}", i);
        let value = db.read(&ro_txn, &key);
        println!("  {} = {:?}", key, value);
    }

    // Try to write in a read-only transaction
    let mut ro_txn2 = db.begin_readonly();
    match db.write(&mut ro_txn2, "key0", 42) {
        Ok(()) => println!("\nBUG: read-only txn allowed a write!"),
        Err(e) => println!("\nCorrectly rejected write in read-only txn: {}", e),
    }

    db.commit(ro_txn);
    println!("\nRead-only transaction committed (no-op).");
    println!("No locks were held. No GC was delayed. No conflicts possible.");
}
```

</details>

---

## Recap

Snapshot isolation gives each transaction a frozen view of the database at its start time. Readers see committed data from before they began, regardless of what happens afterward. Writers create new versions of rows without disturbing old versions that concurrent readers might need. Readers never block writers. Writers never block readers.

The implementation is MVCC -- multi-version concurrency control. Each row has a chain of versions, tagged with the transaction that created them. The visibility rule is simple: a version is visible if its creator committed before the reader's snapshot timestamp. This single rule, applied consistently, prevents dirty reads, non-repeatable reads, and phantom reads.

The costs are real: extra storage for old versions, the complexity of version chain traversal, and the need for garbage collection. But the payoff -- true concurrent read-write access without locking -- is why MVCC is the concurrency control mechanism of choice for PostgreSQL, Oracle, MySQL, CockroachDB, and virtually every modern database. When you run a long report against a production database without blocking any transactions, snapshot isolation is the reason it works.
