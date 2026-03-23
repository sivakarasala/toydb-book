## Exercise 1: Define the Storage Trait

**Goal:** Define the `Storage` trait with four methods and a custom `Error` enum that every storage engine in our database will implement.

### Step 1: Create the project

If you have not already, create a new Rust project for the database:

```bash
cargo new toydb
cd toydb
```

### Step 2: Define the error type

Create a new file `src/error.rs`:

```rust
use std::fmt;

#[derive(Debug, Clone)]
pub enum Error {
    /// A value was not found for the given key.
    NotFound(String),
    /// An internal storage error occurred.
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(key) => write!(f, "key not found: {}", key),
            Error::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}
```

The `Error` enum has two variants. `NotFound` carries the key that was missing. `Internal` carries a description of what went wrong. We implement `Display` so the error can be printed with `{}` formatting — this is a Rust convention for user-facing error messages, as opposed to `Debug` which is for developer-facing output.

Why an enum instead of a string? Because callers can `match` on the variant:

```rust
match store.get("users:1") {
    Err(Error::NotFound(_)) => println!("user does not exist"),
    Err(Error::Internal(msg)) => panic!("storage is broken: {}", msg),
    Ok(value) => { /* use the value */ }
}
```

A string error gives you one option: read the message and hope you parsed it right. An enum error gives you structured control flow. The compiler ensures you handle each variant.

### Step 3: Define the Storage trait

Create a new file `src/storage.rs`:

```rust
use std::ops::RangeBounds;
use crate::error::Error;

/// The storage engine interface.
///
/// Every backend (in-memory, on-disk, distributed) implements this trait.
/// The database layer calls these methods without knowing which backend is active.
pub trait Storage {
    /// Store a key-value pair. Overwrites any existing value for the key.
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;

    /// Retrieve the value for a key. Returns `None` if the key does not exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Delete a key-value pair. Returns `Ok(())` even if the key did not exist.
    fn delete(&mut self, key: &str) -> Result<(), Error>;

    /// Scan all key-value pairs whose keys fall within the given range.
    /// Results are returned in key order.
    fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error>;
}
```

### Step 4: Register the modules

Open `src/main.rs` and add the module declarations:

```rust
mod error;
mod storage;

fn main() {
    println!("toydb starting...");
}
```

Run `cargo build` to verify everything compiles. You have not implemented anything yet — you have only defined the contract.

### Why `Vec<u8>` instead of `String`?

Storage engines are type-agnostic. The engine does not know whether the value is a JSON document, a serialized struct, a JPEG image, or a raw integer. It stores bytes and returns bytes. The layer above the storage engine (the database, the SQL engine) is responsible for interpreting those bytes.

`Vec<u8>` is Rust's owning byte buffer — a growable array of raw bytes. `String` is a `Vec<u8>` that guarantees valid UTF-8. By using `Vec<u8>`, we make no assumptions about the content. This is the same design choice made by RocksDB, LevelDB, and every serious storage engine: keys and values are byte strings.

### Why `Result<T, Error>` everywhere?

Every method returns a `Result`. Even `set()` on an in-memory map cannot fail today, but it will fail when the engine is backed by a disk (out of space), a network (connection dropped), or a transaction (write conflict). By making the return type `Result` from the start, every implementation uses the same signature. Callers always handle the possibility of failure, even when the current implementation never fails. This is designing for the future without overengineering the present.

<details>
<summary>Hint: If you see "unused import" warnings</summary>

The compiler warns about `RangeBounds` and `Error` because nothing uses them yet. These warnings will disappear once Exercise 2 implements the trait. You can suppress them temporarily with `#[allow(unused_imports)]` at the top of the file, but they will resolve naturally as you continue.

</details>

---

## Exercise 2: Implement MemoryStorage

**Goal:** Build a `MemoryStorage` struct backed by `BTreeMap<String, Vec<u8>>` that implements all four methods of the `Storage` trait.

### Step 1: Create the memory storage module

Create `src/memory.rs`:

```rust
use std::collections::BTreeMap;
use std::ops::RangeBounds;
use crate::error::Error;
use crate::storage::Storage;

/// An in-memory storage engine backed by a BTreeMap.
///
/// Keys are stored in sorted order, which makes range scans efficient.
/// All data is lost when the process exits.
pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage {
            data: BTreeMap::new(),
        }
    }
}

impl Storage for MemoryStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error> {
        self.data.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.data.get(key).cloned())
    }

    fn delete(&mut self, key: &str) -> Result<(), Error> {
        self.data.remove(key);
        Ok(())
    }

    fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error> {
        let results: Vec<(String, Vec<u8>)> = self
            .data
            .range(range)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}
```

