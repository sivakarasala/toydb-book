## Rust Gym

### Drill 1: Entry API

Rewrite this code to use the Entry API:

```rust
use std::collections::HashMap;

fn word_count(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        let lower = word.to_lowercase();
        if counts.contains_key(&lower) {
            let count = counts.get_mut(&lower).unwrap();
            *count += 1;
        } else {
            counts.insert(lower, 1);
        }
    }
    counts
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn word_count(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word.to_lowercase()).or_insert(0) += 1;
    }
    counts
}
```

Five lines of if/else collapse into one line. The `entry()` API handles both the "key exists" and "key does not exist" cases. `or_insert(0)` returns a mutable reference to the existing value or inserts 0 and returns a mutable reference to that.

This is the pattern our `AggregateExecutor` uses for GROUP BY — each row either creates a new group (with fresh accumulators) or updates an existing group's accumulators.

</details>

### Drill 2: Custom Sorting

Sort this Vec of tuples by the second element descending, then by the first element ascending (as a tiebreaker):

```rust
fn main() {
    let mut data = vec![
        ("Alice", 90),
        ("Bob", 85),
        ("Carol", 90),
        ("Dave", 85),
        ("Eve", 95),
    ];

    // Sort by score descending, then name ascending
    // Expected: [("Eve", 95), ("Alice", 90), ("Carol", 90), ("Bob", 85), ("Dave", 85)]
    data.sort_by(|a, b| {
        todo!()
    });

    println!("{:?}", data);
}
```

<details>
<summary>Solution</summary>

```rust
fn main() {
    let mut data = vec![
        ("Alice", 90),
        ("Bob", 85),
        ("Carol", 90),
        ("Dave", 85),
        ("Eve", 95),
    ];

    data.sort_by(|a, b| {
        // First: score descending (b.1 compared to a.1)
        b.1.cmp(&a.1)
            // Then: name ascending (a.0 compared to b.0)
            .then(a.0.cmp(&b.0))
    });

    println!("{:?}", data);
    // [("Eve", 95), ("Alice", 90), ("Carol", 90), ("Bob", 85), ("Dave", 85)]
}
```

The `.then()` method on `Ordering` chains comparisons. If the first comparison returns `Equal`, it uses the second. This is exactly how multi-column ORDER BY works in our `SortExecutor` — iterate through sort keys, use the first non-equal comparison, continue to the next key on ties.

</details>

### Drill 3: HashMap with Entry and or_insert_with

Implement a function that groups strings by their first character, where the grouping structure is `HashMap<char, Vec<String>>`:

```rust
use std::collections::HashMap;

fn group_by_first_char(words: &[&str]) -> HashMap<char, Vec<String>> {
    todo!()
}

fn main() {
    let words = &["apple", "avocado", "banana", "blueberry", "cherry"];
    let groups = group_by_first_char(words);
    println!("{:?}", groups);
    // {'a': ["apple", "avocado"], 'b': ["banana", "blueberry"], 'c': ["cherry"]}
}
```

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn group_by_first_char(words: &[&str]) -> HashMap<char, Vec<String>> {
    let mut groups: HashMap<char, Vec<String>> = HashMap::new();
    for &word in words {
        if let Some(first_char) = word.chars().next() {
            groups.entry(first_char)
                .or_insert_with(Vec::new)
                .push(word.to_string());
        }
    }
    groups
}
```

`or_insert_with(Vec::new)` is preferred over `or_insert(Vec::new())` because `or_insert_with` only calls the closure when the key is missing. With `or_insert(Vec::new())`, a new Vec is allocated on every iteration — even when the key already exists and the Vec is immediately discarded. For our `AggregateExecutor`, we use `or_insert_with(|| ...)` to avoid creating accumulators for groups that already have them.

</details>

---
