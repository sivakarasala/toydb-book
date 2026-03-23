## Spotlight: Collections & Algorithms

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **collections and algorithms** -- the standard library data structures that power joins, aggregations, and sorting.

### HashMap: finding things fast

A `HashMap` stores key-value pairs and lets you look up a value by its key in O(1) average time. Think of a phone book: given a name (key), you can quickly find the phone number (value).

```rust
use std::collections::HashMap;

fn main() {
    let mut ages: HashMap<String, i32> = HashMap::new();

    // Insert key-value pairs
    ages.insert("Alice".to_string(), 30);
    ages.insert("Bob".to_string(), 25);
    ages.insert("Carol".to_string(), 35);

    // Look up a value by key
    if let Some(age) = ages.get("Alice") {
        println!("Alice is {} years old", age);  // "Alice is 30 years old"
    }

    // Check if a key exists
    println!("{}", ages.contains_key("Dave"));  // false
}
```

For our database, HashMap is critical in two places:

1. **Hash joins**: Build a HashMap from one table's join column. For each row in the other table, look up the matching rows in O(1) instead of scanning the entire first table.

2. **GROUP BY**: Use a HashMap where the key is the group (e.g., department name) and the value is the accumulated result (count, sum, etc.).

### The Entry API: insert or update in one step

When processing GROUP BY, each row either starts a new group or updates an existing one. The naive approach is awkward:

```rust,ignore
// Without Entry API -- clunky
if let Some(count) = map.get_mut("engineering") {
    *count += 1;
} else {
    map.insert("engineering".to_string(), 1);
}
```

The Entry API does this in one step:

```rust
use std::collections::HashMap;

fn main() {
    let mut word_count: HashMap<String, i32> = HashMap::new();
    let words = vec!["hello", "world", "hello", "rust", "hello"];

    for word in words {
        // entry() returns an Entry -- either Occupied or Vacant
        // or_insert(0) sets the value to 0 if the key is new
        // The * dereferences the mutable reference to add 1
        *word_count.entry(word.to_string()).or_insert(0) += 1;
    }

    println!("{:?}", word_count);
    // {"hello": 3, "world": 1, "rust": 1}
}
```

Let us break down `*word_count.entry(word).or_insert(0) += 1`:

1. `word_count.entry(word)` -- look up the key. Returns an `Entry` enum.
2. `.or_insert(0)` -- if the key is not in the map, insert it with value 0. Either way, return a mutable reference to the value.
3. `*... += 1` -- dereference the mutable reference and add 1.

This pattern is essential for aggregation. When processing GROUP BY, each row either starts a new group (Vacant entry) or updates an existing one (Occupied entry).

> **What just happened?**
>
> The Entry API solves the "get or insert" problem in one step. Without it, you need to look up the key twice -- once to check if it exists, and once to insert or update. The Entry API does one lookup and gives you a mutable reference to work with. Think of it like checking in to a hotel: if your room is ready (Occupied), you go there directly. If not (Vacant), the front desk assigns you one and then you go there.

### Vec and sorting with custom comparators

`Vec::sort_by` lets you sort using any comparison logic you want:

```rust
fn main() {
    let mut people = vec![
        ("Alice", 30),
        ("Bob", 25),
        ("Carol", 35),
    ];

    // Sort by age (ascending)
    people.sort_by(|a, b| a.1.cmp(&b.1));
    println!("{:?}", people);
    // [("Bob", 25), ("Alice", 30), ("Carol", 35)]

    // Sort by age (descending)
    people.sort_by(|a, b| b.1.cmp(&a.1));
    println!("{:?}", people);
    // [("Carol", 35), ("Alice", 30), ("Bob", 25)]
}
```

The closure `|a, b| a.1.cmp(&b.1)` takes two elements and returns an `Ordering`:
- `Ordering::Less` -- `a` should come before `b`
- `Ordering::Equal` -- `a` and `b` are equivalent
- `Ordering::Greater` -- `a` should come after `b`

The `.cmp()` method is from the `Ord` trait. For descending order, swap `a` and `b`: `b.1.cmp(&a.1)`.

### Ord and PartialOrd: making types sortable

To sort values, Rust needs to know how to compare them. This is done through two traits:

- **`PartialOrd`** -- types that can sometimes be compared (floating-point numbers cannot always be compared because `NaN != NaN`)
- **`Ord`** -- types that can always be compared (integers, strings, etc.)

For our `Value` enum, we need to define comparison:

```rust,ignore
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            (Value::Null, _) => Some(std::cmp::Ordering::Less),  // NULL sorts first
            (_, Value::Null) => Some(std::cmp::Ordering::Greater),
            _ => None,  // Different types cannot be compared
        }
    }
}
```

The `partial_cmp` returns `Option<Ordering>` because some comparisons are not possible (e.g., comparing an Integer to a String). When it returns `None`, the comparison is undefined.

We put NULLs first (before all other values). This is how PostgreSQL handles NULLs in ascending sorts.

> **What just happened?**
>
> `PartialOrd` tells Rust how to compare two values of our `Value` type. Integers are compared as numbers, strings alphabetically, booleans as false < true, and NULLs always sort first. Mixed types (like comparing an Integer to a String) return `None`, meaning they cannot be compared.

---
