## Rust Gym

### Drill 1: Iterator Adaptors

Without running the code, predict the output. Then verify.

```rust
fn main() {
    let data = vec![
        ("Alice", 30),
        ("Bob", 25),
        ("Carol", 35),
        ("Dave", 28),
        ("Eve", 22),
    ];

    let result: Vec<&str> = data.iter()
        .filter(|(_, age)| *age >= 28)
        .map(|(name, _)| *name)
        .take(2)
        .collect();

    println!("{:?}", result);
}
```

<details>
<summary>Solution</summary>

```
["Alice", "Carol"]
```

The pipeline processes elements one at a time:
1. `("Alice", 30)` — passes filter (30 >= 28), mapped to "Alice", `take` accepts (1 of 2)
2. `("Bob", 25)` — fails filter (25 < 28), skipped
3. `("Carol", 35)` — passes filter (35 >= 28), mapped to "Carol", `take` accepts (2 of 2)
4. `take` is satisfied — stops pulling. Dave and Eve are never examined.

This is identical to `SELECT name FROM users WHERE age >= 28 LIMIT 2`. The Volcano model works exactly the same way — the `LIMIT` operator at the top stops pulling after 2 rows.

</details>

### Drill 2: Custom Iterator

Implement a `FibonacciIterator` that yields Fibonacci numbers. It should be an infinite iterator (never returns `None`).

```rust
struct FibonacciIterator {
    // what fields do you need?
}

impl FibonacciIterator {
    fn new() -> Self {
        todo!()
    }
}

impl Iterator for FibonacciIterator {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        todo!()
    }
}

fn main() {
    let fibs: Vec<u64> = FibonacciIterator::new().take(10).collect();
    println!("{:?}", fibs);
    // Expected: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
}
```

<details>
<summary>Solution</summary>

```rust
struct FibonacciIterator {
    a: u64,
    b: u64,
}

impl FibonacciIterator {
    fn new() -> Self {
        FibonacciIterator { a: 0, b: 1 }
    }
}

impl Iterator for FibonacciIterator {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let value = self.a;
        let next = self.a + self.b;
        self.a = self.b;
        self.b = next;
        Some(value) // always returns Some — infinite iterator
    }
}

fn main() {
    let fibs: Vec<u64> = FibonacciIterator::new().take(10).collect();
    println!("{:?}", fibs);
    // [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

    // Because it's infinite, you MUST use .take() or .find() or similar
    // .collect() on an infinite iterator would loop forever
}
```

Key insight: the iterator is infinite — `next()` always returns `Some`. This is safe because Rust's lazy evaluation means nothing runs until pulled. You control termination with `.take(10)`, `.find(|&n| n > 100)`, or similar adaptors. Our `ScanExecutor` is a finite iterator (returns `None` when the table is exhausted), but the concept is the same.

</details>

### Drill 3: Composing Executors

Without running the code, trace through the executor pipeline and determine which rows are produced:

```rust
// Table: products
// | id | name      | price | category    |
// |----|-----------|-------|-------------|
// | 1  | Widget    | 25    | electronics |
// | 2  | Gadget    | 75    | electronics |
// | 3  | Doohickey | 10    | accessories |
// | 4  | Thingamob | 50    | electronics |
// | 5  | Gizmo     | 30    | accessories |

// Plan: SELECT name, price FROM products WHERE category = 'electronics' AND price > 30

// What rows does the executor produce?
```

<details>
<summary>Solution</summary>

The pipeline is:

```
Project(name, price)
  Filter(category = 'electronics' AND price > 30)
    Scan(products)
```

Processing each row through Filter:
1. Widget: category='electronics' AND 25 > 30 → false AND false → **skip**
2. Gadget: category='electronics' AND 75 > 30 → true AND true → **pass**
3. Doohickey: category='accessories' AND 10 > 30 → false AND false → **skip**
4. Thingamob: category='electronics' AND 50 > 30 → true AND true → **pass**
5. Gizmo: category='accessories' AND 30 > 30 → false AND false → **skip**

Then Project selects only name and price:

```
name      | price
----------+------
Gadget    | 75
Thingamob | 50
(2 rows)
```

The execution order in the Volcano model:
1. Project calls Filter.next()
2. Filter calls Scan.next() → Widget → evaluates predicate → false → calls Scan.next() again
3. Scan returns Gadget → predicate → true → Filter returns Gadget to Project
4. Project evaluates [ColumnRef("name"), ColumnRef("price")] → ("Gadget", 75)

The scan executor never "knows" that only 2 rows will make it through. It dutifully returns all 5 rows, one at a time, when asked.

</details>

---
