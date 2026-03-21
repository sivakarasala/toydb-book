# Chapter 2: In-Memory Storage Engine

Every database needs a place to put data. Before you worry about disks, files, or network protocols, you need to answer a simpler question: if someone hands you a key and a value, where do you store them so you can get the value back later? This chapter builds that foundation — a storage engine that lives entirely in memory.

By the end of this chapter, you will have:

- A `Storage` trait that defines the contract every storage engine must fulfill
- A `MemoryStorage` struct backed by a `BTreeMap` with ordered iteration
- A generic `Database<S: Storage>` that works with any engine
- Unit tests proving your engine handles set, get, delete, and range scans

---

## Spotlight: Traits & Generics

Every chapter has one spotlight concept. This chapter's spotlight is **traits and generics** — the way Rust defines shared behavior and writes code that works across multiple types.

### What is a trait?

A trait is a contract. It says: "any type that implements me must provide these methods." Think of it as a promise a type makes to the rest of your codebase.

```rust
trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
}
```

This trait declares two methods but provides no implementations. Any type that wants to call itself a `Storage` must define both. The compiler enforces this at compile time — not at runtime, not in tests, not "hopefully in code review." If you forget a method, the code does not build.

### Implementing a trait

To fulfill the contract, you write an `impl TraitName for YourType` block:

```rust
struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl Storage for MemoryStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error> {
        self.data.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.data.get(key).cloned())
    }
}
```

The struct has its own data (`BTreeMap`), and the trait implementation defines how that data is accessed through the `Storage` interface. You can have multiple types implementing the same trait — a `DiskStorage`, a `NetworkStorage`, a `MockStorage` for tests — and they all fulfill the same contract.

### Trait bounds on generics

Here is where traits become powerful. You can write a function (or a struct) that works with *any* type implementing a trait:

```rust
fn count_keys<S: Storage>(store: &S) -> usize {
    // This function works with MemoryStorage, DiskStorage, anything
    // that implements Storage
    todo!()
}
```

The `<S: Storage>` syntax says: "S can be any type, as long as it implements Storage." This is a **trait bound** — it constrains the generic type parameter. The compiler generates specialized code for each concrete type you use, so there is no runtime overhead. You get the flexibility of polymorphism with the performance of monomorphism.

### Why this matters for databases

The `Storage` trait is the seam in your architecture. Everything above it (the SQL engine, the query planner, the client protocol) calls `set()`, `get()`, `delete()`, and `scan()` without knowing whether the data lives in a `BTreeMap`, an on-disk B-tree, or a distributed consensus log. When you build persistent storage in Chapter 3, you will implement the same trait for a different backend. The database will not change — only the engine underneath.

> **Coming from JS/Python/Go?**
>
> **JavaScript:** There is no direct equivalent. JavaScript uses duck typing — if an object has a `.get()` method, you can call it, and if it does not, you get a runtime error. Rust traits are like TypeScript interfaces, but enforced at compile time with no escape hatch. There is no `as any`.
>
> ```typescript
> // TypeScript: interface is optional, duck typing still works
> interface Storage {
>   set(key: string, value: Uint8Array): void;
>   get(key: string): Uint8Array | null;
> }
> ```
>
> **Python:** Traits are closest to `Protocol` (PEP 544) or abstract base classes (`ABC`). The difference: Python protocols are checked by mypy (optional), while Rust traits are checked by the compiler (mandatory). You cannot skip trait checking in Rust.
>
> ```python
> # Python: ABC, but enforcement is optional
> from abc import ABC, abstractmethod
>
> class Storage(ABC):
>     @abstractmethod
>     def set(self, key: str, value: bytes) -> None: ...
>
>     @abstractmethod
>     def get(self, key: str) -> bytes | None: ...
> ```
>
> **Go:** Go interfaces are the closest analog. Both are satisfied implicitly (Go) or explicitly (Rust). The key difference: Rust traits support generics and static dispatch, while Go interfaces always use dynamic dispatch (interface values carry a vtable pointer). Rust gives you the choice.
>
> ```go
> // Go: implicit interface satisfaction
> type Storage interface {
>     Set(key string, value []byte) error
>     Get(key string) ([]byte, error)
> }
> ```
>
> In all three languages, you can define a "shape" that types must match. Rust's version catches violations earlier (compile time), runs faster (static dispatch by default), and cannot be bypassed.

---

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

## Rust Gym

Time for reps. These drills focus on traits and generics.

### Drill 1: Define a Trait (Simple)

Define a trait `Greet` with a single method `fn hello(&self) -> String`. Implement it for two structs: `English` and `Spanish`. Then write a function `print_greeting` that accepts any type implementing `Greet` and prints the result.