### Step 2: Register the module

Update `src/main.rs`:

```rust
mod error;
mod memory;
mod storage;

fn main() {
    println!("toydb starting...");
}
```

Run `cargo build`. It should compile without errors.

### Why BTreeMap instead of HashMap?

This is a critical design decision. Let us compare:

| | `HashMap` | `BTreeMap` |
|---|-----------|-----------|
| **Ordering** | No guaranteed order | Keys sorted in order |
| **Get/Set** | O(1) average | O(log n) |
| **Range scan** | Impossible without sorting | Native `.range()` method |
| **Memory** | Hash table (array + linked lists) | Balanced tree (nodes with children) |

A `HashMap` is faster for point lookups — O(1) versus O(log n). But a database needs range scans: "give me all users with IDs between 100 and 200" or "give me all keys starting with `users:`". A `HashMap` stores keys in arbitrary order, so a range scan requires copying all entries, sorting them, and filtering. A `BTreeMap` stores keys in sorted order by default, and its `.range()` method returns an iterator over the matching entries in O(log n + k) time, where k is the number of results.

Real databases use B-trees (or variants like B+ trees, LSM-trees) for exactly this reason. Our `BTreeMap` is Rust's standard library B-tree — it is the right foundation.

### Understanding the scan implementation

```rust
fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let results: Vec<(String, Vec<u8>)> = self
        .data
        .range(range)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(results)
}
```

Let us trace each step:

1. **`self.data.range(range)`** — returns an iterator over `(&String, &Vec<u8>)` pairs whose keys fall within the range. The `BTreeMap` walks its internal tree structure to find the start of the range, then iterates forward until the end.

2. **`.map(|(k, v)| (k.clone(), v.clone()))`** — the range iterator yields *references* to the map's data. We clone them to produce owned values. This is necessary because we are returning `Vec<(String, Vec<u8>)>` — owned data — not references tied to the lifetime of the map.

3. **`.collect()`** — gathers the iterator into a `Vec`.

The `impl RangeBounds<String>` parameter accepts any range syntax. Callers can write:

```rust
// All keys from "a" to "d" (inclusive start, exclusive end)
store.scan("a".to_string().."d".to_string())?;

// All keys from "user:" onwards
store.scan("user:".to_string()..)?;

// All keys (full scan)
store.scan(..)?;
```

This flexibility comes from Rust's range syntax and the `RangeBounds` trait. The `..` operator creates `Range`, `RangeFrom`, `RangeTo`, or `RangeFull` structs, all of which implement `RangeBounds`. One parameter, many calling conventions.

<details>
<summary>Hint: If you see "trait bound not satisfied" for range</summary>

Make sure you are creating `String` ranges, not `&str` ranges. The `BTreeMap<String, Vec<u8>>` expects range bounds over `String`. Use `.to_string()` on your range endpoints:

```rust
// This works:
store.scan("a".to_string().."z".to_string())?;

// This does NOT work:
store.scan("a".."z")?;
```

The reason: `BTreeMap::range()` requires the range bounds to match the key type. Our keys are `String`, so the bounds must be `String` too.

</details>

---

## Exercise 3: Write a Generic Database

**Goal:** Build a `Database<S: Storage>` struct that delegates to any storage engine. The database does not know or care whether its data lives in memory, on disk, or across the network.

### Step 1: Create the database module

Create `src/database.rs`:

