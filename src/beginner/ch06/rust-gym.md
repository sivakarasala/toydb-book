## Rust Gym

### Drill 1: Enum With Data and Match

Define an enum `Shape` with three variants: `Circle(f64)` (radius), `Rectangle(f64, f64)` (width, height), and `Triangle(f64, f64, f64)` (three sides). Write a function `area(shape: &Shape) -> f64` using match:

```rust,ignore
// Define Shape here

fn area(shape: &Shape) -> f64 {
    // Calculate using match
    todo!()
}

fn main() {
    let shapes = vec![
        Shape::Circle(5.0),
        Shape::Rectangle(4.0, 6.0),
        Shape::Triangle(3.0, 4.0, 5.0),
    ];

    for shape in &shapes {
        println!("Area: {:.2}", area(shape));
    }
}
```

<details>
<summary>Hint: Formulas for each shape</summary>

- Circle: PI * radius * radius
- Rectangle: width * height
- Triangle: Use Heron's formula. First compute `s = (a + b + c) / 2.0`, then `area = sqrt(s * (s-a) * (s-b) * (s-c))`. Use `f64::sqrt()` or `.sqrt()` for the square root.

</details>

<details>
<summary>Solution</summary>

```rust
enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle(f64, f64, f64),
}

fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle(r) => std::f64::consts::PI * r * r,
        Shape::Rectangle(w, h) => w * h,
        Shape::Triangle(a, b, c) => {
            // Heron's formula
            let s = (a + b + c) / 2.0;
            (s * (s - a) * (s - b) * (s - c)).sqrt()
        }
    }
}

fn main() {
    let shapes = vec![
        Shape::Circle(5.0),
        Shape::Rectangle(4.0, 6.0),
        Shape::Triangle(3.0, 4.0, 5.0),
    ];

    for shape in &shapes {
        println!("Area: {:.2}", area(shape));
    }
}
```

Output:

```
Area: 78.54
Area: 24.00
Area: 6.00
```

The `match` destructures each variant and binds the inner values to local variables. For `Circle(r)`, `r` is the radius. For `Rectangle(w, h)`, `w` and `h` are width and height. The compiler ensures every variant is handled -- if you add `Shape::Hexagon`, the match will not compile until you add an arm for it.

</details>

### Drill 2: Implement Display for a Multi-Variant Enum

Implement `Display` for a `LogLevel` enum so that `format!("{}", level)` produces human-readable output:

```rust,ignore
use std::fmt;

enum LogLevel {
    Error(String),
    Warn(String),
    Info(String),
    Debug(String),
}

// Implement Display so that:
// LogLevel::Error("disk full".into()) displays as "[ERROR] disk full"
// LogLevel::Warn("low memory".into()) displays as "[WARN] low memory"
// etc.
```

<details>
<summary>Hint: Use match with write!</summary>

Each arm should use `write!(f, "[LEVEL] {}", msg)` where LEVEL is the uppercase name and `msg` is the inner string. Use `match self { LogLevel::Error(msg) => ... }` to destructure each variant and access the message.

</details>

<details>
<summary>Solution</summary>

```rust
use std::fmt;

enum LogLevel {
    Error(String),
    Warn(String),
    Info(String),
    Debug(String),
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Error(msg) => write!(f, "[ERROR] {}", msg),
            LogLevel::Warn(msg) => write!(f, "[WARN] {}", msg),
            LogLevel::Info(msg) => write!(f, "[INFO] {}", msg),
            LogLevel::Debug(msg) => write!(f, "[DEBUG] {}", msg),
        }
    }
}

fn main() {
    let logs = vec![
        LogLevel::Error("disk full".into()),
        LogLevel::Warn("low memory".into()),
        LogLevel::Info("server started".into()),
        LogLevel::Debug("connection accepted".into()),
    ];

    for log in &logs {
        println!("{}", log);
    }
}
```

Output:

```
[ERROR] disk full
[WARN] low memory
[INFO] server started
[DEBUG] connection accepted
```

`Display` is the trait that `println!("{}", x)` uses. `Debug` (via `{:?}`) is for developers; `Display` (via `{}`) is for users. You derive `Debug` but implement `Display` by hand, because the display format is domain-specific.

</details>

### Drill 3: Build a Simple Calculator Tokenizer

Build a tokenizer for arithmetic expressions. Given `"(3 + 42) * 7"`, produce:

```
[LeftParen, Number(3), Plus, Number(42), RightParen, Star, Number(7)]
```

Define your own `CalcToken` enum and a `tokenize` function. Only handle digits, `+`, `-`, `*`, `/`, `(`, `)`, and whitespace.

<details>
<summary>Hint: Use the same peek/advance pattern</summary>

Convert the input to `Vec<char>`, use a `pos` variable, and loop through with a `match` on the current character. For digits, use a nested `while` loop to collect consecutive digits, then parse with `.parse::<i64>()`. Skip whitespace before each token.

</details>

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, PartialEq)]
enum CalcToken {
    Number(i64),
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
}

fn tokenize_calc(input: &str) -> Result<Vec<CalcToken>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < chars.len() {
        match chars[pos] {
            ' ' | '\t' | '\n' => { pos += 1; }
            '+' => { tokens.push(CalcToken::Plus); pos += 1; }
            '-' => { tokens.push(CalcToken::Minus); pos += 1; }
            '*' => { tokens.push(CalcToken::Star); pos += 1; }
            '/' => { tokens.push(CalcToken::Slash); pos += 1; }
            '(' => { tokens.push(CalcToken::LeftParen); pos += 1; }
            ')' => { tokens.push(CalcToken::RightParen); pos += 1; }
            c if c.is_ascii_digit() => {
                let start = pos;
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
                let num_str: String = chars[start..pos].iter().collect();
                let n: i64 = num_str.parse().map_err(|e| format!("{}", e))?;
                tokens.push(CalcToken::Number(n));
            }
            c => return Err(format!("Unexpected character '{}'", c)),
        }
    }

    Ok(tokens)
}

#[test]
fn calc_tokenizer() {
    let tokens = tokenize_calc("(3 + 42) * 7").unwrap();
    assert_eq!(tokens, vec![
        CalcToken::LeftParen,
        CalcToken::Number(3),
        CalcToken::Plus,
        CalcToken::Number(42),
        CalcToken::RightParen,
        CalcToken::Star,
        CalcToken::Number(7),
    ]);
}
```

Notice how similar this is to the SQL lexer -- same structure, same peek-advance pattern, same match-based dispatch. Tokenizers are all the same shape. Once you have built one, you can build any of them.

</details>

---
