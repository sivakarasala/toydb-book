## Exercise 1: Define the Error Type

**Goal:** Before we define the `Storage` trait, we need an error type. Storage operations can fail (the key might not exist, the disk might be full), and Rust requires us to represent those failures explicitly.

### What is Result?

In Chapter 1, we used `Option<T>` for values that might not exist. `Result<T, E>` is similar, but it carries an error when something goes wrong:

```rust
enum Option<T> {
    Some(T),    // found a value
    None,       // no value
}

enum Result<T, E> {
    Ok(T),      // success — here is the value
    Err(E),     // failure — here is what went wrong
}
```

Think of it this way:
- `Option` answers: "Is there a value?" (yes/no)
- `Result` answers: "Did the operation succeed?" (yes, here is the result / no, here is the error)

### Step 1: Create the error module

Create a new file `src/error.rs`:

```rust
use std::fmt;

#[derive(Debug, Clone)]
pub enum Error {
    NotFound(String),
    Internal(String),
}
```

Our `Error` enum has two variants:
- `NotFound(String)` — the requested key does not exist, and we carry the key name for context.
- `Internal(String)` — something went wrong internally, and we carry a description.

The `pub` keyword makes this type public — visible to other modules (files) in our project. Without `pub`, other files cannot use it.

### Step 2: Implement Display

Add this below the enum:

```rust
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(key) => write!(f, "key not found: {}", key),
            Error::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}
```

> **What just happened?**
>
> We implemented the `Display` trait for our `Error` type. This lets us print errors with `{}` formatting. Without this, we could only print them with `{:?}` (the Debug format, which shows the raw structure). `Display` is for human-readable output; `Debug` is for developer output.

### Step 3: Register the module

Open `src/main.rs` and add at the top (before the `use` statements):

```rust
mod error;
```

This tells Rust: "there is a module called `error` in the file `src/error.rs`."

> **What is a module?**
>
> A module is a way to organize code into separate files. Each `.rs` file can be a module. You declare a module with `mod filename;` (without the `.rs` extension), and Rust looks for the file in `src/filename.rs`.
>
> Think of modules like folders in a filing cabinet. Each module holds related code, and you can access it from other modules by using its name.

---

## Exercise 2: Define the Storage Trait

**Goal:** Define the `Storage` trait that every storage engine must implement. This is the USB port that all our storage devices will plug into.

### Step 1: Create the storage module

Create a new file `src/storage.rs`:

```rust
use crate::error::Error;

pub trait Storage {
    /// Store a key-value pair. Overwrites any existing value.
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;

    /// Get the value for a key. Returns None if the key does not exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Delete a key. Returns Ok(()) even if the key did not exist.
    fn delete(&mut self, key: &str) -> Result<(), Error>;

    /// Scan all key-value pairs in key order.
    fn scan(&self) -> Result<Vec<(String, Vec<u8>)>, Error>;
}
```

Let's understand every piece:

- `use crate::error::Error;` — Import our `Error` type from the `error` module. `crate` means "the root of this project."

- `pub trait Storage` — Declare a public trait named `Storage`.

- `fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;` — The `set` method. It takes a mutable reference to self (because it modifies data), an owned `String` key, and a `Vec<u8>` value (raw bytes — any kind of data). It returns `Result<(), Error>` — either `Ok(())` (success, no value to return) or `Err(error)`.

- `fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;` — The `get` method. It takes an immutable reference (just reading) and a borrowed key. It returns `Result<Option<Vec<u8>>, Error>` — the operation might fail (the `Result` part) and the key might not exist (the `Option` part).

- The `/// comments` above each method are **doc comments**. They describe what the method does and can be turned into documentation with `cargo doc`.

### Why `Vec<u8>` for values?

A `Vec<u8>` is a list of bytes — raw binary data. We use it instead of `String` because a storage engine should store *any* kind of data, not just text. An image, a serialized struct, a compressed blob — they are all just bytes.

