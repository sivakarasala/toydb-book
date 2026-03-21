/// Chapter 5: MVCC — SOLUTION

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionedKey {
    pub key: String,
    pub version: u64,
}

pub struct MvccStore {
    data: BTreeMap<VersionedKey, Option<Value>>,
    next_version: u64,
}

impl MvccStore {
    pub fn new() -> Self {
        MvccStore {
            data: BTreeMap::new(),
            next_version: 1,
        }
    }

    pub fn begin(&mut self) -> Transaction {
        let version = self.next_version;
        self.next_version += 1;
        Transaction {
            read_version: version,
            write_version: version,
            writes: Vec::new(),
        }
    }

    pub fn commit(&mut self, txn: Transaction) {
        for (key, value) in txn.writes {
            self.data.insert(
                VersionedKey {
                    key,
                    version: txn.write_version,
                },
                value,
            );
        }
    }
}

pub struct Transaction {
    pub read_version: u64,
    pub write_version: u64,
    writes: Vec<(String, Option<Value>)>,
}

impl Transaction {
    pub fn get<'a>(&self, store: &'a MvccStore, key: &str) -> Option<&'a Value> {
        // Find the latest version of this key that is <= read_version
        let start = VersionedKey {
            key: key.to_string(),
            version: 0,
        };
        let end = VersionedKey {
            key: key.to_string(),
            version: self.read_version,
        };

        store
            .data
            .range(start..=end)
            .rev()
            .next()
            .and_then(|(vk, val)| {
                if vk.key == key {
                    val.as_ref()
                } else {
                    None
                }
            })
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.writes.push((key.to_string(), Some(value)));
    }

    pub fn delete(&mut self, key: &str) {
        self.writes.push((key.to_string(), None));
    }
}

fn main() {
    println!("=== Chapter 5: MVCC — Solution ===");
    let mut store = MvccStore::new();

    let mut t1 = store.begin();
    t1.set("account", Value::Int(1000));
    store.commit(t1);
    println!("Wrote account=1000 at v1");

    let reader = store.begin();

    let mut t2 = store.begin();
    t2.set("account", Value::Int(900));
    store.commit(t2);
    println!("Wrote account=900 at v3");

    println!(
        "Reader (v2) sees: {:?}",
        reader.get(&store, "account")
    );

    let latest = store.begin();
    println!(
        "Latest (v4) sees: {:?}",
        latest.get(&store, "account")
    );
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
        assert_eq!(txn2.get(&store, "name"), Some(&Value::String("Alice".into())));
    }

    #[test]
    fn test_snapshot_isolation() {
        let mut store = MvccStore::new();
        let mut txn1 = store.begin();
        txn1.set("name", Value::String("Alice".into()));
        store.commit(txn1);

        let txn2 = store.begin();

        let mut txn3 = store.begin();
        txn3.set("name", Value::String("Bob".into()));
        store.commit(txn3);

        assert_eq!(txn2.get(&store, "name"), Some(&Value::String("Alice".into())));

        let txn4 = store.begin();
        assert_eq!(txn4.get(&store, "name"), Some(&Value::String("Bob".into())));
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
