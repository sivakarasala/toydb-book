## Rust Gym

Three focused exercises to build iterator and closure fluency. All exercises use `std` only.

### Rep 1: Fibonacci Iterator

Build a custom iterator that produces Fibonacci numbers:

```rust
struct Fibonacci {
    a: u64,
    b: u64,
}

impl Fibonacci {
    fn new() -> Self {
        Fibonacci { a: 0, b: 1 }
    }
}

// TODO: Implement Iterator for Fibonacci
// Each call to next() should return the next Fibonacci number:
// 0, 1, 1, 2, 3, 5, 8, 13, 21, ...
```

<details>
<summary>Hint</summary>

The state update is: compute the next value as `a + b`, then shift: `a` becomes the old `b`, and `b` becomes the new sum. Since the iterator is infinite (Fibonacci numbers never end), `next()` always returns `Some(value)`.

</details>

<details>
<summary>Solution</summary>

```rust
impl Iterator for Fibonacci {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let value = self.a;
        let next = self.a + self.b;
        self.a = self.b;
        self.b = next;
        Some(value)
    }
}

fn main() {
    // First 10 Fibonacci numbers
    let fibs: Vec<u64> = Fibonacci::new().take(10).collect();
    println!("{:?}", fibs);
    // [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

    // Sum of first 20 Fibonacci numbers
    let sum: u64 = Fibonacci::new().take(20).sum();
    println!("Sum of first 20: {}", sum);
    // Sum of first 20: 17710

    // First Fibonacci number greater than 1000
    let big = Fibonacci::new().find(|&n| n > 1000);
    println!("First > 1000: {:?}", big);
    // First > 1000: Some(1597)
}
```

Key observations:
- `.take(10)` limits an infinite iterator to 10 elements.
- `.sum()` consumes the iterator and adds all elements.
- `.find()` returns the first element matching the predicate, wrapped in `Option`.
- Because `Fibonacci` always returns `Some(...)`, it is an infinite iterator. Without `.take()` or `.find()`, calling `.collect()` would run forever.

</details>

### Rep 2: Iterator Chain — Filter, Map, Collect

Given a list of strings representing log entries, extract and count the error messages:

```rust
fn process_logs(logs: &[&str]) -> Vec<String> {
    // TODO: Use an iterator chain to:
    // 1. Filter only lines starting with "ERROR"
    // 2. Map to extract just the message (after "ERROR: ")
    // 3. Convert to uppercase
    // 4. Collect into a Vec<String>
    todo!()
}

fn count_by_level(logs: &[&str]) -> Vec<(String, usize)> {
    // TODO: Use iterators to count how many logs exist at each level.
    // Return pairs like [("ERROR", 3), ("INFO", 5), ("WARN", 2)],
    // sorted alphabetically by level.
    todo!()
}

fn main() {
    let logs = vec![
        "INFO: server started",
        "ERROR: connection refused",
        "INFO: request received",
        "WARN: slow query (2.3s)",
        "ERROR: disk full",
        "INFO: request completed",
        "ERROR: timeout",
    ];

    let errors = process_logs(&logs);
    println!("Errors: {:?}", errors);

    let counts = count_by_level(&logs);
    println!("Counts: {:?}", counts);
}
```

<details>
<summary>Hint</summary>

For `process_logs`: chain `.filter()`, `.map()`, and `.collect()`. Use `str::strip_prefix("ERROR: ")` (returns `Option<&str>`) or check `starts_with()` then slice. For `count_by_level`: use a `HashMap` to count occurrences, then collect into a sorted `Vec`.

</details>

<details>
<summary>Solution</summary>