```rust
// Define the trait and two structs here.

fn print_greeting(greeter: &impl Greet) {
    println!("{}", greeter.hello());
}

fn main() {
    let en = English;
    let es = Spanish;
    print_greeting(&en); // Expected: "Hello!"
    print_greeting(&es); // Expected: "Hola!"
}
```

<details>
<summary>Solution</summary>

```rust
trait Greet {
    fn hello(&self) -> String;
}

struct English;
struct Spanish;

impl Greet for English {
    fn hello(&self) -> String {
        "Hello!".to_string()
    }
}

impl Greet for Spanish {
    fn hello(&self) -> String {
        "Hola!".to_string()
    }
}

fn print_greeting(greeter: &impl Greet) {
    println!("{}", greeter.hello());
}

fn main() {
    let en = English;
    let es = Spanish;
    print_greeting(&en);
    print_greeting(&es);
}
```

Output:

```
Hello!
Hola!
```

The `&impl Greet` parameter syntax is shorthand for `<G: Greet>(greeter: &G)`. Both forms mean the same thing: "accept a reference to any type that implements Greet." The `impl Trait` syntax is more concise and preferred when you do not need to refer to the type parameter elsewhere in the function signature.

`struct English;` is a **unit struct** — a struct with no fields. It has zero size in memory. Unit structs are useful when you need a type to implement a trait but the type carries no data. Think of it as a tag or a marker.

</details>

### Drill 2: Generic Function (Medium)

Write a generic function `largest` that takes a slice `&[T]` and returns a reference to the largest element. The function should work with any type that can be compared.

```rust
fn largest<T: Ord>(list: &[T]) -> &T {
    // Your code here
}

fn main() {
    let numbers = vec![34, 50, 25, 100, 65];
    println!("Largest number: {}", largest(&numbers));
    // Expected: "Largest number: 100"

    let words = vec!["apple", "zebra", "mango"];
    println!("Largest word: {}", largest(&words));
    // Expected: "Largest word: zebra"
}
```

<details>
<summary>Solution</summary>

```rust
fn largest<T: Ord>(list: &[T]) -> &T {
    let mut max = &list[0];
    for item in &list[1..] {
        if item > max {
            max = item;
        }
    }
    max
}

fn main() {
    let numbers = vec![34, 50, 25, 100, 65];
    println!("Largest number: {}", largest(&numbers));

    let words = vec!["apple", "zebra", "mango"];
    println!("Largest word: {}", largest(&words));
}
```

Output:

```
Largest number: 100
Largest word: zebra
```

The trait bound `T: Ord` means "T must implement the `Ord` trait," which provides total ordering (the `<`, `>`, `<=`, `>=` operators). Without this bound, the compiler would reject `item > max` because it cannot know whether `T` supports comparison.

Notice the function returns `&T`, not `T`. It borrows an element from the input slice rather than cloning it. This means the caller can inspect the largest element without any allocation. The lifetime of the returned reference is tied to the input slice — the compiler ensures you cannot use the result after the slice is freed.

If the slice is empty, `list[0]` will panic. A production version would return `Option<&T>` and handle the empty case. For a drill, the panic is acceptable.

</details>

### Drill 3: Serialization Trait (Advanced)

Define a trait `Codec` with two methods: `fn encode(&self) -> Vec<u8>` (serialize to bytes) and `fn decode(bytes: &[u8]) -> Result<Self, String> where Self: Sized` (deserialize from bytes). Implement it for a `Point { x: i32, y: i32 }` struct using a simple encoding: 4 bytes for x, 4 bytes for y, both as big-endian.

```rust
// Define the Codec trait and Point struct here.

fn main() {
    let p = Point { x: 42, y: -7 };
    let bytes = p.encode();
    println!("Encoded: {:?}", bytes);
    // Expected: [0, 0, 0, 42, 255, 255, 255, 249]

    let decoded = Point::decode(&bytes).unwrap();
    println!("Decoded: ({}, {})", decoded.x, decoded.y);
    // Expected: "Decoded: (42, -7)"
}
```

<details>
<summary>Solution</summary>

```rust
trait Codec {
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self, String>
    where
        Self: Sized;
}

struct Point {
    x: i32,
    y: i32,
}

impl Codec for Point {
    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(&self.x.to_be_bytes());
        buf.extend_from_slice(&self.y.to_be_bytes());
        buf
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 8 {
            return Err(format!("expected 8 bytes, got {}", bytes.len()));
        }
        let x = i32::from_be_bytes(
            bytes[0..4].try_into().map_err(|e| format!("{}", e))?,
        );
        let y = i32::from_be_bytes(
            bytes[4..8].try_into().map_err(|e| format!("{}", e))?,
        );
        Ok(Point { x, y })
    }
}

fn main() {
    let p = Point { x: 42, y: -7 };
    let bytes = p.encode();
    println!("Encoded: {:?}", bytes);

    let decoded = Point::decode(&bytes).unwrap();
    println!("Decoded: ({}, {})", decoded.x, decoded.y);
}
```

