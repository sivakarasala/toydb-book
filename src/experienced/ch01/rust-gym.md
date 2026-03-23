## Rust Gym

Time for reps. These drills focus on variables, types, and HashMap — the spotlight concepts for this chapter. Do them in a Rust playground ([play.rust-lang.org](https://play.rust-lang.org)) or in a scratch `main.rs`.

### Drill 1: Type Declarations (Simple)

Declare one variable of each type and print them all in a single formatted string. Use at least: `i32`, `f64`, `String`, `&str`, `bool`, and `usize`.

```rust
fn main() {
    // Declare your variables here

    // Print them all in one line:
    // println!("int={}, float={}, owned={}, borrowed={}, flag={}, count={}", ...);
}
```

<details>
<summary>Solution</summary>

```rust
fn main() {
    let age: i32 = 25;
    let score: f64 = 98.6;
    let name: String = String::from("toydb");
    let label: &str = "database";
    let active: bool = true;
    let count: usize = 42;

    println!(
        "int={}, float={}, owned={}, borrowed={}, flag={}, count={}",
        age, score, name, label, active, count
    );
}
```

Output:
```
int=25, float=98.6, owned=toydb, borrowed=database, flag=true, count=42
```

</details>

### Drill 2: Word Frequency Counter (Medium)

Given a string, count the frequency of each word using a `HashMap<String, usize>`. Print the results sorted by frequency (highest first).

```rust
use std::collections::HashMap;

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    // Your implementation here
    todo!()
}

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let freqs = word_frequencies(text);

    // Sort by frequency (highest first), then alphabetically for ties
    let mut sorted: Vec<_> = freqs.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    for (word, count) in sorted {
        println!("{:>8}: {}", word, count);
    }
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    let mut freqs = HashMap::new();
    for word in text.split_whitespace() {
        let count = freqs.entry(word.to_string()).or_insert(0);
        *count += 1;
    }
    freqs
}

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let freqs = word_frequencies(text);

    let mut sorted: Vec<_> = freqs.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    for (word, count) in sorted {
        println!("{:>8}: {}", word, count);
    }
}
```

Output:
```
     the: 3
     fox: 2
   brown: 1
     dog: 1
   jumps: 1
    lazy: 1
    over: 1
   quick: 1
```

The key insight is `.entry().or_insert(0)`. The `entry` API gives you a mutable reference to the value at that key, inserting a default if the key does not exist. The `*count += 1` dereferences the mutable reference to increment the value. This is the idiomatic way to build frequency maps in Rust.

</details>

### Drill 3: Namespaced Key-Value Store (Advanced)

Implement a two-level `HashMap` — a namespace maps to a key-value store. Support commands like `SET users:name Alice` where `users` is the namespace and `name` is the key.

```rust
use std::collections::HashMap;

struct NamespacedDb {
    namespaces: HashMap<String, HashMap<String, String>>,
}

impl NamespacedDb {
    fn new() -> Self {
        // Your implementation
        todo!()
    }

    /// Parse "namespace:key" into (namespace, key).
    /// If no colon, use "default" as the namespace.
    fn parse_key(input: &str) -> (&str, &str) {
        // Your implementation
        todo!()
    }

    fn set(&mut self, ns: &str, key: &str, value: String) {
        // Your implementation
        todo!()
    }

    fn get(&self, ns: &str, key: &str) -> Option<&String> {
        // Your implementation
        todo!()
    }

    fn list_namespace(&self, ns: &str) -> Vec<(&String, &String)> {
        // Your implementation
        todo!()
    }

    fn list_namespaces(&self) -> Vec<&String> {
        // Your implementation
        todo!()
    }
}

fn main() {
    let mut db = NamespacedDb::new();

    db.set("users", "name", "Alice".to_string());
    db.set("users", "email", "alice@example.com".to_string());
    db.set("config", "port", "5432".to_string());
    db.set("config", "host", "localhost".to_string());

    println!("Namespaces: {:?}", db.list_namespaces());
    println!();

    for ns in db.list_namespaces() {
        println!("[{}]", ns);
        for (key, value) in db.list_namespace(ns) {
            println!("  {} = {}", key, value);
        }
    }

    println!();
    println!("users:name = {:?}", db.get("users", "name"));
    println!("config:port = {:?}", db.get("config", "port"));
    println!("missing:key = {:?}", db.get("missing", "key"));
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

struct NamespacedDb {
    namespaces: HashMap<String, HashMap<String, String>>,
}

impl NamespacedDb {
    fn new() -> Self {
        NamespacedDb {
            namespaces: HashMap::new(),
        }
    }

    fn parse_key(input: &str) -> (&str, &str) {
        match input.split_once(':') {
            Some((ns, key)) => (ns, key),
            None => ("default", input),
        }
    }

    fn set(&mut self, ns: &str, key: &str, value: String) {
        self.namespaces
            .entry(ns.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), value);
    }

    fn get(&self, ns: &str, key: &str) -> Option<&String> {
        self.namespaces.get(ns)?.get(key)
    }

    fn list_namespace(&self, ns: &str) -> Vec<(&String, &String)> {
        match self.namespaces.get(ns) {
            Some(map) => map.iter().collect(),
            None => Vec::new(),
        }
    }

    fn list_namespaces(&self) -> Vec<&String> {
        let mut ns: Vec<&String> = self.namespaces.keys().collect();
        ns.sort();
        ns
    }
}

fn main() {
    let mut db = NamespacedDb::new();

    db.set("users", "name", "Alice".to_string());
    db.set("users", "email", "alice@example.com".to_string());
    db.set("config", "port", "5432".to_string());
    db.set("config", "host", "localhost".to_string());

    println!("Namespaces: {:?}", db.list_namespaces());
    println!();

    for ns in db.list_namespaces() {
        println!("[{}]", ns);
        for (key, value) in db.list_namespace(ns) {
            println!("  {} = {}", key, value);
        }
    }

    println!();
    println!("users:name = {:?}", db.get("users", "name"));
    println!("config:port = {:?}", db.get("config", "port"));
    println!("missing:key = {:?}", db.get("missing", "key"));
}
```

Output:
```
Namespaces: ["config", "users"]

[config]
  host = localhost
  port = 5432
[users]
  email = alice@example.com
  name = Alice

users:name = Some("Alice")
config:port = Some("5432")
missing:key = None
```

Two things to notice in the `get` method: `self.namespaces.get(ns)?.get(key)`. The `?` operator on `Option` is elegant — if `get(ns)` returns `None`, the entire method returns `None` immediately. No nested `match`, no `if let`. The `?` operator propagates the "nothing" case automatically.

In `set`, the `.entry().or_insert_with(HashMap::new)` pattern creates a new inner HashMap only if the namespace does not exist yet. `or_insert_with` takes a closure (a function), unlike `or_insert` which takes a value. This means we only allocate the HashMap when actually needed.

</details>

---
