## Spotlight: File I/O & Error Handling

Every chapter has one spotlight concept. This chapter's spotlight is **file I/O and error handling** — how Rust reads and writes files, and how it forces you to deal with everything that can go wrong.

### Opening and creating files

Rust's `std::fs::File` is your handle to a file on disk. The simplest way to open one:

```rust
use std::fs::File;

let file = File::open("data.log")?;       // read-only, fails if missing
let file = File::create("data.log")?;      // write-only, truncates if exists
```

But databases need more control. `OpenOptions` lets you specify exactly what you want:

```rust
use std::fs::OpenOptions;

let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)       // create if missing, keep contents if exists
    .append(true)       // all writes go to end of file
    .open("data.log")?;
```

Each method call returns the same `OpenOptions` builder, letting you chain them. This is the **builder pattern** — common in Rust when a constructor has many optional parameters.

### Buffered I/O

Every `write()` call to a `File` is a system call — a round trip to the operating system kernel. Writing one record at a time is like mailing one letter at a time instead of batching them. Rust provides `BufWriter` and `BufReader` to batch I/O operations:

```rust
use std::io::{BufWriter, BufReader, Write, Read, Seek};

let file = File::create("data.log")?;
let mut writer = BufWriter::new(file);
writer.write_all(b"hello")?;   // buffered — may not hit disk yet
writer.flush()?;                // forces the buffer to disk
```

`BufWriter` collects small writes into an internal buffer (default 8 KB) and flushes them in one system call when the buffer is full or when you call `flush()`. `BufReader` does the same for reads — it reads a large chunk from disk and serves small `read()` calls from the buffer.

### The `?` operator: error propagation

Notice those `?` marks at the end of file operations. Every I/O operation in Rust can fail — the file might not exist, the disk might be full, permissions might be wrong. Rust does not use exceptions. Instead, functions return `Result<T, E>`:

```rust
enum Result<T, E> {
    Ok(T),    // success — contains the value
    Err(E),   // failure — contains the error
}
```

The `?` operator is syntactic sugar for "if this is an error, return it immediately; if it is OK, unwrap the value":

```rust
// These two are equivalent:
let file = File::open("data.log")?;

let file = match File::open("data.log") {
    Ok(f) => f,
    Err(e) => return Err(e.into()),
};
```

The `?` operator does one more thing: it calls `.into()` on the error, which converts it to the error type your function returns. This means you can use `?` with different error types as long as they can convert into your function's error type. We will use this shortly with custom errors.

### Custom error types

Real applications have multiple kinds of errors — I/O errors, data corruption, missing keys. Rust models these with enums:

```rust
use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    CorruptedRecord { offset: u64, message: String },
    KeyNotFound(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "I/O error: {}", e),
            StorageError::CorruptedRecord { offset, message } =>
                write!(f, "corrupted record at offset {}: {}", offset, message),
            StorageError::KeyNotFound(key) =>
                write!(f, "key not found: {}", key),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}
```

That `From<std::io::Error>` impl is the bridge that makes `?` work. When a `File::open()` returns `Err(io::Error)`, the `?` operator calls `StorageError::from(e)` to convert it into your custom error type. No try/catch, no error swallowing — every error path is explicit and type-checked.

> **Coming from JS?**
>
> JavaScript uses `try/catch` with untyped exceptions — any value can be thrown, and you discover what went wrong at runtime:
>
> ```javascript
> try {
>   const data = fs.readFileSync("data.log");
> } catch (e) {
>   // e could be anything — no guarantee it has .code or .message
>   console.error(e);
> }
> ```
>
> Rust's `Result<T, E>` makes the error type part of the function signature. You cannot call `File::open()` without handling the possible `io::Error` — the compiler refuses to compile code that ignores a `Result`.

> **Coming from Python?**
>
> Python's `try/except` is similar to JavaScript — exceptions are untyped by default, and you learn what they are by reading docs or catching `Exception`:
>
> ```python
> try:
>     with open("data.log") as f:
>         data = f.read()
> except FileNotFoundError:
>     pass  # handle it
> except PermissionError:
>     pass  # handle it differently
> ```
>
> Rust's `match` on `Result` is the same pattern, but enforced by the compiler. You cannot accidentally forget a case because the `match` must be exhaustive.

> **Coming from Go?**
>
> Go is the closest to Rust — explicit error returns instead of exceptions:
>
> ```go
> file, err := os.Open("data.log")
> if err != nil {
>     return err
> }
> ```
>
> Rust's `?` operator is like Go's `if err != nil { return err }` compressed into a single character. But Rust adds type safety: the error type is part of the function signature, and the `From` trait handles conversions. Go's `error` interface is stringly-typed — you compare `err.Error()` strings or use `errors.Is()`. Rust's error enums let you pattern match on specific variants at compile time.

---