Output:

```
Encoded: [0, 0, 0, 42, 255, 255, 255, 249]
Decoded: (42, -7)
```

Key details:

- **`to_be_bytes()`** converts an `i32` into 4 bytes in big-endian order. Big-endian means the most significant byte comes first — this is the network byte order used by most binary protocols.
- **`try_into()`** converts a byte slice `&[u8]` into a fixed-size array `[u8; 4]`. It returns a `Result` because the slice might have the wrong length.
- **`where Self: Sized`** is required on `decode` because the compiler needs to know the size of `Self` at compile time to return it by value. Most types are `Sized`; this bound is only relevant for trait objects (which we do not use here).
- **`Vec::with_capacity(8)`** pre-allocates exactly 8 bytes. Without it, the `Vec` would start with a default capacity and potentially reallocate as bytes are added. Pre-allocation is a micro-optimization, but it demonstrates intentional memory management.

This pattern — encode to bytes, decode from bytes — is exactly what Chapter 4 (Serialization) will build on at scale. Every row in the database will be serialized to `Vec<u8>` before storage and deserialized back on retrieval.

</details>

---

## DSA in Context: BTreeMap vs HashMap

You used a `BTreeMap` to back your storage engine. Here is why that choice matters, examined through the lens of algorithmic complexity and real-world database behavior.

### The B-tree

A B-tree is a self-balancing tree where each node can hold multiple keys. Rust's `BTreeMap` uses a B-tree with a branching factor tuned for cache performance (typically 6-11 keys per node). The key property: **all keys are stored in sorted order**.

```
                    [   dog   |   mango   ]
                   /          |            \
          [apple|banana]   [cherry|date]   [orange|zebra]
```

Each node holds multiple keys in sorted order. Child pointers sit between and around the keys. To find "cherry", you start at the root: "cherry" is between "dog" and "mango" is wrong — "cherry" < "dog", so go left? No — "cherry" > "banana", so it is in the middle child. One comparison per level, and the tree is shallow because each node holds many keys.

### Complexity comparison

| Operation | `HashMap` | `BTreeMap` |
|-----------|-----------|-----------|
| `get(key)` | O(1) average, O(n) worst | O(log n) |
| `insert(key, value)` | O(1) average, O(n) worst | O(log n) |
| `remove(key)` | O(1) average, O(n) worst | O(log n) |
| `range(a..b)` | Not supported | O(log n + k) |
| `iteration order` | Arbitrary | Sorted by key |
| `min / max key` | O(n) | O(log n) |

For a million keys, O(log n) means about 20 comparisons. O(1) means one hash computation. The `HashMap` wins on raw point-lookup speed, but it cannot answer "give me the next 100 keys after this one" without a full scan and sort.

### Why databases choose B-trees

Databases are not just key-value stores. They answer queries like:

- `SELECT * FROM users WHERE id BETWEEN 100 AND 200` (range scan)
- `SELECT * FROM users ORDER BY name LIMIT 10` (ordered iteration)
- `SELECT MIN(created_at) FROM events` (minimum key)

All of these require ordered data. A hash table makes these operations O(n) — you must scan every entry. A B-tree makes them O(log n + k) — find the start point, then walk forward.

This is why PostgreSQL, MySQL (InnoDB), SQLite, and most relational databases use B-tree variants for their indexes. The O(1) point lookup of a hash table does not compensate for the inability to scan ranges efficiently.

Our `MemoryStorage` inherits these properties from `BTreeMap`. When we test `scan("banana"..="date")`, the `BTreeMap` finds "banana" in O(log n) and iterates to "date" without visiting any other keys. The data structure does the heavy lifting.

---

## System Design Corner: Pluggable Storage Engines

In a system design interview, you might be asked: *"Design a storage engine for a database."* The trait pattern you built in this chapter is the answer to the first question every interviewer asks: *"How do you support multiple backends?"*

### The architecture

```
┌──────────────────────────────────────────┐
│            SQL Engine / API              │
├──────────────────────────────────────────┤
│          Database<S: Storage>            │
├──────────────┬───────────┬───────────────┤
│ MemoryStorage│DiskStorage│DistributedStore│
│  (BTreeMap)  │ (BitCask) │   (Raft)      │
└──────────────┴───────────┴───────────────┘
```

Everything above the `Storage` trait is engine-agnostic. The SQL parser, the query planner, the client protocol — none of them know which engine is active. This is the **pluggable storage engine** pattern, used by MySQL (InnoDB, MyISAM, Memory), MongoDB (WiredTiger, MMAPv1), and many other databases.

### Interview talking points