```rust
use std::collections::HashMap;

fn process_logs(logs: &[&str]) -> Vec<String> {
    logs.iter()
        .filter_map(|line| line.strip_prefix("ERROR: "))
        .map(|msg| msg.to_uppercase())
        .collect()
}

fn count_by_level(logs: &[&str]) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for log in logs {
        // Extract the level (everything before the first ':')
        if let Some(level) = log.split(':').next() {
            *counts.entry(level.trim().to_string()).or_insert(0) += 1;
        }
    }

    let mut result: Vec<(String, usize)> = counts.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn main() {
    let logs = vec![
        "INFO: server started",
        "ERROR: connection refused",
        "INFO: request received",
        "WARN: slow query (2.3s)",
        "ERROR: disk full",
        "INFO: request completed",
        "ERROR: timeout",
    ];

    let errors = process_logs(&logs);
    println!("Errors: {:?}", errors);
    // Errors: ["CONNECTION REFUSED", "DISK FULL", "TIMEOUT"]

    let counts = count_by_level(&logs);
    println!("Counts: {:?}", counts);
    // Counts: [("ERROR", 3), ("INFO", 3), ("WARN", 1)]
}
```

Note the use of `filter_map` — it combines `filter` and `map` in one step. `strip_prefix` returns `Some(&str)` if the prefix matched (with the prefix removed), or `None` if it did not. `filter_map` keeps only the `Some` values and unwraps them. This is a common Rust idiom: "try to transform each element, keep only the successes."

</details>

### Rep 3: Tree Walker Iterator

Build a custom iterator that performs a depth-first traversal of a binary tree:

```rust
#[derive(Debug)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn leaf(value: i32) -> Self {
        TreeNode { value, left: None, right: None }
    }

    fn with_children(value: i32, left: TreeNode, right: TreeNode) -> Self {
        TreeNode {
            value,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

struct TreeIter<'a> {
    stack: Vec<&'a TreeNode>,
}

// TODO: Implement TreeIter::new() and Iterator for TreeIter
// The iterator should yield values in pre-order: root, left, right
```

<details>
<summary>Hint</summary>

Pre-order DFS uses a stack. Push the root first. On each `next()` call, pop a node, push its right child first (so left is processed first), then push its left child. Return the popped node's value.

</details>

<details>
<summary>Solution</summary>

```rust
impl<'a> TreeIter<'a> {
    fn new(root: &'a TreeNode) -> Self {
        TreeIter { stack: vec![root] }
    }
}

impl<'a> Iterator for TreeIter<'a> {
    type Item = i32;

    fn next(&mut self) -> Option<i32> {
        let node = self.stack.pop()?;

        // Push right first so left is processed first (LIFO)
        if let Some(right) = &node.right {
            self.stack.push(right);
        }
        if let Some(left) = &node.left {
            self.stack.push(left);
        }

        Some(node.value)
    }
}

fn main() {
    //       1
    //      / \
    //     2   3
    //    / \
    //   4   5
    let tree = TreeNode::with_children(
        1,
        TreeNode::with_children(
            2,
            TreeNode::leaf(4),
            TreeNode::leaf(5),
        ),
        TreeNode::leaf(3),
    );

    let values: Vec<i32> = TreeIter::new(&tree).collect();
    println!("Pre-order: {:?}", values);
    // Pre-order: [1, 2, 4, 5, 3]

    // Use iterator adaptors on the tree
    let sum: i32 = TreeIter::new(&tree).sum();
    println!("Sum: {}", sum);
    // Sum: 15

    let evens: Vec<i32> = TreeIter::new(&tree)
        .filter(|v| v % 2 == 0)
        .collect();
    println!("Even values: {:?}", evens);
    // Even values: [2, 4]
}
```

This is powerful: once you implement `Iterator`, you get `.sum()`, `.filter()`, `.map()`, `.any()`, `.find()`, and dozens of other methods for free. The tree walker produces values lazily — if you only need `.find(|v| v > 3)`, it stops traversal as soon as it finds 4, without visiting the rest of the tree.

This is exactly how the query executor in Chapter 10 will work. Plan nodes will be iterators that produce rows, and the executor will chain them together with iterator adaptors.

</details>

---
