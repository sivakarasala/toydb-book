## Spotlight: Advanced Iterators

Every chapter has one spotlight concept. This chapter's spotlight is **advanced iterators** — moving beyond basic `for` loops and `.map()` chains into custom iterator implementations, composition patterns, and the design philosophy behind Rust's iterator system.

### Review: the Iterator trait

You have seen Rust's `Iterator` trait before:

```rust
pub trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

One method. One associated type. That is the entire contract. Everything else — `map`, `filter`, `take`, `chain`, `collect`, `fold`, `zip`, `enumerate`, `peekable` — is built on top of this single method. The standard library provides over 70 adaptor methods on `Iterator`, all implemented in terms of `next()`.

### Custom iterators: beyond Vec and slices

Most Rust iterators you have used come from collections — `vec.iter()`, `map.keys()`, `string.chars()`. But any struct can be an iterator. All it needs is `impl Iterator` with a `next()` method:

```rust
/// An iterator that counts from a start value, incrementing by a step.
struct StepCounter {
    current: i64,
    step: i64,
    end: i64,
}

impl StepCounter {
    fn new(start: i64, end: i64, step: i64) -> Self {
        StepCounter { current: start, step, end }
    }
}

impl Iterator for StepCounter {
    type Item = i64;

    fn next(&mut self) -> Option<i64> {
        if self.current >= self.end {
            return None;
        }
        let value = self.current;
        self.current += self.step;
        Some(value)
    }
}

fn main() {
    let evens: Vec<i64> = StepCounter::new(0, 10, 2).collect();
    println!("{:?}", evens); // [0, 2, 4, 6, 8]
}
```

The iterator holds mutable state (`current`) and produces values on demand. Nothing is computed until `next()` is called. This is exactly how our database executors will work — each executor holds state (a position in a table, a buffer of rows, a child executor) and produces rows one at a time.

### Composing iterators: the adaptor pattern

The power of iterators comes from composition. Each adaptor wraps an existing iterator and transforms its output:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let result: Vec<i64> = numbers.iter()
    .filter(|&&n| n % 2 == 0)      // keep even numbers
    .map(|&n| n * n)                // square each one
    .take(3)                         // stop after 3 results
    .collect();                      // gather into a Vec

println!("{:?}", result); // [4, 16, 36]
```

This pipeline does NOT:
1. Filter all 10 numbers, creating a temporary Vec of evens
2. Square all evens, creating another temporary Vec
3. Take 3 from the squared Vec

Instead, it pulls one number at a time through the entire chain. `take(3)` calls `map.next()`, which calls `filter.next()`, which calls `iter.next()`. When `take` has accumulated 3 results, it stops — numbers 7 through 10 are never even examined. This is lazy evaluation, and it is exactly how the Volcano model works.

### Peekable: looking ahead without consuming

Sometimes you need to look at the next value without advancing the iterator. `Peekable` wraps any iterator and adds a `peek()` method:

```rust
let mut iter = vec![1, 2, 3].into_iter().peekable();

// Look at the next value without consuming it
assert_eq!(iter.peek(), Some(&1));
assert_eq!(iter.peek(), Some(&1)); // still 1 — peek does not advance

// Now consume it
assert_eq!(iter.next(), Some(1));
assert_eq!(iter.peek(), Some(&2)); // now it's 2
```

This is useful when parsing or processing sequences where decisions depend on the upcoming value. Our SQL lexer used `peek()` in Chapter 6 to decide whether `>` is `GreaterThan` or `>=` is `GreaterOrEqual`.

### Chain: concatenating iterators

`chain` links two iterators end-to-end:

```rust
let first = vec![1, 2, 3];
let second = vec![4, 5, 6];

let all: Vec<i32> = first.into_iter().chain(second).collect();
println!("{:?}", all); // [1, 2, 3, 4, 5, 6]
```

In a database, you might use `chain` to implement `UNION` — appending the results of one query to another.

### Lazy vs eager: why it matters for databases

Consider a table with 10 million rows and a query that selects the first 5 rows matching a condition:

```sql
SELECT name FROM users WHERE age > 30 LIMIT 5;
```

**Eager evaluation** (what Python/JavaScript do by default): scan all 10 million rows, filter down to matching rows (maybe 3 million), then take 5. You touched 10 million rows to produce 5.

**Lazy evaluation** (the Volcano model): pull one row from the scan, check the filter, if it passes, add to results. Repeat until you have 5 results. If the first 5 matching rows are in the first 100 rows of the table, you only read 100 rows — not 10 million.

This is why every production database uses an iterator-based (pull) model. The operator at the top of the tree controls how many rows flow through the system. A `LIMIT 5` at the top means the entire pipeline stops after producing 5 rows, regardless of how large the table is.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Custom iterator | `[Symbol.iterator]() { return { next() {} } }` | `__iter__` + `__next__` | No built-in; use channels | `impl Iterator` |
> | Lazy chain | None built-in (arrays are eager) | Generators (`yield`) | Channels | `.filter().map().take()` |
> | Peek ahead | No built-in | `itertools.peekable()` | No built-in | `.peekable()` |
> | Collect results | `Array.from(iter)` | `list(iter)` | `for range` into slice | `.collect::<Vec<_>>()` |
> | Early termination | `.find()` stops | `next(filter(...))` | `break` in loop | `.take(n)`, `.find()` |
>
> The biggest difference: Rust's iterator adaptors are zero-cost abstractions. The compiler inlines the entire chain of `.filter().map().take()` into a single loop with no intermediate allocations, no virtual dispatch, no heap-allocated closures. The generated machine code is identical to a hand-written `for` loop with `if` statements. JavaScript and Python iterators carry per-element overhead from function calls and dynamic dispatch.

---