In later chapters, we will serialize our `Value` enum into bytes before storing it. The storage engine does not care what the bytes represent — it just stores and retrieves them.

> **Analogy: Storage = Post Office**
>
> Think of the storage engine as a post office. It accepts packages (bytes), labels them with an address (the key), stores them, and retrieves them when asked. The post office does not open the packages or care what is inside. That is the job of whoever sends and receives them.

### Step 2: Register the module

Add to `src/main.rs`:

```rust
mod error;
mod storage;
```

### Step 3: Verify it compiles

```bash
cargo build
```

If it compiles without errors, you have a valid trait definition. No implementations yet — just the contract.

> **What just happened?**
>
> You defined a trait with four methods. Any type that implements `Storage` must provide all four. The compiler will enforce this — if you forget one, your code will not build. This is the contract that all storage engines in our database will follow.

---

## Exercise 3: Implement MemoryStorage

**Goal:** Build a `MemoryStorage` struct that implements the `Storage` trait using a `BTreeMap`.

### BTreeMap vs HashMap

In Chapter 1, we used `HashMap`. Now we will use `BTreeMap`. What is the difference?

| Feature | HashMap | BTreeMap |
|---------|---------|----------|
| Lookup speed | O(1) average | O(log n) |
| Insert speed | O(1) average | O(log n) |
| Ordering | **No order** — keys come out in random order | **Sorted** — keys come out in alphabetical order |
| When to use | When you only need fast lookup | When you need keys in order |

For a database, sorted order is important. When you scan all keys, you want them in a predictable order. When you do range queries ("all keys between A and M"), you need the keys sorted. That is why real databases use ordered data structures.

> **Analogy: HashMap vs BTreeMap**
>
> A `HashMap` is like throwing letters into a pile. Finding one letter is fast (you remember roughly where you tossed it), but getting them in alphabetical order means sorting the whole pile.
>
> A `BTreeMap` is like a filing cabinet with alphabetical dividers. Finding a letter takes a few more steps (open the right drawer, flip through the dividers), but they are always in order. Getting all letters from A to M means just pulling those drawers.

### Step 1: Create the memory module

Create a new file `src/memory.rs`:

```rust
use std::collections::BTreeMap;
use crate::error::Error;
use crate::storage::Storage;
```

We import three things:
- `BTreeMap` from the standard library
- Our `Error` type
- Our `Storage` trait

### Step 2: Define the struct

```rust
pub struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}
```

This is almost identical to the `Database` struct from Chapter 1, but it uses `BTreeMap` instead of `HashMap`, and stores `Vec<u8>` (bytes) instead of `Value`.

### Step 3: Add a constructor

```rust
impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage {
            data: BTreeMap::new(),
        }
    }
}
```

### Step 4: Implement the Storage trait

Now the important part — making `MemoryStorage` fulfill the `Storage` contract:

```rust
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

    fn scan(&self) -> Result<Vec<(String, Vec<u8>)>, Error> {
        let pairs: Vec<(String, Vec<u8>)> = self.data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(pairs)
    }
}
```

Let's walk through each method:

**`set`** — Inserts the key-value pair into the BTreeMap. Returns `Ok(())` because an in-memory insert cannot fail. (On-disk storage *can* fail — disk full, permission denied — which is why the trait returns `Result`.)

**`get`** — Looks up the key. `self.data.get(key)` returns `Option<&Vec<u8>>` (a reference). We call `.cloned()` to turn that into `Option<Vec<u8>>` (an owned copy). We need to return an owned value because the caller should not hold a reference into our internal data structure.

**`delete`** — Removes the key. We ignore whether the key existed (`remove` returns `Option`, which we discard).

**`scan`** — Iterates over all entries in key order and collects them into a Vec. The `.map()` call clones each key and value because we need to return owned data.

