/// In-Memory Storage Engine (Ch1-2)
///
/// Uses a BTreeMap for ordered key-value storage.
/// This is the default engine — fast, no persistence.

use std::collections::BTreeMap;
use crate::error::Result;
use super::Storage;

pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage { data: BTreeMap::new() }
    }
}

impl Storage for MemoryStorage {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.data.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<()> {
        self.data.insert(key.to_string(), value);
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<bool> {
        Ok(self.data.remove(key).is_some())
    }

    fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        Ok(self.data.range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_set_delete() {
        let mut s = MemoryStorage::new();
        assert_eq!(s.get("x").unwrap(), None);
        s.set("x", b"hello".to_vec()).unwrap();
        assert_eq!(s.get("x").unwrap(), Some(b"hello".to_vec()));
        assert!(s.delete("x").unwrap());
        assert_eq!(s.get("x").unwrap(), None);
    }

    #[test]
    fn test_scan_prefix() {
        let mut s = MemoryStorage::new();
        s.set("user:1", b"Alice".to_vec()).unwrap();
        s.set("user:2", b"Bob".to_vec()).unwrap();
        s.set("item:1", b"Widget".to_vec()).unwrap();
        let users = s.scan_prefix("user:").unwrap();
        assert_eq!(users.len(), 2);
    }
}
