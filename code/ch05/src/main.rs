/// Chapter 5: MVCC — Multi-Version Concurrency Control
/// Exercise: Build snapshot isolation with versioned keys.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    String(String),
}

/// A versioned key: (key, version) pair for MVCC.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionedKey {
    pub key: String,
    pub version: u64,
}

/// The MVCC store: stores all versions of all keys.
pub struct MvccStore {
    data: BTreeMap<VersionedKey, Option<Value>>, // None = deleted at this version
    next_version: u64,
}

impl MvccStore {
    pub fn new() -> Self {
        MvccStore {
            data: BTreeMap::new(),
            next_version: 1,
        }
    }

    /// Begin a new transaction at the current version.
    pub fn begin(&mut self) -> Transaction {
        // TODO: Create a Transaction with the current next_version as its read_version
        // Then increment next_version
        todo!("Implement begin")
    }

    /// Commit a transaction's writes into the store.
    pub fn commit(&mut self, txn: Transaction) {
        // TODO: For each (key, value) in txn.writes, insert into self.data
        // using VersionedKey { key, version: txn.write_version }
        todo!("Implement commit")
    }
}

/// A transaction with snapshot isolation.
pub struct Transaction {
    pub read_version: u64,
    pub write_version: u64,
    writes: Vec<(String, Option<Value>)>,
}

impl Transaction {
    /// Read a key, seeing only versions <= read_version.
    pub fn get<'a>(&self, store: &'a MvccStore, key: &str) -> Option<&'a Value> {
        // TODO: Scan store.data for entries with this key and version <= self.read_version
        // Return the value from the highest version <= read_version
        // If it's None (tombstone), return None
        // Hint: iterate store.data.range(..) in reverse
        todo!("Implement get")
    }

    /// Buffer a write (will be applied on commit).
    pub fn set(&mut self, key: &str, value: Value) {
        // TODO: Add (key, Some(value)) to self.writes
        todo!("Implement set")
    }

    /// Buffer a delete.
    pub fn delete(&mut self, key: &str) {
        // TODO: Add (key, None) to self.writes
        todo!("Implement delete")
    }
}

fn main() {
    println!("=== Chapter 5: MVCC ===");
    println!("Exercise: Implement snapshot isolation with versioned keys.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_read_write() {
        let mut store = MvccStore::new();
        let mut txn = store.begin();
        txn.set("name", Value::String("Alice".into()));
        store.commit(txn);

        let txn2 = store.begin();
        assert_eq!(
            txn2.get(&store, "name"),
            Some(&Value::String("Alice".into()))
        );
    }

    #[test]
    fn test_snapshot_isolation() {
        let mut store = MvccStore::new();

        // txn1 writes name=Alice
        let mut txn1 = store.begin();
        txn1.set("name", Value::String("Alice".into()));
        store.commit(txn1);

        // txn2 starts (sees Alice)
        let txn2 = store.begin();

        // txn3 writes name=Bob and commits
        let mut txn3 = store.begin();
        txn3.set("name", Value::String("Bob".into()));
        store.commit(txn3);

        // txn2 should still see Alice (snapshot isolation)
        assert_eq!(
            txn2.get(&store, "name"),
            Some(&Value::String("Alice".into()))
        );

        // A new txn should see Bob
        let txn4 = store.begin();
        assert_eq!(
            txn4.get(&store, "name"),
            Some(&Value::String("Bob".into()))
        );
    }

    #[test]
    fn test_delete() {
        let mut store = MvccStore::new();
        let mut txn = store.begin();
        txn.set("k", Value::Int(1));
        store.commit(txn);

        let mut txn2 = store.begin();
        txn2.delete("k");
        store.commit(txn2);

        let txn3 = store.begin();
        assert_eq!(txn3.get(&store, "k"), None);
    }
}