> **What just happened?**
>
> We implemented all four methods of the `Storage` trait for `MemoryStorage`. The compiler now knows that `MemoryStorage` fulfills the `Storage` contract. Any function that accepts `S: Storage` can now work with `MemoryStorage`.
>
> Notice that every method returns `Ok(...)`. For in-memory storage, nothing can go wrong. But we still wrap the return values in `Ok` because the trait requires `Result`. This ensures consistency — the code that calls these methods handles errors the same way regardless of which storage engine is behind it.

### Step 5: Register the module

Add to `src/main.rs`:

```rust
mod error;
mod memory;
mod storage;
```

### Step 6: Verify it compiles

```bash
cargo build
```

### Common mistakes

**Mistake: Returning `&Vec<u8>` instead of `Vec<u8>` from `get`**

The trait says `get` returns `Result<Option<Vec<u8>>, Error>` — an owned `Vec<u8>`. If you try to return a reference (`&Vec<u8>`), the compiler will complain. Use `.cloned()` to create an owned copy.

**Mistake: Not importing the trait**

If you write `impl Storage for MemoryStorage` but forget `use crate::storage::Storage;`, the compiler will say it does not know what `Storage` is.

---

## Exercise 4: Build a Generic Database

**Goal:** Rewrite the `Database` struct to be generic over any `Storage` implementation. This means the same `Database` code works with `MemoryStorage`, the `DiskStorage` you will build in Chapter 3, or any future engine.

### Step 1: Define the generic Database struct

The old `Database` had a concrete type:

```rust
// Old — locked to HashMap
struct Database {
    data: HashMap<String, Value>,
}
```

The new one uses a generic type parameter:

```rust
// New — works with any Storage
struct Database<S: Storage> {
    storage: S,
}
```

Let's understand `<S: Storage>`:

- `<S>` declares a type parameter named `S`. It is a placeholder for a real type.
- `: Storage` is a **trait bound**. It says "S must implement the `Storage` trait."
- `storage: S` is a field of type `S`.

When you create a `Database<MemoryStorage>`, `S` becomes `MemoryStorage`. The struct becomes:

```rust
// What the compiler sees for Database<MemoryStorage>
struct Database {
    storage: MemoryStorage,
}
```

> **Analogy: Generic = Recipe**
>
> A recipe for "fruit smoothie" does not specify which fruit. It says "take 2 cups of fruit, add yogurt, blend." You choose the fruit when you make it. `Database<S: Storage>` is a recipe that says "take a storage engine, wrap it in a database." You choose the engine when you create it.

### Step 2: Create the database module

Create `src/database.rs`:

```rust
use crate::error::Error;
use crate::storage::Storage;

pub struct Database<S: Storage> {
    storage: S,
}

impl<S: Storage> Database<S> {
    pub fn new(storage: S) -> Self {
        Database { storage }
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), Error> {
        self.storage.set(key.to_string(), value.to_vec())
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        self.storage.get(key)
    }

    pub fn delete(&mut self, key: &str) -> Result<(), Error> {
        self.storage.delete(key)
    }

    pub fn list(&self) -> Result<Vec<(String, Vec<u8>)>, Error> {
        self.storage.scan()
    }
}
```

Notice the `impl<S: Storage> Database<S>` syntax. This says: "For any type `S` that implements `Storage`, here are the methods of `Database<S>`." The `<S: Storage>` appears twice — once on `impl` (declaring the type parameter) and once on `Database` (using it).

> **What just happened?**
>
> The `Database` struct no longer knows or cares which storage engine it uses. It calls `self.storage.set()`, `self.storage.get()`, etc. — the trait's methods. Whether the storage is in memory, on disk, or across a network, the `Database` code is identical.
>
> The `set` method converts `&str` to `String` and `&[u8]` to `Vec<u8>`. This is a convenience: callers pass borrowed data, and the database converts to owned data before passing to the storage engine. This pattern is common in Rust APIs.

### Step 3: Register the module and test

