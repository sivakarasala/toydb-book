## Spotlight: Collections & Algorithms

Every chapter has one spotlight concept. This chapter's spotlight is **collections and algorithms** — the standard library data structures that power joins, aggregations, and sorting, and the Rust idioms that make working with them ergonomic.

### HashMap: O(1) lookup by key

You have used `HashMap` before (storage tables in Chapter 2, aggregation in the optimizer). For joins and GROUP BY, `HashMap` is the critical data structure:

```rust
use std::collections::HashMap;

let mut word_count: HashMap<String, usize> = HashMap::new();
let words = vec!["hello", "world", "hello", "rust", "hello"];

for word in words {
    let count = word_count.entry(word.to_string()).or_insert(0);
    *count += 1;
}

println!("{:?}", word_count);
// {"hello": 3, "world": 1, "rust": 1}
```

### The Entry API: ergonomic insert-or-update

The `entry()` method returns an `Entry` enum — either `Occupied` (key exists) or `Vacant` (key does not). This eliminates the awkward get-then-insert pattern:

```rust
// Without Entry API (awkward)
if let Some(count) = map.get_mut("hello") {
    *count += 1;
} else {
    map.insert("hello".to_string(), 1);
}

// With Entry API (idiomatic)
*map.entry("hello".to_string()).or_insert(0) += 1;

// Even more powerful: or_insert_with for expensive defaults
map.entry(key)
    .or_insert_with(|| compute_default_value())
    .update();
```

For aggregation, the Entry API is essential. When processing GROUP BY, each row either starts a new group or updates an existing group. The Entry API handles both cases in one expression.

### BTreeMap: sorted keys

`BTreeMap` has the same API as `HashMap` but keeps keys sorted:

```rust
use std::collections::BTreeMap;

let mut scores: BTreeMap<String, i64> = BTreeMap::new();
scores.insert("Carol".to_string(), 95);
scores.insert("Alice".to_string(), 87);
scores.insert("Bob".to_string(), 92);

// Iteration is in key order
for (name, score) in &scores {
    println!("{}: {}", name, score);
}
// Alice: 87
// Bob: 92
// Carol: 95
```

`HashMap` has O(1) average lookup; `BTreeMap` has O(log n). Use `HashMap` when you need speed, `BTreeMap` when you need ordering. For GROUP BY, we typically use `HashMap` (we do not need groups in any particular order). For ORDER BY on the grouping key, `BTreeMap` gives us sorted output for free.

### Vec and sorting with custom comparators

`Vec::sort_by` takes a closure that compares two elements:

```rust
let mut people = vec![
    ("Alice", 30),
    ("Bob", 25),
    ("Carol", 35),
];

// Sort by age descending
people.sort_by(|a, b| b.1.cmp(&a.1));
// [("Carol", 35), ("Alice", 30), ("Bob", 25)]
```

For our `SortExecutor`, we need to sort `Row`s by arbitrary expressions. The comparator must evaluate the sort expression for each row and compare the results.

### Custom Ord: making types sortable

Rust's sort functions require `Ord` — a total ordering. For our `Value` enum, we need to define how values compare:

```rust
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            (Value::Null, _) => Some(std::cmp::Ordering::Less), // NULL sorts first
            (_, Value::Null) => Some(std::cmp::Ordering::Greater),
            // Mixed types — not comparable
            _ => None,
        }
    }
}
```

The NULL handling matters. SQL defines that NULLs sort either first or last (configurable with `NULLS FIRST` / `NULLS LAST`). We choose NULLs-first as the default, which is PostgreSQL's behavior for ascending sorts.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Hash map | `Map` or `{}` | `dict` | `map[K]V` | `HashMap<K, V>` |
> | Sorted map | No built-in | No built-in (use `sortedcontainers`) | No built-in | `BTreeMap<K, V>` |
> | Insert-or-update | `map[k] = (map[k] ?? 0) + 1` | `d[k] = d.get(k, 0) + 1` | `m[k] += 1` (zero default) | `*entry.or_insert(0) += 1` |
> | Custom sort | `.sort((a, b) => a - b)` | `.sort(key=lambda x: x.age)` | `sort.Slice(s, func(i, j int) bool { ... })` | `.sort_by(\|a, b\| a.cmp(b))` |
> | Type-safe keys | No (any key type) | Hashable keys | Comparable keys | `Hash + Eq` for HashMap, `Ord` for BTreeMap |
>
> The biggest difference: Rust's `HashMap` requires keys to implement `Hash + Eq`. You cannot use `f64` as a HashMap key because floating-point equality is unreliable (NaN != NaN). Python and JavaScript silently accept this and produce bugs. Rust refuses to compile it. For database values, this means we need to handle the `Float` case carefully — either by not supporting it as a group-by key, or by wrapping it in a type that implements `Hash`.

---
