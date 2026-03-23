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