Add to `src/main.rs`:

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

    db.set("name", b"toydb").unwrap();
    db.set("version", b"0.1.0").unwrap();

    match db.get("name").unwrap() {
        Some(value) => {
            let text = String::from_utf8(value).unwrap();
            println!("name = {}", text);
        }
        None => println!("name not found"),
    }

    let entries = db.list().unwrap();
    println!("\nAll entries:");
    for (key, value) in &entries {
        let text = String::from_utf8(value.clone()).unwrap();
        println!("  {} = {}", key, text);
    }
}
```

The `b"toydb"` syntax creates a byte slice (`&[u8]`) from a string literal. It is shorthand for `&[116, 111, 121, 100, 98]` — the ASCII codes for "toydb".

`String::from_utf8(value)` converts bytes back into a `String`. It returns `Result` because not all byte sequences are valid UTF-8 text.

Run it:

```bash
cargo run
```

Expected output:

```
name = toydb

All entries:
  name = toydb
  version = 0.1.0
```

Notice the entries are in alphabetical order ("name" before "version") — that is the BTreeMap at work.

---

## Exercise 5: Write Unit Tests

**Goal:** Write tests to verify that your `MemoryStorage` works correctly. Tests catch bugs before users do.

### What is a test?

A test is a function that runs your code and checks that it does what you expect. In Rust, you mark test functions with `#[test]` and put them in a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
```

Let's understand each piece:

- `#[cfg(test)]` — This module only compiles when running tests. It is not included in the final program.
- `mod tests` — A module named "tests" (convention, not requirement).
- `use super::*;` — Import everything from the parent module (so we can use `MemoryStorage`, `Storage`, etc.).
- `#[test]` — Marks a function as a test.
- `assert_eq!(a, b)` — Panics (crashes) if `a` does not equal `b`. In test mode, a panic means the test failed.

### Step 1: Add tests to memory.rs

Add this at the bottom of `src/memory.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut store = MemoryStorage::new();

        store.set("key1".to_string(), b"value1".to_vec()).unwrap();

        let result = store.get("key1").unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }
}
```

> **What just happened?**
>
> This test creates a `MemoryStorage`, sets a key-value pair, then retrieves it and checks that the value matches. If `get` returns something other than `Some(b"value1".to_vec())`, the test fails.
>
> The `b"value1".to_vec()` syntax: `b"value1"` is a byte string literal (`&[u8]`), and `.to_vec()` converts it to `Vec<u8>` (owned bytes).
>
> The `.unwrap()` calls convert `Result<T, Error>` to `T`, panicking on errors. In tests, panicking is fine — a panic means the test fails with a clear error message.

### Step 2: Add more tests

```rust
    #[test]
    fn get_missing_key() {
        let store = MemoryStorage::new();
        let result = store.get("nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn set_overwrites_existing() {
        let mut store = MemoryStorage::new();

        store.set("key".to_string(), b"first".to_vec()).unwrap();
        store.set("key".to_string(), b"second".to_vec()).unwrap();

        let result = store.get("key").unwrap();
        assert_eq!(result, Some(b"second".to_vec()));
    }

    #[test]
    fn delete_existing_key() {
        let mut store = MemoryStorage::new();

        store.set("key".to_string(), b"value".to_vec()).unwrap();
        store.delete("key").unwrap();

        let result = store.get("key").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn delete_missing_key_is_ok() {
        let mut store = MemoryStorage::new();

        // Deleting a key that does not exist should not fail
        let result = store.delete("nonexistent");
        assert!(result.is_ok());
    }

    #[test]
    fn scan_returns_sorted_entries() {
        let mut store = MemoryStorage::new();

        // Insert in non-alphabetical order
        store.set("cherry".to_string(), b"3".to_vec()).unwrap();
        store.set("apple".to_string(), b"1".to_vec()).unwrap();
        store.set("banana".to_string(), b"2".to_vec()).unwrap();

        let entries = store.scan().unwrap();

        // BTreeMap returns entries in sorted key order
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].0, "apple");
        assert_eq!(entries[1].0, "banana");
        assert_eq!(entries[2].0, "cherry");
    }

    #[test]
    fn scan_empty_store() {
        let store = MemoryStorage::new();
        let entries = store.scan().unwrap();
        assert!(entries.is_empty());
    }
```

