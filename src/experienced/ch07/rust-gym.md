## Rust Gym

### Drill 1: Evaluate a Recursive Expression Tree

Define an enum for arithmetic expressions (`Num`, `Add`, `Mul`) using `Box` for recursion. Write an `eval` function:

```rust
enum Expr {
    Num(f64),
    Add { left: Box<Expr>, right: Box<Expr> },
    Mul { left: Box<Expr>, right: Box<Expr> },
}

fn eval(expr: &Expr) -> f64 {
    todo!()
}

fn main() {
    // (2 + 3) * (4 + 1) = 25
    let expr = Expr::Mul {
        left: Box::new(Expr::Add {
            left: Box::new(Expr::Num(2.0)),
            right: Box::new(Expr::Num(3.0)),
        }),
        right: Box::new(Expr::Add {
            left: Box::new(Expr::Num(4.0)),
            right: Box::new(Expr::Num(1.0)),
        }),
    };
    println!("{}", eval(&expr)); // 25
}
```

<details>
<summary>Solution</summary>

```rust
enum Expr {
    Num(f64),
    Add { left: Box<Expr>, right: Box<Expr> },
    Mul { left: Box<Expr>, right: Box<Expr> },
}

fn eval(expr: &Expr) -> f64 {
    match expr {
        Expr::Num(n) => *n,
        Expr::Add { left, right } => eval(left) + eval(right),
        Expr::Mul { left, right } => eval(left) * eval(right),
    }
}

fn main() {
    // (2 + 3) * (4 + 1)
    let expr = Expr::Mul {
        left: Box::new(Expr::Add {
            left: Box::new(Expr::Num(2.0)),
            right: Box::new(Expr::Num(3.0)),
        }),
        right: Box::new(Expr::Add {
            left: Box::new(Expr::Num(4.0)),
            right: Box::new(Expr::Num(1.0)),
        }),
    };
    println!("{}", eval(&expr));
}
```

Output:

```
25
```

The recursive `eval` function mirrors the recursive data structure. Each `match` arm handles one variant: `Num` is the base case (return the number), `Add` and `Mul` are recursive cases (evaluate children, combine results). This is the fundamental pattern for all tree-processing algorithms — you will see it again when the query executor evaluates WHERE clause expressions.

</details>

### Drill 2: Transform a `Box<Tree>` Recursively

Write a function `double_all` that takes a `Box<Tree>` and returns a new `Box<Tree>` where every `Leaf` value is doubled:

```rust
enum Tree {
    Leaf(i64),
    Node { left: Box<Tree>, right: Box<Tree> },
}

fn double_all(tree: Box<Tree>) -> Box<Tree> {
    todo!()
}

fn sum(tree: &Tree) -> i64 {
    match tree {
        Tree::Leaf(n) => *n,
        Tree::Node { left, right } => sum(left) + sum(right),
    }
}

fn main() {
    let tree = Box::new(Tree::Node {
        left: Box::new(Tree::Leaf(3)),
        right: Box::new(Tree::Node {
            left: Box::new(Tree::Leaf(5)),
            right: Box::new(Tree::Leaf(7)),
        }),
    });

    println!("Before: {}", sum(&tree));   // 15
    let doubled = double_all(tree);
    println!("After:  {}", sum(&doubled)); // 30
}
```

<details>
<summary>Solution</summary>

```rust
enum Tree {
    Leaf(i64),
    Node { left: Box<Tree>, right: Box<Tree> },
}

fn double_all(tree: Box<Tree>) -> Box<Tree> {
    match *tree {
        Tree::Leaf(n) => Box::new(Tree::Leaf(n * 2)),
        Tree::Node { left, right } => {
            Box::new(Tree::Node {
                left: double_all(left),
                right: double_all(right),
            })
        }
    }
}

fn sum(tree: &Tree) -> i64 {
    match tree {
        Tree::Leaf(n) => *n,
        Tree::Node { left, right } => sum(left) + sum(right),
    }
}

fn main() {
    let tree = Box::new(Tree::Node {
        left: Box::new(Tree::Leaf(3)),
        right: Box::new(Tree::Node {
            left: Box::new(Tree::Leaf(5)),
            right: Box::new(Tree::Leaf(7)),
        }),
    });

    println!("Before: {}", sum(&tree));
    let doubled = double_all(tree);
    println!("After:  {}", sum(&doubled));
}
```

Output:

```
Before: 15
After:  30
```

Notice `match *tree` — we dereference the `Box` to pattern-match on the `Tree` inside it. This *moves* the value out of the box, consuming the original tree. The function returns a new `Box<Tree>` rather than modifying the original. This is the Rust ownership model in action: `double_all` takes ownership of the input tree, so it can safely destructure and rebuild it without worrying about other references.

</details>

### Drill 3: Build a Simple JSON Parser

