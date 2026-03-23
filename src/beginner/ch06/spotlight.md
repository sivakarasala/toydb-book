## Spotlight: Enums & Pattern Matching

Every chapter has one spotlight concept. This chapter's spotlight is **enums with data and exhaustive pattern matching** -- the feature that makes Rust feel different from almost every other language.

### Enums in Rust are not like enums elsewhere

In most languages, an enum is a list of named constants -- integers with nice names:

```rust,ignore
// In C:  enum Color { RED = 0, GREEN = 1, BLUE = 2 };
// In Java:  enum Color { RED, GREEN, BLUE }
```

These are just labels for numbers. You cannot attach extra data to a variant.

In Rust, each enum variant can **carry data**. This makes enums a full-blown algebraic data type:

```rust,ignore
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

`Token::Number(42)` and `Token::Ident("name".to_string())` are both `Token` values, but they carry different types of data. You cannot access the `i64` inside a `Number` without first checking that it *is* a `Number`. The compiler enforces this.

> **Analogy: Labeled boxes of different sizes**
>
> Think of enum variants as labeled boxes. A `Number` box always contains an `i64`. A `String` box always contains a `String`. A `Plus` box is empty -- the label itself is the information. You cannot open a `Plus` box and look for a number inside -- there is nothing there.

### Creating enum values

To create an enum value, you use the variant name followed by the data in parentheses:

```rust,ignore
let t1 = Token::Number(42);
let t2 = Token::Ident("users".to_string());
let t3 = Token::Plus;  // no data, no parentheses
```

### Pattern matching with `match`

The `match` expression is how you work with enums. It looks at which variant you have and runs the corresponding code:

```rust,ignore
fn describe(token: &Token) -> String {
    match token {
        Token::Number(n) => format!("number {}", n),
        Token::Ident(name) => format!("identifier {}", name),
        Token::Plus => "plus sign".to_string(),
        Token::Equals => "equals sign".to_string(),
        // ... other variants
    }
}
```

Each line inside `match` is called an **arm**. The pattern on the left (e.g., `Token::Number(n)`) is matched against the value, and the code on the right runs if it matches. The variable `n` is **bound** to the inner value -- you can use it in the arm's code.

### Exhaustive matching: the compiler has your back

Here is the most important property of `match`: **the compiler checks that you handle every variant**. If you forget one, it tells you:

```rust,ignore
fn describe(token: &Token) -> String {
    match token {
        Token::Number(n) => format!("number {}", n),
        Token::Plus => "plus sign".to_string(),
        // ERROR: non-exhaustive patterns: `Ident(_)`, `Equals`, `LeftParen`... not covered
    }
}
```

This is called **exhaustive matching**. If you add a new variant to your enum, the compiler shows you every `match` statement in your codebase that needs updating. In other languages, you would add a new case to your enum and hope that every `switch` statement handles it. In Rust, the compiler tells you exactly which files need fixing.

> **What just happened?**
>
> Exhaustive matching eliminates an entire category of bugs: "I forgot to handle the new case." When you add `Token::Semicolon` to the enum, every function that matches on `Token` will fail to compile until you add a `Semicolon` arm. This is one of Rust's most valuable safety features.

### The wildcard pattern: `_`

Sometimes you want to handle "everything else" without listing every variant:

```rust,ignore
match token {
    Token::Number(n) => println!("Got number: {}", n),
    _ => println!("Got something else"),
}
```

The `_` wildcard matches any variant you have not explicitly listed. Use it sparingly -- it silences the exhaustiveness check. If you add a new variant, the `_` arm will silently handle it instead of warning you.

### Destructuring: extracting data from variants

Pattern matching does not just check which variant you have -- it **extracts the data inside**:

```rust,ignore
match token {
    Token::Number(n) => {
        // n is the i64 inside the Number variant
        println!("The number is {}", n);
    }
    Token::Keyword(Keyword::Select) => {
        // Nested pattern: matches ONLY the Select keyword
        println!("Found SELECT");
    }
    Token::Keyword(other) => {
        // Catches all other keywords
        println!("Found keyword: {:?}", other);
    }
    _ => {}
}
```

The second arm shows **nested destructuring**: `Token::Keyword(Keyword::Select)` matches a `Token` that is a `Keyword` variant containing specifically the `Select` keyword. The third arm catches all other keywords -- `other` is bound to whichever `Keyword` variant it is.

### `if let`: matching a single variant

When you only care about one variant, `if let` is cleaner than a full `match`:

```rust,ignore
if let Token::Number(n) = token {
    println!("Got number: {}", n);
}

// Equivalent to:
match token {
    Token::Number(n) => println!("Got number: {}", n),
    _ => {}
}
```

`if let` says: "if this value matches this pattern, run this code." If it does not match, nothing happens. This is useful when you want to handle one case and ignore all others.

### The `matches!` macro: boolean checks

For quick boolean checks without extracting data:

```rust,ignore
let is_keyword = matches!(token, Token::Keyword(_));
let is_select = matches!(token, Token::Keyword(Keyword::Select));
let is_operator = matches!(token, Token::Plus | Token::Minus | Token::Star);
```

The `|` (pipe) inside `matches!` means "or" -- the last example checks if the token is Plus, Minus, or Star.

> **What just happened?**
>
> We learned four ways to work with enums:
> 1. **`match`** -- handle every variant, with exhaustive checking
> 2. **`if let`** -- handle one variant, ignore the rest
> 3. **`matches!`** -- check if a value matches a pattern, returns `bool`
> 4. **Nested destructuring** -- match on data inside data (e.g., `Token::Keyword(Keyword::Select)`)
>
> These tools are used everywhere in Rust. The lexer, parser, optimizer, and executor all use pattern matching extensively.

### Common mistakes with enums and matching

**Mistake: Forgetting that enum variants are namespaced**

```rust,ignore
let t = Number(42);        // ERROR: not found in this scope
let t = Token::Number(42); // OK: use the full path
```

Unlike some languages where enum variants are global, Rust requires the enum name as a prefix: `Token::Number`, not just `Number`. You can import variants with `use Token::*` to avoid this, but it is usually clearer to use the full path.

**Mistake: Using `_` when you should handle every case**

```rust,ignore
match token {
    Token::Number(n) => process_number(n),
    _ => {}  // silently ignores all other tokens
}
```

If someone later adds `Token::Float(f64)`, the `_` arm will silently swallow it. Consider listing every variant explicitly, or at least using `_` only when you have thought about what "everything else" means.

**Mistake: Trying to access variant data without matching**

```rust,ignore
let t = Token::Number(42);
println!("{}", t.0);  // ERROR: Token does not have field .0
```

You cannot access enum data like struct fields. You must use `match` or `if let` to extract it.

---