### Step 3: Run the tests

```bash
cargo test
```

Expected output:

```
running 7 tests
test memory::tests::set_and_get ... ok
test memory::tests::get_missing_key ... ok
test memory::tests::set_overwrites_existing ... ok
test memory::tests::delete_existing_key ... ok
test memory::tests::delete_missing_key_is_ok ... ok
test memory::tests::scan_returns_sorted_entries ... ok
test memory::tests::scan_empty_store ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

> **What just happened?**
>
> You ran seven tests, and all passed. Each test is independent — it creates its own `MemoryStorage`, does some operations, and checks the results. If any `assert_eq!` or `assert!` fails, that test is marked as failed and Rust shows you exactly what was expected vs. what was received.

### Common mistakes in tests

**Mistake: Forgetting `use super::*;`**

Without this import, the test module cannot see `MemoryStorage`, `Storage`, or `Error`.

**Mistake: Not calling `.unwrap()` on Results**

```rust
let result = store.get("key");  // This is Result<Option<Vec<u8>>, Error>
assert_eq!(result, Some(b"value".to_vec()));  // ERROR: comparing Result with Option
```

Fix: unwrap the Result first:

```rust
let result = store.get("key").unwrap();  // Now it is Option<Vec<u8>>
assert_eq!(result, Some(b"value".to_vec()));  // OK
```

**Mistake: Comparing `&[u8]` with `Vec<u8>`**

```rust
assert_eq!(result, Some(b"value"));  // ERROR: b"value" is &[u8], not Vec<u8>
```

Fix: convert to `Vec<u8>`:

```rust
assert_eq!(result, Some(b"value".to_vec()));  // OK
```

---

## Exercise 6: Generic Functions

**Goal:** Write a function that works with *any* storage engine. This demonstrates why traits and generics are powerful.

### Step 1: Write a generic populate function

Add this function to `src/database.rs`:

```rust
/// Populate a database with sample data for testing.
pub fn populate_sample_data<S: Storage>(db: &mut Database<S>) -> Result<(), Error> {
    db.set("name", b"toydb")?;
    db.set("version", b"0.1.0")?;
    db.set("author", b"you")?;
    db.set("language", b"Rust")?;
    Ok(())
}
```

> **What just happened?**
>
> This function takes a `&mut Database<S>` where `S` can be *any* type that implements `Storage`. It does not know or care whether the data ends up in memory, on disk, or in the cloud. It just calls `db.set(...)` and trusts that the trait implementation does the right thing.
>
> The `?` operator is new here. It is shorthand for "if this returns an error, return the error immediately." It replaces verbose match statements:
>
> ```rust
> // Without ?
> match db.set("name", b"toydb") {
>     Ok(()) => {},
>     Err(e) => return Err(e),
> }
>
> // With ?
> db.set("name", b"toydb")?;
> ```
>
> The `?` only works in functions that return `Result`. We will explore it deeply in Chapter 3.

### Step 2: Use it in main

Update `src/main.rs`:

```rust
mod database;
mod error;
mod memory;
mod storage;

use database::{Database, populate_sample_data};
use memory::MemoryStorage;

fn main() {
    let storage = MemoryStorage::new();
    let mut db = Database::new(storage);

    populate_sample_data(&mut db).unwrap();

    let entries = db.list().unwrap();
    println!("Entries (in key order):");
    for (key, value) in &entries {
        let text = String::from_utf8(value.clone()).unwrap();
        println!("  {} = {}", key, text);
    }
}
```

Run it:

```bash
cargo run
```

Expected output:

```
Entries (in key order):
  author = you
  language = Rust
  name = toydb
  version = 0.1.0
