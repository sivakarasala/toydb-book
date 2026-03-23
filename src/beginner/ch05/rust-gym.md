## Rust Gym

### Drill 1: Lifetime Annotations on Struct Fields

This code does not compile. Add the correct lifetime annotations to fix it:

```rust,ignore
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
<summary>Hint: The struct holds references, so it needs a lifetime parameter</summary>

When a struct contains references (`&str`), Rust needs to know how long those references are valid. Add a lifetime parameter `<'a>` to the struct and annotate each reference field with `'a`. The function that creates the struct also needs the lifetime parameter.

</details>

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

```rust,ignore
let name = String::from("toydb");
let config = make_config(&name, "0.1.0");
drop(name); // ERROR: cannot drop name while config borrows it
```

The compiler would catch this -- `config` borrows `name`, so `name` cannot be dropped first.

</details>

### Drill 2: Return a Reference With Proper Lifetime

This function should return the longer of two string slices. Fix the lifetime annotations:

```rust,ignore
fn longest(a: &str, b: &str) -> &str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let result;
    let a = String::from("hello");
    {
        let b = String::from("hi");
        result = longest(&a, &b);
        println!("{}", result); // Can we use result here?
    }
    // println!("{}", result);  // Can we use result here?
}
```

<details>
<summary>Hint: Which lifetime constraint applies?</summary>

The return value could borrow from either `a` or `b`. The lifetime must be the shorter of the two. Since `b` is dropped at the end of the inner block, `result` cannot be used after that block.

</details>

<details>
<summary>Solution</summary>

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let a = String::from("hello");
    {
        let b = String::from("hi");
        let result = longest(&a, &b);
        println!("{}", result); // OK: both a and b are alive
    }
    // result is NOT available here -- b was dropped
}
```

The lifetime `'a` constrains `result` to live no longer than the shortest-lived input. Since `b` is dropped at the end of the inner block, `result` cannot be used after that block.

</details>

### Drill 3: Borrowing Rules in Practice

Predict which of these code snippets will compile. Then check with `cargo build`:

**Snippet A:**
```rust,ignore
let mut v = vec![1, 2, 3];
let first = &v[0];
v.push(4);
println!("{}", first);
```

**Snippet B:**
```rust,ignore
let mut v = vec![1, 2, 3];
let first = &v[0];
println!("{}", first);
v.push(4);
```

**Snippet C:**
```rust,ignore
let mut v = vec![1, 2, 3];
let first = v[0];  // note: no &
v.push(4);
println!("{}", first);
```

<details>
<summary>Solution</summary>

**Snippet A: Does NOT compile.** `first` is a reference to the first element. `v.push(4)` might reallocate the vector's memory, invalidating `first`. Since `first` is used after `push`, the borrow checker rejects this.

**Snippet B: Compiles.** `first` is used before `push`, so the borrow ends before the mutation begins. The borrow checker sees that `first` is never used after the point where `v` is mutated.

**Snippet C: Compiles.** `first` is a copy of `v[0]`, not a reference to it. `i32` implements `Copy`, so `let first = v[0]` copies the value. `first` is independent of `v` after that point, so mutating `v` is fine.

The key lesson: references create dependencies between variables. Copies break those dependencies.

</details>

---
