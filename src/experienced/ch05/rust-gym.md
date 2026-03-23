## Rust Gym

### Drill 1: Lifetime Annotations on Struct Fields

This code does not compile. Add the correct lifetime annotations to fix it:

```rust
struct Config {
    name: &str,
    version: &str,
}

fn make_config(name: &str, version: &str) -> Config {
    Config { name, version }
}

fn main() {
    let config = make_config("toydb", "0.1.0");
    println!("{} v{}", config.name, config.version);
}
```

<details>
<summary>Solution</summary>

```rust
struct Config<'a> {
    name: &'a str,
    version: &'a str,
}

fn make_config<'a>(name: &'a str, version: &'a str) -> Config<'a> {
    Config { name, version }
}

fn main() {
    let config = make_config("toydb", "0.1.0");
    println!("{} v{}", config.name, config.version);
}
```

The struct holds references, so it needs a lifetime parameter `'a`. The function signature says: "the returned `Config` lives as long as both inputs." Since we pass string literals (`&'static str`), the config is valid for the entire program.

If instead you had:

```rust
let name = String::from("toydb");
let config = make_config(&name, "0.1.0");
drop(name); // ERROR: cannot drop name while config borrows it
```

The compiler would catch this — `config` borrows `name`, so `name` cannot be dropped first.

</details>

### Drill 2: Return a Reference With Proper Lifetime

This function should return the longer of two string slices. Fix the lifetime annotations:

```rust
fn longest(a: &str, b: &str) -> &str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let result;
    {
        let a = String::from("hello");
        let b = String::from("hi");
        result = longest(&a, &b);
    }
    // Can we use result here?
    // println!("{}", result);
}
```

<details>
<summary>Solution</summary>

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let result;
    {
        let a = String::from("hello");
        let b = String::from("hi");
        result = longest(&a, &b);
        // result is valid here — a and b are still alive
        println!("{}", result); // prints "hello"
    }
    // result is NOT valid here — a and b were dropped
    // println!("{}", result);  // COMPILE ERROR: borrowed value does not live long enough
}
```

The lifetime `'a` constrains `result` to live no longer than the shortest-lived input. Since `a` and `b` are dropped at the end of the inner block, `result` cannot be used after that block.

To fix this, either use `result` inside the block, or move `a` and `b` to the same scope as `result`.

</details>

### Drill 3: Iterator That Borrows From a Collection

Implement an iterator that yields references to values in a `Vec<i32>` that are above a threshold:

```rust
struct AboveThreshold<'a> {
    data: &'a [i32],
    threshold: i32,
    index: usize,
}

// Implement Iterator for AboveThreshold
// It should yield &'a i32 references to elements above the threshold
```

<details>
<summary>Solution</summary>

```rust
struct AboveThreshold<'a> {
    data: &'a [i32],
    threshold: i32,
    index: usize,
}

impl<'a> AboveThreshold<'a> {
    fn new(data: &'a [i32], threshold: i32) -> Self {
        AboveThreshold {
            data,
            threshold,
            index: 0,
        }
    }
}

impl<'a> Iterator for AboveThreshold<'a> {
    type Item = &'a i32;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.data.len() {
            let val = &self.data[self.index];
            self.index += 1;
            if *val > self.threshold {
                return Some(val);
            }
        }
        None
    }
}

#[test]
fn above_threshold_iterator() {
    let data = vec![1, 5, 3, 8, 2, 9, 4];
    let above_five: Vec<&i32> = AboveThreshold::new(&data, 5).collect();
    assert_eq!(above_five, vec![&8, &9]);
}
```

The lifetime `'a` threads from the `data` slice through the `Iterator::Item` type. This tells the compiler: "the references this iterator yields are valid as long as the original slice is valid." If you dropped `data` while iterating, the compiler would catch the dangling reference.

In practice, you would use `data.iter().filter(|&&x| x > 5)` instead of a custom iterator. But understanding how to build one teaches you what `.filter()` does internally.

</details>

---