```

The entries are in alphabetical order because `BTreeMap` maintains sorted keys.

---

## Exercises

Try these before moving to Chapter 3.

**Exercise 2.1: Add a `contains` method to the Storage trait**

Add a method `fn contains(&self, key: &str) -> Result<bool, Error>;` to the `Storage` trait. Provide a default implementation that calls `get` and checks if the result is `Some`.

<details>
<summary>Hint</summary>

Traits can have **default implementations** — methods with a body that types inherit automatically:

```rust
pub trait Storage {
    // ... existing methods ...

    /// Check if a key exists. Default implementation uses get().
    fn contains(&self, key: &str) -> Result<bool, Error> {
        Ok(self.get(key)?.is_some())
    }
}
```

Because this has a default implementation, `MemoryStorage` does not need to implement it explicitly — it inherits the default. But a specific engine could override it with a more efficient implementation.

</details>

**Exercise 2.2: Add a `count` method**

Add `fn count(&self) -> Result<usize, Error>;` to the `Storage` trait. Implement it for `MemoryStorage` using `self.data.len()`.

<details>
<summary>Hint</summary>

This one does not have a good default implementation (you would need to scan all entries just to count them), so leave it as a required method:

```rust
// In the trait:
fn count(&self) -> Result<usize, Error>;

// In MemoryStorage:
fn count(&self) -> Result<usize, Error> {
    Ok(self.data.len())
}
```

</details>

**Exercise 2.3: Write a test for the generic Database**

Write a test in `src/database.rs` that creates a `Database<MemoryStorage>`, adds entries, and verifies they can be retrieved.

<details>
<summary>Hint</summary>

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStorage;

    #[test]
    fn database_set_and_get() {
        let storage = MemoryStorage::new();
        let mut db = Database::new(storage);

        db.set("name", b"toydb").unwrap();

        let value = db.get("name").unwrap();
        assert_eq!(value, Some(b"toydb".to_vec()));
    }

    #[test]
    fn database_list_is_sorted() {
        let storage = MemoryStorage::new();
        let mut db = Database::new(storage);

        db.set("zebra", b"z").unwrap();
        db.set("apple", b"a").unwrap();

        let entries = db.list().unwrap();
        assert_eq!(entries[0].0, "apple");
        assert_eq!(entries[1].0, "zebra");
    }
}
```

</details>

**Exercise 2.4: Implement a second storage engine**

Create a `NullStorage` struct that implements `Storage` but discards all writes and always returns `None` for reads. This is useful for benchmarking the database layer without storage overhead.

<details>
<summary>Hint</summary>

```rust
pub struct NullStorage;

impl Storage for NullStorage {
    fn set(&mut self, _key: String, _value: Vec<u8>) -> Result<(), Error> {
        Ok(())  // discard
    }

    fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(None)  // always empty
    }

    fn delete(&mut self, _key: &str) -> Result<(), Error> {
        Ok(())  // nothing to delete
    }

    fn scan(&self) -> Result<Vec<(String, Vec<u8>)>, Error> {
        Ok(Vec::new())  // always empty
    }
}
```

The `_` prefix on parameter names (`_key`, `_value`) tells Rust "I know I am not using this parameter." Without the underscore, the compiler would warn about unused variables.

</details>

---

## Key Takeaways

- **A trait is a contract.** It defines what methods a type must provide, without specifying how.
- **`impl TraitName for TypeName`** is how you fulfill the contract. The compiler ensures you implement every required method.
- **Generics with trait bounds (`<S: Storage>`)** let you write code that works with any type implementing a trait.
- **`BTreeMap` keeps keys sorted.** Use it when order matters (databases, range queries).
- **`Result<T, E>`** represents success or failure. Use `Ok(value)` for success, `Err(error)` for failure.
- **Unit tests (`#[test]`)** verify your code works correctly. Run them with `cargo test`.
- **Separating interface (trait) from implementation (struct)** is a core design principle. It lets you swap implementations without changing the code that uses them.
