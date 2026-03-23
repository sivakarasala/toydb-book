## Rust Gym

### Drill 1: Read and Write Traits

Implement a function that writes a greeting to any `Write` destination:

```rust
use std::io::Write;

fn write_greeting<W: Write>(writer: &mut W, name: &str) -> std::io::Result<()> {
    todo!()
}

fn main() {
    // Write to stdout
    write_greeting(&mut std::io::stdout(), "Alice").unwrap();

    // Write to a Vec<u8> (in-memory buffer)
    let mut buffer: Vec<u8> = Vec::new();
    write_greeting(&mut buffer, "Bob").unwrap();
    println!("Buffer: {:?}", String::from_utf8(buffer).unwrap());
}
```

<details>
<summary>Solution</summary>

```rust
use std::io::Write;

fn write_greeting<W: Write>(writer: &mut W, name: &str) -> std::io::Result<()> {
    write!(writer, "Hello, {}!\n", name)?;
    writer.flush()
}

fn main() {
    // Write to stdout
    write_greeting(&mut std::io::stdout(), "Alice").unwrap();
    // Output: Hello, Alice!

    // Write to a Vec<u8> (in-memory buffer)
    let mut buffer: Vec<u8> = Vec::new();
    write_greeting(&mut buffer, "Bob").unwrap();
    println!("Buffer: {:?}", String::from_utf8(buffer).unwrap());
    // Output: Buffer: "Hello, Bob!\n"
}
```

Key insight: `Vec<u8>` implements `Write`. This is how we test I/O code without touching the network — write to a `Vec<u8>`, then inspect the bytes. The `write!` macro works with any `Write` implementation, just like `format!` works with strings. Our protocol tests use this pattern: write messages to a `Vec<u8>`, then read them back with a `BufReader` over a slice.

</details>

### Drill 2: Big-Endian Encoding

Convert this number to big-endian bytes and back, without using `to_be_bytes`:

```rust
fn to_big_endian(value: u32) -> [u8; 4] {
    todo!()
}

fn from_big_endian(bytes: [u8; 4]) -> u32 {
    todo!()
}

fn main() {
    let original: u32 = 1024; // 0x00000400
    let bytes = to_big_endian(original);
    println!("{:?}", bytes); // [0, 0, 4, 0]

    let recovered = from_big_endian(bytes);
    assert_eq!(original, recovered);
    println!("Round trip: {} -> {:?} -> {}", original, bytes, recovered);
}
```

<details>
<summary>Solution</summary>

```rust
fn to_big_endian(value: u32) -> [u8; 4] {
    [
        ((value >> 24) & 0xFF) as u8,  // most significant byte first
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,          // least significant byte last
    ]
}

fn from_big_endian(bytes: [u8; 4]) -> u32 {
    (bytes[0] as u32) << 24
        | (bytes[1] as u32) << 16
        | (bytes[2] as u32) << 8
        | (bytes[3] as u32)
}

fn main() {
    let original: u32 = 1024;
    let bytes = to_big_endian(original);
    println!("{:?}", bytes); // [0, 0, 4, 0]

    let recovered = from_big_endian(bytes);
    assert_eq!(original, recovered);
    println!("Round trip: {} -> {:?} -> {}", original, bytes, recovered);
    // Round trip: 1024 -> [0, 0, 4, 0] -> 1024
}
```

Big-endian means "most significant byte first" — the way humans read numbers (thousands, hundreds, tens, ones). 1024 = 0x00000400, so the bytes are [0x00, 0x00, 0x04, 0x00].

Network protocols traditionally use big-endian (also called "network byte order"). Rust's `u32::to_be_bytes()` and `u32::from_be_bytes()` do exactly what we implemented here, but are clearer and optimized by the compiler. Our protocol uses `to_be_bytes()` — this drill teaches you what it does under the hood.

</details>

### Drill 3: Error Conversion with From

Our `ProtocolError` has a `From<io::Error>` implementation. Implement `From` conversions for this custom error type:

```rust
use std::io;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound(String),
}

// Implement From<io::Error> and From<ParseIntError> for AppError
// so that ? works automatically

fn read_number_from_file(path: &str) -> Result<i64, AppError> {
    let content = std::fs::read_to_string(path)?; // needs From<io::Error>
    let number: i64 = content.trim().parse()?;      // needs From<ParseIntError>
    Ok(number)
}
```

<details>
<summary>Solution</summary>

```rust
use std::io;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound(String),
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<ParseIntError> for AppError {
    fn from(err: ParseIntError) -> Self {
        AppError::Parse(err.to_string())
    }
}

fn read_number_from_file(path: &str) -> Result<i64, AppError> {
    let content = std::fs::read_to_string(path)?;
    let number: i64 = content.trim().parse()?;
    Ok(number)
}
```

The `?` operator does two things: unwraps the `Ok` value, or converts the error using `From` and returns it. Without the `From` implementations, `?` would not compile because `io::Error` and `ParseIntError` are not `AppError`.

Our `ProtocolError` uses the same pattern. `From<io::Error>` lets us use `?` on any `io::Result` inside protocol functions, automatically wrapping the error in `ProtocolError::Io`.

</details>

---
