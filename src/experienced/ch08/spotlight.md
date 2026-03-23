## Spotlight: Iterators & Closures

Every chapter has one spotlight concept. This chapter's spotlight is **iterators and closures** — the feature that makes Rust's data processing feel functional while remaining zero-cost.

### The Iterator trait

At its core, Rust's iterator system is a single trait with a single required method:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

That is the entire contract. Call `next()`, and you get `Some(value)` if there is more data, or `None` when the iterator is exhausted. Every `for` loop in Rust is syntactic sugar for calling `next()` until `None`:

```rust
let names = vec!["Alice", "Bob", "Carol"];

// This:
for name in &names {
    println!("{}", name);
}

// Is equivalent to:
let mut iter = names.iter();
while let Some(name) = iter.next() {
    println!("{}", name);
}
```

### Creating custom iterators

Any type can become iterable by implementing `Iterator`. Here is a counter that counts from a start value up to (but not including) an end value:

```rust
struct Counter {
    current: u32,
    end: u32,
}

impl Counter {
    fn new(start: u32, end: u32) -> Self {
        Counter { current: start, end }
    }
}

impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.current < self.end {
            let value = self.current;
            self.current += 1;
            Some(value)
        } else {
            None
        }
    }
}

fn main() {
    let counter = Counter::new(3, 7);
    let values: Vec<u32> = counter.collect();
    println!("{:?}", values); // [3, 4, 5, 6]
}
```

The key insight: the iterator holds mutable state (`current`) and produces values lazily. Nothing is computed until `next()` is called. This matters for the query planner — plan nodes will eventually be iterators that produce rows on demand.

### Closures

A closure is an anonymous function that captures variables from its surrounding scope:

```rust
let threshold = 18;
let is_adult = |age: i32| age >= threshold;   // captures `threshold`
println!("{}", is_adult(21));  // true
println!("{}", is_adult(15));  // false
```

Rust has three closure traits, based on how the closure uses captured variables:

```rust
// Fn — borrows captured variables immutably (can be called many times)
let name = String::from("Alice");
let greet = || println!("Hello, {}", name);     // borrows `name`
greet();
greet();   // OK — Fn can be called repeatedly
println!("{}", name);  // OK — name is still usable

// FnMut — borrows captured variables mutably (can be called many times)
let mut count = 0;
let mut increment = || { count += 1; count };   // mutably borrows `count`
println!("{}", increment());  // 1
println!("{}", increment());  // 2

// FnOnce — takes ownership of captured variables (can be called only once)
let name = String::from("Alice");
let consume = move || {
    let owned = name;  // moves `name` into the closure
    println!("Consumed: {}", owned);
};
consume();
// consume();  // ERROR — already consumed
// println!("{}", name);  // ERROR — name was moved
```

The `move` keyword forces a closure to take ownership of all captured variables, even if it only needs a reference. This is essential when sending closures to other threads or storing them in structs that outlive the current scope.

### Iterator adaptors

Iterator adaptors are methods on `Iterator` that transform one iterator into another. They are lazy — no work happens until you consume the result:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// .filter() keeps elements matching a predicate
let evens: Vec<&i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)
    .collect();
// [2, 4, 6, 8, 10]

// .map() transforms each element
let doubled: Vec<i32> = numbers.iter()
    .map(|n| n * 2)
    .collect();
// [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]

// Chain them together
let result: Vec<i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)    // keep evens
    .map(|n| n * n)               // square them
    .collect();
// [4, 16, 36, 64, 100]

// .flat_map() maps and flattens in one step
let words = vec!["hello world", "foo bar"];
let chars: Vec<char> = words.iter()
    .flat_map(|s| s.chars())
    .collect();
// ['h', 'e', 'l', 'l', 'o', ' ', 'w', 'o', 'r', 'l', 'd', ' ', 'f', 'o', 'o', ' ', 'b', 'a', 'r']

// .enumerate() pairs each element with its index
let indexed: Vec<(usize, &i32)> = numbers.iter()
    .enumerate()
    .collect();
// [(0, 1), (1, 2), (2, 3), ...]

// .zip() pairs elements from two iterators
let names = vec!["Alice", "Bob", "Carol"];
let ages = vec![30, 25, 28];
let pairs: Vec<(&str, &i32)> = names.iter()
    .copied()
    .zip(ages.iter())
    .collect();
// [("Alice", 30), ("Bob", 25), ("Carol", 28)]
```

### Collecting into different types

The `.collect()` method is generic — it can produce different collection types based on the type annotation:

```rust
let numbers = vec![1, 2, 3, 4, 5];

// Collect into a Vec
let v: Vec<i32> = numbers.iter().copied().collect();

// Collect into a HashSet
use std::collections::HashSet;
let s: HashSet<i32> = numbers.iter().copied().collect();

// Collect into a HashMap
use std::collections::HashMap;
let names = vec!["Alice", "Bob"];
let ages = vec![30, 25];
let map: HashMap<&str, i32> = names.into_iter()
    .zip(ages.into_iter())
    .collect();

// Collect Results — stops at the first error
let strings = vec!["1", "2", "oops", "4"];
let parsed: Result<Vec<i32>, _> = strings.iter()
    .map(|s| s.parse::<i32>())
    .collect();
// Err(ParseIntError)
```

That last example — collecting `Result`s — is particularly useful in the planner. When validating a list of columns, you want to stop at the first invalid column and return an error. `.collect::<Result<Vec<_>, _>>()` does exactly this.

> **Coming from other languages?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Transform elements | `arr.map(x => x * 2)` | `[x * 2 for x in arr]` | `for _, v := range arr { ... }` | `iter.map(\|x\| x * 2)` |
> | Filter elements | `arr.filter(x => x > 0)` | `[x for x in arr if x > 0]` | `for _, v := range arr { if v > 0 { ... } }` | `iter.filter(\|x\| **x > 0)` |
> | Chaining | `arr.filter(...).map(...)` | Nested comprehension or generator pipeline | Manual loop composition | `iter.filter(...).map(...)` |
> | Laziness | Eager (arrays) | Lazy (generators) | N/A (manual loops) | Lazy (adaptors) until `.collect()` |
> | Anonymous functions | `(x) => x + 1` | `lambda x: x + 1` | `func(x int) int { return x + 1 }` | `\|x\| x + 1` |
> | Capture semantics | Automatic (closure) | Automatic (closure) | Automatic (closure) | Explicit (`move` for ownership) |
>
> The biggest difference: Rust iterators are **lazy and zero-cost**. The chain `.filter().map().collect()` compiles to a single loop — the compiler fuses the operations. In JavaScript, `arr.filter().map()` creates two intermediate arrays. Rust creates zero.

---