```rust
use std::ops::RangeBounds;
use crate::error::Error;
use crate::storage::Storage;

/// A key-value database that delegates to a pluggable storage engine.
///
/// The type parameter `S` can be any type that implements `Storage`.
/// This means the same `Database` code works with `MemoryStorage`,
/// `DiskStorage`, or any future engine you build.
pub struct Database<S: Storage> {
    storage: S,
}

impl<S: Storage> Database<S> {
    /// Create a new database with the given storage engine.
    pub fn new(storage: S) -> Self {
        Database { storage }
    }

    /// Store a key-value pair.
    pub fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Error> {
        self.storage.set(key.to_string(), value)
    }

    /// Retrieve a value by key. Returns `None` if the key does not exist.
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        self.storage.get(key)
    }

    /// Delete a key.
    pub fn delete(&mut self, key: &str) -> Result<(), Error> {
        self.storage.delete(key)
    }

    /// Scan a range of keys in sorted order.
    pub fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error> {
        self.storage.scan(range)
    }

    /// Convenience: store a string value (converts to bytes internally).
    pub fn set_string(&mut self, key: &str, value: &str) -> Result<(), Error> {
        self.set(key, value.as_bytes().to_vec())
    }

    /// Convenience: retrieve a value as a UTF-8 string.
    /// Returns an error if the value is not valid UTF-8.
    pub fn get_string(&self, key: &str) -> Result<Option<String>, Error> {
        match self.get(key)? {
            Some(bytes) => {
                let s = String::from_utf8(bytes)
                    .map_err(|e| Error::Internal(format!("invalid UTF-8: {}", e)))?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }
}
```

### Step 2: Register the module and use it

Update `src/main.rs`:

```rust
mod database;
mod error;
mod memory;
mod storage;

use database::Database;
use memory::MemoryStorage;

fn main() {
    let storage = MemoryStorage::new();
    let mut db = Database::new(storage);

    // Store some data
    db.set_string("name", "toydb").unwrap();
    db.set_string("version", "0.1.0").unwrap();
    db.set_string("author", "you").unwrap();

    // Retrieve it
    match db.get_string("name") {
        Ok(Some(value)) => println!("name = {}", value),
        Ok(None) => println!("name not found"),
        Err(e) => println!("error: {}", e),
    }

    // Scan all keys
    match db.scan(..) {
        Ok(entries) => {
            println!("\nAll entries:");
            for (key, value) in entries {
                let v = String::from_utf8_lossy(&value);
                println!("  {} = {}", key, v);
            }
        }
        Err(e) => println!("scan error: {}", e),
    }
}
```

Run it:

```bash
cargo run
```

Expected output:

```
name = toydb

All entries:
  author = you
  name = toydb
  version = 0.1.0
```

Notice the scan output is in alphabetical order — `author`, `name`, `version` — even though we inserted `name` first. The `BTreeMap` sorts keys automatically. This is not a coincidence; it is the whole point of choosing a B-tree.

### Understanding the generic architecture

Look at this line:

```rust
pub struct Database<S: Storage> {
    storage: S,
}
```

`S` is a type parameter. `S: Storage` is a trait bound. Together they say: "Database holds a storage engine of some type S, and S must implement the Storage trait." When you write `Database::new(MemoryStorage::new())`, the compiler substitutes `S = MemoryStorage` and generates a concrete `Database<MemoryStorage>` type. There is no vtable, no dynamic dispatch, no pointer indirection. The compiler inlines the `MemoryStorage` method calls directly. You get abstraction at zero runtime cost.

This pattern — generic struct with trait bound — is how Rust achieves what other languages call "dependency injection" or "strategy pattern." The database does not depend on `MemoryStorage` specifically. It depends on the `Storage` trait. In Chapter 3, you will create `DiskStorage` and plug it in with zero changes to `Database`.

### Why `key: &str` in Database but `key: String` in Storage?

The `Database` methods take `key: &str` (a borrowed reference) and internally convert to `key.to_string()` (an owned `String`). The `Storage` trait takes `key: String` (already owned). This is an ergonomic choice:

- **Callers of Database** should not need to allocate a `String` just to look up a key. `db.get("name")` is cleaner than `db.get("name".to_string())`.
- **Implementors of Storage** receive an owned `String` because the storage engine may need to store it (e.g., inserting into a `BTreeMap`). If it received `&str`, it would need to clone it anyway.

The `Database` layer sits between the caller and the engine, converting from the most ergonomic API to the most efficient one. This is a common pattern in Rust: borrow at the boundary, own at the core.

---

## Exercise 4: Testing with the Trait

**Goal:** Write unit tests that verify `MemoryStorage` correctly handles all four operations: set, get, delete, and scan.

### Step 1: Add tests to the memory module

