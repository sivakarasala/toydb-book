## Spotlight: Iterators & Closures

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **iterators and closures**.

### What is an iterator?

An iterator is something that produces a sequence of values, one at a time. Think of a ticket dispenser at a deli counter. Each time you pull a ticket, you get the next number. The dispenser keeps track of where it is. When there are no more tickets, it stops.

In Rust, iterators work through a single trait with a single method:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

Let us break this down:

- **`type Item`** -- what kind of values does this iterator produce? Numbers? Strings? Database rows? This is an "associated type" -- a type that is defined by the implementation.
- **`fn next(&mut self)`** -- give me the next value. Returns `Some(value)` if there is one, or `None` when the sequence is finished.
- **`&mut self`** -- the iterator can change its internal state (like advancing a counter). That is why it needs a mutable reference.

### Your first encounter: for loops

Every `for` loop you have written in Rust uses an iterator behind the scenes:

```rust
let names = vec!["Alice", "Bob", "Carol"];

// This:
for name in &names {
    println!("{}", name);
}

// Is actually this:
let mut iter = names.iter();
loop {
    match iter.next() {
        Some(name) => println!("{}", name),
        None => break,
    }
}
```

`vec.iter()` creates an iterator over references to the items in the vector. Each call to `.next()` returns the next item. When we have gone through all items, `.next()` returns `None` and the loop ends.

> **What just happened?**
>
> A `for` loop is syntactic sugar -- a convenient shorthand -- for repeatedly calling `.next()` on an iterator. The compiler transforms your `for` loop into the `loop` / `match` version automatically. You do not pay any performance cost for using `for` -- it compiles to the same code.

### The power of iterator adapters

The magic of iterators is that you can chain transformations together. Each transformation is called an **adapter** -- it takes an iterator and produces a new iterator:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// .filter() keeps only elements that match a condition
let evens: Vec<&i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)
    .collect();
// [2, 4, 6, 8, 10]
```

Let us unpack this step by step:

1. `numbers.iter()` -- create an iterator over `&i32` references
2. `.filter(|n| **n % 2 == 0)` -- keep only even numbers
3. `.collect()` -- gather the results into a `Vec`

The thing inside `.filter()` -- that `|n| **n % 2 == 0` -- is called a **closure**. We will explain closures in detail shortly.

### More adapter examples

```rust
let numbers = vec![1, 2, 3, 4, 5];

// .map() transforms each element
let doubled: Vec<i32> = numbers.iter()
    .map(|n| n * 2)
    .collect();
// [2, 4, 6, 8, 10]

// Chain adapters together
let result: Vec<i32> = numbers.iter()
    .filter(|n| **n % 2 == 0)    // keep evens: [2, 4]
    .map(|n| n * n)               // square them: [4, 16]
    .collect();
// [4, 16]

// .enumerate() pairs each element with its index
let indexed: Vec<(usize, &i32)> = numbers.iter()
    .enumerate()
    .collect();
// [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]

// .any() checks if ANY element matches
let has_even = numbers.iter().any(|n| n % 2 == 0);
// true

// .find() returns the first match
let first_even = numbers.iter().find(|n| **n % 2 == 0);
// Some(2)
```

### What are closures?

A closure is a small, anonymous function that you define right where you need it. Think of it as a sticky note with instructions that you hand to someone.

```rust
// A regular function
fn double(x: i32) -> i32 {
    x * 2
}

// The same thing as a closure
let double = |x: i32| -> i32 { x * 2 };

// Closures can be even shorter -- types are often inferred
let double = |x| x * 2;
```

The `|x|` part is the parameter list (between pipes instead of parentheses). The `x * 2` part is the body. If the body is a single expression, you do not need curly braces or a `return` statement.

The special thing about closures is that they can **capture** variables from their surroundings:

```rust
let threshold = 18;

// This closure "captures" the threshold variable
let is_adult = |age: i32| age >= threshold;

println!("{}", is_adult(21));  // true
println!("{}", is_adult(15));  // false
```

A regular function cannot access `threshold` -- it only sees its parameters. A closure can reach out and grab variables from the code around it. That is what makes closures so useful with iterator adapters: you can customize the behavior of `.filter()`, `.map()`, etc. with values from your surrounding code.

> **What just happened?**
>
> A closure is like a sticky note that says "do this" plus a snapshot of any variables it needs from the surrounding code. When you write `.filter(|n| *n > threshold)`, you are handing the iterator a sticky note that says "keep items greater than 18." The iterator reads the sticky note each time it has a new item.

### Closures with iterators: a real example

Here is a practical example from our codebase. Suppose we have a list of column names and want to check if a specific column exists:

```rust
let columns = vec!["id", "name", "age", "email"];

// Does the column "age" exist?
let exists = columns.iter().any(|col| *col == "age");
// true

// Find all columns that start with "e"
let e_columns: Vec<&&str> = columns.iter()
    .filter(|col| col.starts_with("e"))
    .collect();
// ["email"]

// Transform column names to uppercase
let upper: Vec<String> = columns.iter()
    .map(|col| col.to_uppercase())
    .collect();
// ["ID", "NAME", "AGE", "EMAIL"]
```

### .collect() and the turbofish

`.collect()` is special -- it can produce different collection types depending on what you ask for. The type annotation tells Rust what to collect into:

```rust
let numbers = vec![1, 2, 3, 4, 5];

// Collect into a Vec
let v: Vec<i32> = numbers.iter().copied().collect();

// Or use "turbofish" syntax (the ::<> after collect)
let v = numbers.iter().copied().collect::<Vec<i32>>();
```

The `::<Vec<i32>>` syntax is called "turbofish" because it looks like a fish: `::<>`. It provides a type hint to `.collect()` when Rust cannot infer the type from context.

### Collecting Results: stopping at the first error

One powerful pattern is collecting a sequence of `Result` values. If any element is an `Err`, the entire collection becomes an `Err`:

```rust
let strings = vec!["1", "2", "oops", "4"];
let parsed: Result<Vec<i32>, _> = strings.iter()
    .map(|s| s.parse::<i32>())
    .collect();
// Err(ParseIntError) -- stopped at "oops"
```

This is incredibly useful in the planner: when validating a list of columns, we want to stop at the first invalid column and return an error.

> **Common Mistakes**
>
> 1. **Forgetting `.collect()`**: Iterator adapters are lazy -- `.filter()` and `.map()` do not actually do anything until you consume the iterator (with `.collect()`, `for`, `.count()`, etc.). If you write `numbers.iter().filter(...)` without `.collect()`, nothing happens.
>
> 2. **Double references in `.filter()`**: When you call `.iter()` on a `Vec<i32>`, you get an iterator of `&i32`. Then `.filter()` gives you `&&i32` (a reference to a reference). Use `**n` to dereference twice, or `|&&n|` in the pattern to destructure.
>
> 3. **Using `.iter()` vs `.into_iter()`**: `.iter()` borrows the elements (you keep the original Vec). `.into_iter()` takes ownership (the original Vec is consumed). Use `.iter()` when you want to keep the original data.

---