**Why in-memory first?** It is the simplest correct implementation. You validate the interface, build tests, and get the database logic working before adding the complexity of disk I/O. This is how CockroachDB, TiDB, and other production databases develop their storage layers — memory first, then disk.

**Why a trait instead of an enum?** A trait is open for extension. Adding a new engine means adding a new struct and `impl Storage for NewEngine`. An enum is closed — adding a variant requires changing the enum definition and every `match` that handles it. Traits follow the open-closed principle: open for extension, closed for modification.

**What about performance?** The generic `Database<S: Storage>` uses static dispatch — the compiler generates one version of `Database` per engine type. There is no vtable lookup, no pointer indirection. This is equivalent to writing separate `DatabaseMemory` and `DatabaseDisk` types, but without duplicating any code. In Rust, abstraction does not cost performance.

**What about testing?** The trait enables mock storage engines for testing. You can create a `FailingStorage` that returns errors on every operation to test your database's error handling, or a `SlowStorage` that adds latency to test timeout behavior. The database under test never knows the difference.

> **Interview framing:** *"We define a Storage trait with set, get, delete, and scan. The Database struct is generic over any Storage implementation. This gives us pluggable backends — we start with in-memory for development, add disk persistence for production, and can later add distributed storage. The trait boundary is where we swap engines without touching the database logic."*

---

## Design Insight: Deep Modules

In *A Philosophy of Software Design*, John Ousterhout introduces the concept of **deep modules** — modules that provide powerful functionality behind a simple interface. The value of a module is the ratio of its functionality to the complexity of its interface. Deep modules have simple interfaces and rich implementations. Shallow modules have complex interfaces that do not hide much.

The `Storage` trait is a deep module. Its interface is four methods:

```rust
fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;
fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
fn delete(&mut self, key: &str) -> Result<(), Error>;
fn scan(&self, range: impl RangeBounds<String>) -> Result<Vec<(String, Vec<u8>)>, Error>;
```

Four methods. A caller can learn this interface in two minutes. Behind it, an implementation might manage:

- A B-tree with node splitting, merging, and rebalancing
- Write-ahead logging for crash recovery
- Memory-mapped files for zero-copy reads
- Bloom filters for negative lookups
- Compaction threads that merge sorted runs
- Checksums for data integrity

All of that complexity is hidden. The caller writes `store.get("key")` and gets bytes back. They do not know, need to know, or want to know about B-tree fanout or compaction strategies.

Contrast this with a shallow interface that exposes implementation details:

```rust
// Shallow: leaks implementation details
fn get(&self, key: &str, use_bloom_filter: bool, cache_hint: CachePolicy) -> ...
fn set(&mut self, key: String, value: Vec<u8>, sync: bool, compression: Codec) -> ...
```

Every parameter beyond the essentials (`key`, `value`) is a leak. The caller must understand bloom filters, cache policies, sync semantics, and compression codecs to use the API. The interface is wide, and its complexity grows with every new feature.

Deep modules are the goal. The `Storage` trait stays at four methods even as the implementation grows from 20 lines (MemoryStorage) to thousands (a production disk engine). That is depth.

---

## What You Built

In this chapter, you:

1. **Defined a `Storage` trait** — four methods that every storage engine must implement, with a custom `Error` enum for structured error handling
2. **Implemented `MemoryStorage`** — a B-tree-backed in-memory engine with ordered keys and efficient range scans
3. **Built a generic `Database<S: Storage>`** — a database layer that works with any storage engine through trait bounds and static dispatch
4. **Wrote unit tests** — eight tests covering set, get, delete, overwrite, missing keys, range scans, full scans, and empty ranges

Your database can store and retrieve key-value pairs, scan ranges in sorted order, and swap storage engines at compile time. But when the process exits, everything is gone. In Chapter 3, we will add persistent storage — writing to disk so data survives restarts.

---

### DS Deep Dive

Ready to go deeper? This chapter's data structure deep dive explores the B-tree — the data structure that powers your `BTreeMap` and nearly every database index in production.

**-> [B-Tree -- "The filing cabinet that sorts itself"](../ds-narratives/ch02-b-tree.md)**

You used `BTreeMap` as a black box. This deep dive opens the box: node structure, key splitting, tree rebalancing, and why B-trees are cache-friendly. You will understand why databases chose this structure fifty years ago and why they still choose it today.

---

### Reference

The files you built in this chapter:

| File | Purpose |
|------|---------|
| `src/error.rs` | Custom `Error` enum with `NotFound` and `Internal` variants |
| `src/storage.rs` | `Storage` trait — the contract for all storage engines |
| `src/memory.rs` | `MemoryStorage` — B-tree-backed in-memory implementation with tests |
| `src/database.rs` | `Database<S: Storage>` — generic database layer |
| `src/main.rs` | Module registration and demo usage |
