## Spotlight: File I/O & Error Handling

Every chapter has one spotlight concept. This chapter's spotlight is **file I/O and error handling** — how Rust reads and writes files, and how it forces you to deal with everything that can go wrong.

### Why files?

When you store data in a `BTreeMap`, it lives in RAM. RAM is fast but temporary — it loses its contents when the power goes off. A file lives on your hard drive or SSD, which keeps data even without power. The trade-off: files are slower to access than RAM, but they are durable.

> **Analogy: RAM vs Disk**
>
> RAM is like a whiteboard. It is fast to write on and fast to read, but when you erase it (or the power goes out), everything is gone.
>
> A file on disk is like a notebook. Writing is slower (you need to open the notebook, find the right page, write carefully), but the words stay there until you deliberately erase them.

### Opening files in Rust

Rust's `std::fs::File` is your handle to a file on disk:

```rust
use std::fs::File;

let file = File::open("data.log");
```

But wait — what if the file does not exist? In many languages, this would throw an exception. In Rust, `File::open` returns a `Result`:

```rust
let file: Result<File, std::io::Error> = File::open("data.log");
```

This is either `Ok(file)` (success — the file was opened) or `Err(error)` (failure — maybe the file does not exist, or you do not have permission to read it).

You handle it with `match`:

```rust
match File::open("data.log") {
    Ok(file) => println!("File opened!"),
    Err(error) => println!("Failed to open: {}", error),
}
```

### The `?` operator: error propagation made simple

Writing `match` for every file operation gets tedious fast. The `?` operator is shorthand for "if this fails, return the error immediately":

```rust
fn read_config() -> Result<String, std::io::Error> {
    let mut file = File::open("config.txt")?;   // ? = return error if this fails
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;         // ? again
    Ok(contents)
}
```

Without `?`, this function would be much longer:

```rust
fn read_config() -> Result<String, std::io::Error> {
    let mut file = match File::open("config.txt") {
        Ok(f) => f,
        Err(e) => return Err(e),
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(e) => return Err(e),
    }
    Ok(contents)
}
```

The `?` version is much cleaner. It does exactly the same thing: if the Result is `Err`, return the error immediately. If it is `Ok`, unwrap the value and continue.

> **What just happened?**
>
> The `?` operator is one of Rust's most useful features. It says "try this, and if it fails, propagate the error to my caller." It only works in functions that return `Result` (because it needs somewhere to send the error).
>
> Think of `?` as a shortcut: instead of writing 4 lines of error handling, you write one character.

### Important rule: `?` only works in functions that return `Result`

This will not compile:

```rust
fn main() {
    let file = File::open("data.log")?;  // ERROR: main does not return Result
}
```

Fix: make `main` return a `Result`, or use `match`/`unwrap` instead:

```rust
fn main() -> Result<(), std::io::Error> {
    let file = File::open("data.log")?;  // OK now
    Ok(())
}
```

### OpenOptions: fine-grained file control

`File::open` opens a file for reading only. `File::create` creates a new file (or truncates an existing one) for writing only. But databases need more control — we want to both read and write, create the file if it does not exist, and append to it instead of overwriting:

```rust
use std::fs::OpenOptions;

let file = OpenOptions::new()
    .read(true)       // allow reading
    .write(true)      // allow writing
    .create(true)     // create if it does not exist
    .append(true)     // all writes go to the end
    .open("data.log")?;
```

Each method call configures one aspect of how the file is opened. This is called the **builder pattern** — a common Rust idiom for constructing things with many options.

> **Analogy: Builder Pattern = Ordering a sandwich**
>
> When you order a sandwich, you do not say everything at once. You say: "I want wheat bread" (`.bread(wheat)`), "add turkey" (`.meat(turkey)`), "add lettuce" (`.lettuce(true)`), "toast it" (`.toasted(true)`). Each step configures one option. At the end, you say "make it" (`.build()`). The builder pattern works the same way.

### Writing to files

Once you have a file handle, you can write bytes to it:

```rust
use std::io::Write;

let mut file = File::create("output.txt")?;
file.write_all(b"Hello, world!")?;
```

The `write_all` method writes all the bytes in the slice to the file. The `b"..."` prefix creates a byte string (`&[u8]`).

### Buffered I/O

Every `write_all` call is a **system call** — a request to the operating system to perform the write. System calls are relatively expensive. If you write one byte at a time, each byte costs a full round trip to the OS kernel.

`BufWriter` batches small writes into one big write:

```rust
use std::io::BufWriter;

let file = File::create("output.txt")?;
let mut writer = BufWriter::new(file);
writer.write_all(b"Hello")?;     // buffered — may not hit disk yet
writer.write_all(b", world!")?;  // buffered
writer.flush()?;                  // NOW it hits disk — one write instead of two
```

> **Analogy: Buffered I/O = Batch mailing**
>
> Instead of walking to the mailbox for each letter, you collect all the letters on your desk and take them all at once. `BufWriter` is your desk — it collects writes and sends them in one batch.

### Custom error types

Real applications have multiple kinds of errors. For our storage engine:
- The file might not exist (I/O error)
- A record might have corrupted data (data integrity error)
- A key might not be found (application logic error)

We model these with an enum:

```rust
#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    CorruptedRecord { offset: u64, message: String },
    KeyNotFound(String),
}
```

Each variant carries different context. `Io` wraps the standard library's `io::Error`. `CorruptedRecord` carries the file offset and a description. `KeyNotFound` carries the key name.

### The `From` trait: automatic error conversion

To use `?` with our custom error type, we need to tell Rust how to convert `std::io::Error` into `StorageError`:

```rust
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}
```

Now when a file operation returns `Err(io::Error)`, the `?` operator automatically converts it to `Err(StorageError::Io(e))`. This is why `?` is so powerful — it handles both the error propagation and the type conversion in one character.

> **What just happened?**
>
> The `From` trait is a standard Rust trait that says "I know how to create myself from type X." By implementing `From<io::Error> for StorageError`, we taught Rust how to convert I/O errors into our custom error type. The `?` operator uses this conversion automatically.

### Common mistakes with error handling

**Mistake: Using `unwrap()` in non-test code**

```rust
let file = File::open("data.log").unwrap();  // Crashes if file is missing!
```

`unwrap()` panics (crashes) on errors. Use it in tests and prototypes, but in production code, use `?` or `match`.

**Mistake: Forgetting to `flush()` a BufWriter**

```rust
let mut writer = BufWriter::new(file);
writer.write_all(b"important data")?;
// Forgot flush! Data might be lost if the program crashes.
```

Always call `flush()` after writing data you cannot afford to lose.

**Mistake: Using `?` in a function that returns `()`**

```rust
fn save_data() {  // Returns () — no Result!
    let file = File::open("data.log")?;  // ERROR: ? requires Result return type
}
```

Fix: change the return type to `Result`:

```rust
fn save_data() -> Result<(), StorageError> {
    let file = File::open("data.log")?;  // OK now
    Ok(())
}
```

---