Open `src/memory.rs` and add a test module at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut store = MemoryStorage::new();

        store.set("key1".to_string(), b"value1".to_vec()).unwrap();
        store.set("key2".to_string(), b"value2".to_vec()).unwrap();

        assert_eq!(
            store.get("key1").unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            store.get("key2").unwrap(),
            Some(b"value2".to_vec())
        );
    }

    #[test]
    fn get_missing_key() {
        let store = MemoryStorage::new();
        assert_eq!(store.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn overwrite_existing_key() {
        let mut store = MemoryStorage::new();

        store.set("key".to_string(), b"first".to_vec()).unwrap();
        store.set("key".to_string(), b"second".to_vec()).unwrap();

        assert_eq!(
            store.get("key").unwrap(),
            Some(b"second".to_vec())
        );
    }

    #[test]
    fn delete_existing_key() {
        let mut store = MemoryStorage::new();

        store.set("key".to_string(), b"value".to_vec()).unwrap();
        store.delete("key").unwrap();

        assert_eq!(store.get("key").unwrap(), None);
    }

    #[test]
    fn delete_missing_key() {
        let mut store = MemoryStorage::new();
        // Deleting a key that does not exist should succeed silently.
        store.delete("ghost").unwrap();
    }

    #[test]
    fn scan_range() {
        let mut store = MemoryStorage::new();

        store.set("apple".to_string(), b"1".to_vec()).unwrap();
        store.set("banana".to_string(), b"2".to_vec()).unwrap();
        store.set("cherry".to_string(), b"3".to_vec()).unwrap();
        store.set("date".to_string(), b"4".to_vec()).unwrap();
        store.set("elderberry".to_string(), b"5".to_vec()).unwrap();

        // Scan from "banana" to "date" (inclusive start, exclusive end)
        let results = store
            .scan("banana".to_string().."date".to_string())
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "banana");
        assert_eq!(results[1].0, "cherry");
    }

    #[test]
    fn scan_all() {
        let mut store = MemoryStorage::new();

        store.set("z".to_string(), b"last".to_vec()).unwrap();
        store.set("a".to_string(), b"first".to_vec()).unwrap();
        store.set("m".to_string(), b"middle".to_vec()).unwrap();

        let results = store.scan(..).unwrap();

        // Results should be in sorted key order regardless of insertion order.
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "m");
        assert_eq!(results[2].0, "z");
    }

    #[test]
    fn scan_empty_range() {
        let mut store = MemoryStorage::new();

        store.set("apple".to_string(), b"1".to_vec()).unwrap();
        store.set("banana".to_string(), b"2".to_vec()).unwrap();

        // No keys start with "x", so this range is empty.
        let results = store
            .scan("x".to_string().."z".to_string())
            .unwrap();

        assert_eq!(results.len(), 0);
    }
}
```

### Step 2: Run the tests

```bash
cargo test
```

Expected output:

```
running 8 tests
test memory::tests::delete_existing_key ... ok
test memory::tests::delete_missing_key ... ok
test memory::tests::get_missing_key ... ok
test memory::tests::overwrite_existing_key ... ok
test memory::tests::scan_all ... ok
test memory::tests::scan_empty_range ... ok
test memory::tests::scan_range ... ok
test memory::tests::set_and_get ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All eight tests pass. Let us look at the testing patterns used:

**`#[cfg(test)]`** — this attribute means the `mod tests` block is only compiled when running `cargo test`. It does not exist in your release binary. This is a compile-time conditional, not a runtime one.

**`#[test]`** — marks a function as a test case. `cargo test` discovers and runs every function with this attribute.

**`b"value1"`** — a byte string literal. It produces a `&[u8]` (a reference to a byte slice), and `.to_vec()` converts it to an owned `Vec<u8>`. This is more concise than writing `"value1".as_bytes().to_vec()`.

**`assert_eq!(left, right)`** — panics if `left != right`, printing both values. Tests pass by returning without panicking. There is no `expect()` or assertion library — the standard `assert_eq!` macro is sufficient for most cases.

**Each test creates its own `MemoryStorage`** — tests are isolated by design. There is no shared state, no setup/teardown, no test ordering dependencies. Each test starts with an empty storage engine and cannot be affected by other tests.

<details>
<summary>Hint: If a test fails with "not implemented"</summary>

Make sure all four methods in your `impl Storage for MemoryStorage` block have real implementations, not `todo!()` placeholders. The `todo!()` macro compiles but panics at runtime — your test will show a panic message pointing to the `todo!()` line.

</details>

---
