## Rust Gym

Time for reps. These drills focus on file I/O and error handling — the spotlight concept for this chapter.

### Drill 1: Line Counter

Read a file line by line, count the total lines and total bytes. Use `BufReader` and `lines()`.

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

fn count_file(path: &str) -> Result<(usize, usize), std::io::Error> {
    // Your code here
    // Return (line_count, byte_count)
    todo!()
}

fn main() {
    // Create a test file first
    std::fs::write("/tmp/drill1.txt", "hello\nworld\nrust\n").unwrap();

    match count_file("/tmp/drill1.txt") {
        Ok((lines, bytes)) => println!("{} lines, {} bytes", lines, bytes),
        Err(e) => println!("Error: {}", e),
    }
    // Expected: 3 lines, 17 bytes
}
```

<details>
<summary>Solution</summary>

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

fn count_file(path: &str) -> Result<(usize, usize), std::io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut line_count = 0;
    let mut byte_count = 0;

    for line in reader.lines() {
        let line = line?;          // each line can fail — handle with ?
        byte_count += line.len() + 1; // +1 for the newline character
        line_count += 1;
    }

    Ok((line_count, byte_count))
}

fn main() {
    std::fs::write("/tmp/drill1.txt", "hello\nworld\nrust\n").unwrap();

    match count_file("/tmp/drill1.txt") {
        Ok((lines, bytes)) => println!("{} lines, {} bytes", lines, bytes),
        Err(e) => println!("Error: {}", e),
    }
}
```

Note that `reader.lines()` returns an iterator of `Result<String, io::Error>` — each line read can independently fail. The `?` inside the loop converts each line's `Result` into the function's return type. This is fundamentally different from Python's `for line in file:`, which silently ignores encoding errors by default.

</details>

### Drill 2: Custom Error Type

Write a `ConfigError` enum with three variants: `FileNotFound(String)`, `ParseError { line: usize, message: String }`, and `MissingKey(String)`. Implement `Display` and `Error` manually (no derive macro libraries). Then write a `load_config()` function that returns `Result<HashMap<String, String>, ConfigError>` and parses a simple `key=value` config file.

```rust
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
enum ConfigError {
    // Your variants here
}

// Implement Display and Error traits

fn load_config(path: &str) -> Result<HashMap<String, String>, ConfigError> {
    // Your code here
    // Parse lines like "key=value", error on malformed lines
    todo!()
}

fn main() {
    std::fs::write("/tmp/drill2.conf", "host=localhost\nport=5432\nname=toydb\n").unwrap();

    match load_config("/tmp/drill2.conf") {
        Ok(config) => {
            println!("host = {}", config.get("host").unwrap());
            println!("port = {}", config.get("port").unwrap());
        }
        Err(e) => println!("Config error: {}", e),
    }
    // Expected:
    // host = localhost
    // port = 5432
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
enum ConfigError {
    FileNotFound(String),
    ParseError { line: usize, message: String },
    MissingKey(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileNotFound(path) =>
                write!(f, "config file not found: {}", path),
            ConfigError::ParseError { line, message } =>
                write!(f, "parse error on line {}: {}", line, message),
            ConfigError::MissingKey(key) =>
                write!(f, "missing required key: {}", key),
        }
    }
}

impl std::error::Error for ConfigError {}

fn load_config(path: &str) -> Result<HashMap<String, String>, ConfigError> {
    let file = File::open(path).map_err(|_| ConfigError::FileNotFound(path.to_string()))?;
    let reader = BufReader::new(file);
    let mut config = HashMap::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| ConfigError::ParseError {
            line: i + 1,
            message: e.to_string(),
        })?;

        let line = line.trim().to_string();
        if line.is_empty() || line.starts_with('#') {
            continue; // skip blank lines and comments
        }

        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(ConfigError::ParseError {
                line: i + 1,
                message: format!("expected key=value, got: {}", line),
            });
        }

        config.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
    }

    Ok(config)
}

fn main() {
    std::fs::write("/tmp/drill2.conf", "host=localhost\nport=5432\nname=toydb\n").unwrap();

    match load_config("/tmp/drill2.conf") {
        Ok(config) => {
            println!("host = {}", config.get("host").unwrap());
            println!("port = {}", config.get("port").unwrap());
        }
        Err(e) => println!("Config error: {}", e),
    }
}
```

The key technique is `map_err()` — it transforms one error type into another. `File::open()` returns `io::Error`, but our function returns `ConfigError`. The `map_err()` call converts the `io::Error` into a `ConfigError::FileNotFound`. This is the manual alternative to implementing `From<io::Error>` — useful when the conversion needs extra context (like the file path).

</details>

### Drill 3: Write-Ahead Log

Build a simple write-ahead log (WAL) that records `set` and `delete` operations. On restart, replay the log to reconstruct state.

```rust
use std::collections::HashMap;

struct Wal {
    // Your fields here
}

impl Wal {
    fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Open file, replay existing log
        todo!()
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Write "SET key value\n" to log, update state
        todo!()
    }

    fn delete(&mut self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Write "DEL key\n" to log, update state
        todo!()
    }

    fn get(&self, key: &str) -> Option<&str> {
        // Lookup in current state
        todo!()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/drill3.wal";
    let _ = std::fs::remove_file(path);

    let mut wal = Wal::new(path)?;
    wal.set("name", "ToyDB")?;
    wal.set("version", "0.1")?;
    wal.delete("version")?;
    println!("name = {:?}", wal.get("name"));       // Some("ToyDB")
    println!("version = {:?}", wal.get("version")); // None
    drop(wal);

    // Reopen — should replay the log
    let wal2 = Wal::new(path)?;
    println!("after restart: name = {:?}", wal2.get("name")); // Some("ToyDB")
    println!("after restart: version = {:?}", wal2.get("version")); // None
    Ok(())
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

struct Wal {
    file: File,
    state: HashMap<String, String>,
}

impl Wal {
    fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let mut state = HashMap::new();

        // Replay existing log
        let reader = BufReader::new(&file);
        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            match parts.as_slice() {
                ["SET", key, value] => {
                    state.insert(key.to_string(), value.to_string());
                }
                ["DEL", key] => {
                    state.remove(*key);
                }
                _ => {} // skip malformed lines
            }
        }

        // Reopen for appending (seek to end)
        let file = OpenOptions::new()
            .append(true)
            .open(path)?;

        Ok(Wal { file, state })
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(self.file, "SET {} {}", key, value)?;
        self.file.sync_data()?;
        self.state.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(self.file, "DEL {}", key)?;
        self.file.sync_data()?;
        self.state.remove(key);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.as_str())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/drill3.wal";
    let _ = std::fs::remove_file(path);

    let mut wal = Wal::new(path)?;
    wal.set("name", "ToyDB")?;
    wal.set("version", "0.1")?;
    wal.delete("version")?;
    println!("name = {:?}", wal.get("name"));
    println!("version = {:?}", wal.get("version"));
    drop(wal);

    let wal2 = Wal::new(path)?;
    println!("after restart: name = {:?}", wal2.get("name"));
    println!("after restart: version = {:?}", wal2.get("version"));
    Ok(())
}
```

This text-based WAL is simpler than our binary `LogStorage` but demonstrates the same principle: append operations to a log, replay them on startup. The binary format is more efficient (no parsing overhead, no delimiter escaping issues), but the text format is easier to debug — you can `cat` the file and read it.

</details>

---