Build a JSON value parser. Given a string like `{"name": "Alice", "age": 30, "scores": [95, 87]}`, produce a structured `JsonValue` enum. Use only `std` — no external crates.

```rust
#[derive(Debug, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut chars = input.chars().peekable();
    parse_value(&mut chars)
}

fn parse_value(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<JsonValue, String> {
    todo!()
}
```

<details>
<summary>Solution</summary>

```rust
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut chars = input.chars().peekable();
    let value = parse_value(&mut chars)?;
    Ok(value)
}

fn skip_ws(chars: &mut Peekable<Chars>) {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_value(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    skip_ws(chars);
    match chars.peek() {
        Some('"') => parse_string(chars).map(JsonValue::Str),
        Some('{') => parse_object(chars),
        Some('[') => parse_array(chars),
        Some('t') | Some('f') => parse_bool(chars),
        Some('n') => parse_null(chars),
        Some(c) if c.is_ascii_digit() || *c == '-' => parse_number(chars),
        Some(c) => Err(format!("Unexpected character: {}", c)),
        None => Err("Unexpected end of input".to_string()),
    }
}

fn parse_string(chars: &mut Peekable<Chars>) -> Result<String, String> {
    chars.next(); // consume opening "
    let mut s = String::new();
    loop {
        match chars.next() {
            Some('"') => return Ok(s),
            Some('\\') => {
                match chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(c) => s.push(c),
                    None => return Err("Unterminated escape".to_string()),
                }
            }
            Some(c) => s.push(c),
            None => return Err("Unterminated string".to_string()),
        }
    }
}

fn parse_number(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    let mut num_str = String::new();
    if chars.peek() == Some(&'-') {
        num_str.push('-');
        chars.next();
    }
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' {
            num_str.push(c);
            chars.next();
        } else {
            break;
        }
    }
    num_str.parse::<f64>()
        .map(JsonValue::Number)
        .map_err(|e| format!("Invalid number '{}': {}", num_str, e))
}

fn parse_bool(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    let word: String = chars.take_while(|c| c.is_alphabetic()).collect();
    // Note: take_while consumes the first non-matching char from the
    // underlying iterator, but Peekable handles this correctly.
    match word.as_str() {
        "true" => Ok(JsonValue::Bool(true)),
        "false" => Ok(JsonValue::Bool(false)),
        _ => Err(format!("Expected true/false, got '{}'", word)),
    }
}

fn parse_null(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    let word: String = (0..4).filter_map(|_| chars.next()).collect();
    if word == "null" {
        Ok(JsonValue::Null)
    } else {
        Err(format!("Expected null, got '{}'", word))
    }
}

fn parse_array(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    chars.next(); // consume [
    let mut items = Vec::new();
    skip_ws(chars);

    if chars.peek() == Some(&']') {
        chars.next();
        return Ok(JsonValue::Array(items));
    }

    loop {
        items.push(parse_value(chars)?);
        skip_ws(chars);
        match chars.peek() {
            Some(',') => { chars.next(); }
            Some(']') => { chars.next(); return Ok(JsonValue::Array(items)); }
            _ => return Err("Expected , or ]".to_string()),
        }
    }
}

fn parse_object(chars: &mut Peekable<Chars>) -> Result<JsonValue, String> {
    chars.next(); // consume {
    let mut pairs = Vec::new();
    skip_ws(chars);

    if chars.peek() == Some(&'}') {
        chars.next();
        return Ok(JsonValue::Object(pairs));
    }

    loop {
        skip_ws(chars);
        let key = parse_string(chars)?;
        skip_ws(chars);
        match chars.next() {
            Some(':') => {}
            _ => return Err("Expected :".to_string()),
        }
        let value = parse_value(chars)?;
        pairs.push((key, value));
        skip_ws(chars);
        match chars.peek() {
            Some(',') => { chars.next(); }
            Some('}') => { chars.next(); return Ok(JsonValue::Object(pairs)); }
            _ => return Err("Expected , or }".to_string()),
        }
    }
}

fn main() {
    let input = r#"{"name": "Alice", "age": 30, "scores": [95, 87], "active": true}"#;
    match parse_json(input) {
        Ok(value) => println!("{:#?}", value),
        Err(e) => println!("Error: {}", e),
    }
}
```

Output:

```
Object(
    [
        ("name", Str("Alice")),
        ("age", Number(30.0)),
        ("scores", Array([Number(95.0), Number(87.0)])),
        ("active", Bool(true)),
    ],
)
```

This JSON parser uses the exact same techniques as the SQL parser: peek at the next character to decide what to parse, consume characters as you go, and recursively parse nested structures. `JsonValue::Array` contains `Vec<JsonValue>` and `JsonValue::Object` contains `Vec<(String, JsonValue)>` — both are recursive types. They do not need `Box` because `Vec` already heap-allocates its elements. `Box` is only needed when a type *directly* contains itself, not when it is wrapped in a `Vec` or other heap-allocated container.

</details>

---
