## Rust Gym

### Drill 1: Derive Debug, Clone, PartialEq

Given this struct, add the necessary derives so the code compiles:

```rust,ignore
struct Config {
    host: String,
    port: u16,
    max_connections: usize,
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 5432,
        max_connections: 100,
    };

    let backup = config.clone();          // needs Clone
    println!("{:?}", config);             // needs Debug
    assert_eq!(config, backup);           // needs PartialEq
}
```

<details>
<summary>Hint: What derives do you need?</summary>

You need three derives -- one for each operation used in `main()`. Look at the comments next to each line.

</details>

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
struct Config {
    host: String,
    port: u16,
    max_connections: usize,
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 5432,
        max_connections: 100,
    };

    let backup = config.clone();
    println!("{:?}", config);
    assert_eq!(config, backup);
}
```

All three derives work because every field type (`String`, `u16`, `usize`) already implements `Debug`, `Clone`, and `PartialEq`. If any field did not implement a trait, the derive would fail with a compiler error pointing at the offending field.

</details>

### Drill 2: Serialize a Struct to JSON and Back

Add `serde_json = "1"` to your dependencies. Add the right derives to make this code compile and run:

```rust,ignore
use serde::{Serialize, Deserialize};

struct Address {
    street: String,
    city: String,
    zip: String,
}

struct Person {
    name: String,
    age: u32,
    address: Address,
}

fn main() {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            zip: "62701".to_string(),
        },
    };

    let json = serde_json::to_string_pretty(&person).unwrap();
    println!("{}", json);

    let decoded: Person = serde_json::from_str(&json).unwrap();
    assert_eq!(person.name, decoded.name);
    assert_eq!(person.address.city, decoded.address.city);
}
```

<details>
<summary>Hint: Both structs need derives</summary>

Serde needs to serialize `Address` as part of `Person`. That means both `Address` and `Person` need `#[derive(Serialize, Deserialize)]`. If you only derive on `Person`, the compile will fail because `address` is an `Address` field that serde does not know how to serialize.

</details>

<details>
<summary>Solution</summary>

```rust,ignore
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    address: Address,
}

fn main() {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            zip: "62701".to_string(),
        },
    };

    let json = serde_json::to_string_pretty(&person).unwrap();
    println!("{}", json);

    let decoded: Person = serde_json::from_str(&json).unwrap();
    assert_eq!(person.name, decoded.name);
    assert_eq!(person.address.city, decoded.address.city);
}
```

Output:

```json
{
  "name": "Alice",
  "age": 30,
  "address": {
    "street": "123 Main St",
    "city": "Springfield",
    "zip": "62701"
  }
}
```

Serde handles nested structs automatically -- it recursively serializes each field. If `Address` did not derive `Serialize`, the `Person` derive would fail because the macro would not know how to serialize the `address` field.

</details>

### Drill 3: Manual `to_bytes` / `from_bytes`

Without using serde, implement `to_bytes()` and `from_bytes()` for this struct:

```rust,ignore
struct Measurement {
    sensor_id: u16,
    timestamp: u64,
    value: f32,
}
```

The format should be: `[sensor_id: 2 bytes LE][timestamp: 8 bytes LE][value: 4 bytes LE]` -- exactly 14 bytes total.

<details>
<summary>Hint: Use `to_le_bytes()` for each field</summary>

Each numeric type has `to_le_bytes()` that returns a fixed-size array: `u16` returns `[u8; 2]`, `u64` returns `[u8; 8]`, `f32` returns `[u8; 4]`. Use `copy_from_slice` to write each into the right position in a `[u8; 14]` output array.

</details>

<details>
<summary>Solution</summary>

```rust
struct Measurement {
    sensor_id: u16,
    timestamp: u64,
    value: f32,
}

impl Measurement {
    fn to_bytes(&self) -> [u8; 14] {
        let mut buf = [0u8; 14];
        buf[0..2].copy_from_slice(&self.sensor_id.to_le_bytes());
        buf[2..10].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[10..14].copy_from_slice(&self.value.to_le_bytes());
        buf
    }

    fn from_bytes(bytes: &[u8; 14]) -> Self {
        Measurement {
            sensor_id: u16::from_le_bytes([bytes[0], bytes[1]]),
            timestamp: u64::from_le_bytes(bytes[2..10].try_into().unwrap()),
            value: f32::from_le_bytes(bytes[10..14].try_into().unwrap()),
        }
    }
}

#[test]
fn measurement_round_trip() {
    let m = Measurement {
        sensor_id: 42,
        timestamp: 1700000000,
        value: 23.5,
    };
    let bytes = m.to_bytes();
    assert_eq!(bytes.len(), 14);
    let decoded = Measurement::from_bytes(&bytes);
    assert_eq!(m.sensor_id, decoded.sensor_id);
    assert_eq!(m.timestamp, decoded.timestamp);
    assert_eq!(m.value, decoded.value);
}
```

Using `[u8; 14]` instead of `Vec<u8>` as the return type communicates the fixed size at the type level. The caller knows exactly how many bytes to expect.

</details>

---
