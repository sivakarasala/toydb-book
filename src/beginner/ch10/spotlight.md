## Spotlight: Advanced Iterators

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **advanced iterators** -- moving beyond basic `for` loops and `.map()` chains into custom iterator implementations and the composition pattern that makes the Volcano model possible.

### Review: the Iterator trait

We covered iterators in Chapter 8, but let us refresh the core idea:

```rust
pub trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

One method. One associated type. That is the entire contract.

- `type Item` -- what kind of value does this iterator produce?
- `fn next(&mut self)` -- give me the next value, or `None` if finished

Every `for` loop, every `.map()`, every `.filter()`, every `.collect()` -- they all work through this single `next()` method. The standard library builds over 70 adapter methods on top of it.

### Building your own iterator

Most iterators you have used come from collections: `vec.iter()`, `map.keys()`, `string.chars()`. But any struct can be an iterator. All it needs is `impl Iterator` with a `next()` method.

Let us build a simple one -- a counter that counts by a given step:

```rust
/// An iterator that counts from a start value to an end value,
/// incrementing by a step each time.
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
            return None;  // No more values
        }
        let value = self.current;
        self.current += self.step;  // Advance for next call
        Some(value)
    }
}

fn main() {
    let evens: Vec<i64> = StepCounter::new(0, 10, 2).collect();
    println!("{:?}", evens); // [0, 2, 4, 6, 8]
}
```

Let us trace through what happens:

1. First `next()` call: `current` is 0, which is less than `end` (10). Return `Some(0)`. Advance `current` to 2.
2. Second call: `current` is 2 < 10. Return `Some(2)`. Advance to 4.
3. Third call: `current` is 4 < 10. Return `Some(4)`. Advance to 6.
4. Fourth call: `current` is 6 < 10. Return `Some(6)`. Advance to 8.
5. Fifth call: `current` is 8 < 10. Return `Some(8)`. Advance to 10.
6. Sixth call: `current` is 10, which is NOT less than `end` (10). Return `None`.
7. The `.collect()` call stops when it gets `None`.

Notice that the iterator holds **mutable state** (`current`). Each call to `next()` advances the state. That is why the method takes `&mut self` -- it needs to modify the struct. Nothing is computed until `next()` is called. This is called **lazy evaluation**.

> **What just happened?**
>
> We created a struct that produces values on demand. The struct remembers where it is (via `current`), and each call to `next()` produces the next value and advances the internal state. This is exactly how our database executors will work -- each executor holds state (a position in a table, a child executor) and produces rows one at a time when asked.

### Composing iterators: wrapping one in another

The real power of iterators comes from composition -- wrapping one iterator inside another. Each wrapper transforms the output:

```rust
let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let result: Vec<i32> = numbers.iter()
    .filter(|&&n| n % 2 == 0)      // keep even numbers
    .map(|&n| n * n)                // square each one
    .take(3)                         // stop after 3 results
    .collect();                      // gather into a Vec

println!("{:?}", result); // [4, 16, 36]
```

This pipeline does NOT do three separate passes over the data. It does not:
1. Create a temporary list of all evens
2. Create another temporary list of squares
3. Take 3 from that list

Instead, values flow through one at a time:

```
take asks map for next →
  map asks filter for next →
    filter asks iter for next → gets 1 (odd, skip)
    filter asks iter for next → gets 2 (even, pass)
  map squares 2 → returns 4
take has 1 result, asks for more →
  ...
```

When `take` has 3 results, it stops asking. Numbers 7 through 10 are never examined.

This is **lazy evaluation** -- nothing happens until someone pulls. This is exactly how the Volcano model works in databases.

### The Volcano model: pull-based execution

Consider a table with 10 million rows and this query:

```sql
SELECT name FROM users WHERE age > 30 LIMIT 5;
```

**Eager approach** (bad): Scan all 10 million rows. Filter to get maybe 3 million matches. Take 5. You touched 10 million rows to produce 5.

**Volcano/pull approach** (good): The LIMIT operator asks the Project for a row. Project asks Filter. Filter asks Scan for rows one at a time until it finds one where age > 30. When LIMIT has 5 rows, it stops asking. If the first 5 matching rows are in the first 100 rows of the table, you only read 100 rows -- not 10 million.

The operator at the top of the tree controls how many rows flow through the entire system. This is why every production database uses this model.

```
       LIMIT 5
         │ pulls from
       PROJECT
         │ pulls from
       FILTER
         │ pulls from
       SCAN
```

Each operator is an iterator. The top operator pulls from the one below, which pulls from the one below that, all the way down to the Scan. Data flows upward, one row at a time.

> **What just happened?**
>
> The Volcano model turns a query plan into a chain of iterators. The top-level operator controls how much data flows. A `LIMIT 5` at the top means only 5 rows flow through the system, even if the table has millions of rows. Each operator is independent -- it just knows how to pull from its child and transform the data. This separation makes it easy to add new operators without changing existing ones.

---
