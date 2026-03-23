## Spotlight: Enums & Pattern Matching

Every chapter has one spotlight concept. This chapter's spotlight is **enums with data and exhaustive pattern matching** — the feature that makes Rust feel different from almost every other language.

### Enums in Rust are not like enums elsewhere

In most languages, an enum is a list of named constants — integers with nice names:

```c
// C enum: just integers
enum Color { RED = 0, GREEN = 1, BLUE = 2 };
```

```java
// Java enum: slightly fancier integers
enum Color { RED, GREEN, BLUE }
```

In Rust, each enum variant can carry data. This makes enums a full-blown algebraic data type:

```rust
enum Token {
    Number(i64),                    // carries an i64
    String(String),                 // carries a String
    Keyword(Keyword),               // carries another enum
    Ident(String),                  // carries a String
    Plus,                           // carries nothing
    Equals,                         // carries nothing
    LeftParen,                      // carries nothing
    EOF,                            // carries nothing
}
```

`Token::Number(42)` and `Token::String("hello".to_string())` are both `Token` values, but they carry different types of data. You cannot access the `i64` inside a `Number` without first checking that it is a `Number`. The compiler enforces this.

### Pattern matching is exhaustive

When you `match` on an enum, the compiler checks that you handle every variant:

```rust
fn describe(token: &Token) -> String {
    match token {
        Token::Number(n) => format!("number {}", n),
        Token::String(s) => format!("string '{}'", s),
        Token::Keyword(kw) => format!("keyword {:?}", kw),
        Token::Ident(name) => format!("identifier {}", name),
        Token::Plus => "plus".to_string(),
        Token::Equals => "equals".to_string(),
        Token::LeftParen => "left paren".to_string(),
        Token::EOF => "end of input".to_string(),
        // If you forget a variant, the compiler says:
        // "non-exhaustive patterns: `RightParen` not covered"
    }
}
```

This is exhaustive matching. Add a new variant to `Token` and the compiler shows you every `match` statement that needs updating. In JavaScript, you would add a new case and hope that every `switch` statement in the codebase handles it. In Rust, the compiler tells you exactly which files to fix.

### Destructuring in match arms

Pattern matching is not just checking variants — it is extracting data:

```rust
match token {
    Token::Number(n) => {
        // n is the i64 inside the Number variant
        println!("The number is {}", n);
    }
    Token::Keyword(Keyword::Select) => {
        // Nested pattern: matches only the Select keyword
        println!("Found SELECT");
    }
    Token::Keyword(other) => {
        // Catches all other keywords
        println!("Found keyword: {:?}", other);
    }
    _ => {
        // Wildcard: matches everything else
    }
}
```

The `_` wildcard catches all remaining variants. Use it sparingly — it silences the exhaustiveness check. If you add a new variant, the `_` arm silently handles it instead of warning you.

### `if let` for single-variant matching

When you only care about one variant, `if let` is cleaner than a full `match`:

```rust
if let Token::Number(n) = token {
    println!("Got number: {}", n);
}

// Equivalent to:
match token {
    Token::Number(n) => println!("Got number: {}", n),
    _ => {}
}
```

### The `matches!` macro

For boolean checks without extracting data:

```rust
let is_keyword = matches!(token, Token::Keyword(_));
let is_select = matches!(token, Token::Keyword(Keyword::Select));
let is_operator = matches!(token, Token::Plus | Token::Minus | Token::Star);
```

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Enum definition | `const Color = { RED: 0 }` | `class Color(Enum)` | `const (Red = iota)` | `enum Color { Red, Green }` |
> | Enum with data | Not possible | Not built-in | Not possible | `enum Shape { Circle(f64) }` |
> | Switch/match | `switch(x)` (fall-through) | `match x:` (3.10+) | `switch x` (no fall-through) | `match x` (exhaustive) |
> | Exhaustiveness | Not checked | Not checked | Not checked | Compile-time error |
> | Type narrowing | `typeof x === "number"` | `isinstance(x, int)` | Type assertion | Pattern destructuring |
> | Null handling | `if (x !== null)` | `if x is not None` | `if x != nil` | `match opt { Some(v) => ... }` |
>
> The key difference: in other languages, enums are labels and you check them with `if` chains. In Rust, enums carry data and the compiler ensures you handle every case. This eliminates the "I forgot to handle the new case" class of bugs entirely.

---
