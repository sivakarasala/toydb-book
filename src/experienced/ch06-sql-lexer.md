# Chapter 6: SQL Lexer — Tokenization

Your database has a storage engine, serialization, and MVCC transactions. But the interface is still Rust function calls — `txn.set("name", Value::String("Alice"))`. No one wants to write Rust code to query a database. They want to write SQL: `INSERT INTO users (name) VALUES ('Alice')`. This chapter begins the bridge between human-readable SQL and the internal operations you have already built.

The first step in understanding any language — SQL, Python, English — is breaking the input into tokens. The sentence "SELECT name FROM users WHERE id = 42" is just a string of characters. Before you can understand its meaning, you need to identify the pieces: `SELECT` is a keyword, `name` is an identifier, `42` is a number, `=` is an operator. This process is called lexing (or tokenization), and you will build one from scratch.

By the end of this chapter, you will have:

- A `Token` enum with variants for keywords, identifiers, numbers, strings, and operators
- A `Keyword` enum covering the SQL subset your database will support
- A `Lexer` struct that scans a string character-by-character and emits typed tokens
- Proper error handling for unterminated strings, unknown characters, and invalid input
- A deep understanding of Rust enums with data, exhaustive pattern matching, and the Display trait

---

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

## Exercise 1: Define the Token Enum

**Goal:** Define the `Token` and `Keyword` enums that represent every piece of SQL your database will understand.

### Step 1: Create the lexer module

Create `src/lexer.rs`:

```rust
/// SQL keywords recognized by our database.
#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Create,
    Table,
    And,
    Or,
    Not,
    Null,
    True,
    False,
}
```

### Step 2: Define the Token enum

```rust
/// A single token in a SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A SQL keyword (SELECT, FROM, WHERE, etc.)
    Keyword(Keyword),
    /// An identifier (table name, column name)
    Ident(String),
    /// An integer literal
    Number(i64),
    /// A string literal (single-quoted in SQL)
    Str(String),
    /// +
    Plus,
    /// -
    Minus,
    /// *
    Star,
    /// /
    Slash,
    /// =
    Equals,
    /// !=  or  <>
    NotEquals,
    /// <
    LessThan,
    /// >
    GreaterThan,
    /// <=
    LessOrEqual,
    /// >=
    GreaterOrEqual,
    /// (
    LeftParen,
    /// )
    RightParen,
    /// ,
    Comma,
    /// ;
    Semicolon,
    /// End of input
    EOF,
}
```

We name it `Str` instead of `String` to avoid colliding with Rust's `std::string::String`. This is a common Rust idiom — when your domain concept clashes with a standard library name, pick a short alias.

### Step 3: Implement Display for both enums

Readable error messages require readable token names. Implement `Display` for `Token` and `Keyword`:

```rust
use std::fmt;

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Keyword::Select => write!(f, "SELECT"),
            Keyword::From => write!(f, "FROM"),
            Keyword::Where => write!(f, "WHERE"),
            Keyword::Insert => write!(f, "INSERT"),
            Keyword::Into => write!(f, "INTO"),
            Keyword::Values => write!(f, "VALUES"),
            Keyword::Update => write!(f, "UPDATE"),
            Keyword::Set => write!(f, "SET"),
            Keyword::Delete => write!(f, "DELETE"),
            Keyword::Create => write!(f, "CREATE"),
            Keyword::Table => write!(f, "TABLE"),
            Keyword::And => write!(f, "AND"),
            Keyword::Or => write!(f, "OR"),
            Keyword::Not => write!(f, "NOT"),
            Keyword::Null => write!(f, "NULL"),
            Keyword::True => write!(f, "TRUE"),
            Keyword::False => write!(f, "FALSE"),
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Keyword(kw) => write!(f, "{}", kw),
            Token::Ident(name) => write!(f, "{}", name),
            Token::Number(n) => write!(f, "{}", n),
            Token::Str(s) => write!(f, "'{}'", s),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Equals => write!(f, "="),
            Token::NotEquals => write!(f, "!="),
            Token::LessThan => write!(f, "<"),
            Token::GreaterThan => write!(f, ">"),
            Token::LessOrEqual => write!(f, "<="),
            Token::GreaterOrEqual => write!(f, ">="),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Semicolon => write!(f, ";"),
            Token::EOF => write!(f, "EOF"),
        }
    }
}
```

### Step 4: Add a keyword lookup function

SQL keywords are case-insensitive: `SELECT`, `select`, and `SeLeCt` are all the same keyword. We need a function that checks whether a string is a keyword:

```rust
impl Keyword {
    /// Try to match a string to a keyword (case-insensitive).
    /// Returns None if the string is not a keyword.
    pub fn from_str(s: &str) -> Option<Keyword> {
        match s.to_uppercase().as_str() {
            "SELECT" => Some(Keyword::Select),
            "FROM" => Some(Keyword::From),
            "WHERE" => Some(Keyword::Where),
            "INSERT" => Some(Keyword::Insert),
            "INTO" => Some(Keyword::Into),
            "VALUES" => Some(Keyword::Values),
            "UPDATE" => Some(Keyword::Update),
            "SET" => Some(Keyword::Set),
            "DELETE" => Some(Keyword::Delete),
            "CREATE" => Some(Keyword::Create),
            "TABLE" => Some(Keyword::Table),
            "AND" => Some(Keyword::And),
            "OR" => Some(Keyword::Or),
            "NOT" => Some(Keyword::Not),
            "NULL" => Some(Keyword::Null),
            "TRUE" => Some(Keyword::True),
            "FALSE" => Some(Keyword::False),
            _ => None,
        }
    }
}
```

### Step 5: Register the module and test

In `src/lib.rs`:

```rust
pub mod lexer;
```

Write a quick test to make sure the types compile:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lookup() {
        assert_eq!(Keyword::from_str("SELECT"), Some(Keyword::Select));
        assert_eq!(Keyword::from_str("select"), Some(Keyword::Select));
        assert_eq!(Keyword::from_str("SeLeCt"), Some(Keyword::Select));
        assert_eq!(Keyword::from_str("name"), None);
    }

    #[test]
    fn token_display() {
        assert_eq!(Token::Keyword(Keyword::Select).to_string(), "SELECT");
        assert_eq!(Token::Number(42).to_string(), "42");
        assert_eq!(Token::Str("hello".to_string()).to_string(), "'hello'");
        assert_eq!(Token::Star.to_string(), "*");
    }
}
```

```
$ cargo test lexer::tests
running 2 tests
test lexer::tests::keyword_lookup ... ok
test lexer::tests::token_display ... ok

test result: ok. 2 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why not use Rust's FromStr trait?</summary>

We could implement `std::str::FromStr` for `Keyword`, but that trait returns `Result<Self, Self::Err>` and requires defining an error type. Our `from_str` returns `Option<Keyword>` — the string is either a keyword or it is not, and "not a keyword" is not an error (it just means the string is an identifier). Using `Option` is more semantically correct here.

</details>

---

## Exercise 2: Define the Keyword Enum and Classify Tokens

**Goal:** Extend the `Keyword` enum with helper methods for classification, and add utility methods to `Token` for use by the parser in later chapters.

### Step 1: Add classification methods to Token

The parser will need to ask questions like "is this token an operator?" or "is this token a literal value?" Add these methods:

```rust
impl Token {
    /// Is this token a comparison operator?
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Token::Equals
                | Token::NotEquals
                | Token::LessThan
                | Token::GreaterThan
                | Token::LessOrEqual
                | Token::GreaterOrEqual
        )
    }

    /// Is this token a literal value (number, string, true, false, null)?
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Token::Number(_)
                | Token::Str(_)
                | Token::Keyword(Keyword::True)
                | Token::Keyword(Keyword::False)
                | Token::Keyword(Keyword::Null)
        )
    }

    /// Is this token an arithmetic operator?
    pub fn is_arithmetic(&self) -> bool {
        matches!(self, Token::Plus | Token::Minus | Token::Star | Token::Slash)
    }

    /// Is this token a statement-starting keyword?
    pub fn is_statement_start(&self) -> bool {
        matches!(
            self,
            Token::Keyword(Keyword::Select)
                | Token::Keyword(Keyword::Insert)
                | Token::Keyword(Keyword::Update)
                | Token::Keyword(Keyword::Delete)
                | Token::Keyword(Keyword::Create)
        )
    }
}
```

### Step 2: Add classification methods to Keyword

```rust
impl Keyword {
    /// Is this keyword a DML (Data Manipulation Language) keyword?
    pub fn is_dml(&self) -> bool {
        matches!(
            self,
            Keyword::Select | Keyword::Insert | Keyword::Update | Keyword::Delete
        )
    }

    /// Is this keyword a DDL (Data Definition Language) keyword?
    pub fn is_ddl(&self) -> bool {
        matches!(self, Keyword::Create | Keyword::Table)
    }

    /// Is this keyword a logical operator?
    pub fn is_logical(&self) -> bool {
        matches!(self, Keyword::And | Keyword::Or | Keyword::Not)
    }

    /// Is this keyword a literal value?
    pub fn is_value(&self) -> bool {
        matches!(self, Keyword::True | Keyword::False | Keyword::Null)
    }
}
```

### Step 3: Test the classification methods

```rust
#[cfg(test)]
mod classification_tests {
    use super::*;

    #[test]
    fn comparison_operators() {
        assert!(Token::Equals.is_comparison());
        assert!(Token::NotEquals.is_comparison());
        assert!(Token::LessThan.is_comparison());
        assert!(!Token::Plus.is_comparison());
        assert!(!Token::Keyword(Keyword::Select).is_comparison());
    }

    #[test]
    fn literals() {
        assert!(Token::Number(42).is_literal());
        assert!(Token::Str("hello".to_string()).is_literal());
        assert!(Token::Keyword(Keyword::True).is_literal());
        assert!(Token::Keyword(Keyword::False).is_literal());
        assert!(Token::Keyword(Keyword::Null).is_literal());
        assert!(!Token::Keyword(Keyword::Select).is_literal());
        assert!(!Token::Ident("name".to_string()).is_literal());
    }

    #[test]
    fn statement_starters() {
        assert!(Token::Keyword(Keyword::Select).is_statement_start());
        assert!(Token::Keyword(Keyword::Insert).is_statement_start());
        assert!(!Token::Keyword(Keyword::From).is_statement_start());
        assert!(!Token::Keyword(Keyword::Where).is_statement_start());
    }

    #[test]
    fn keyword_categories() {
        assert!(Keyword::Select.is_dml());
        assert!(!Keyword::Create.is_dml());
        assert!(Keyword::Create.is_ddl());
        assert!(Keyword::And.is_logical());
        assert!(Keyword::True.is_value());
        assert!(!Keyword::Select.is_value());
    }
}
```

```
$ cargo test classification_tests
running 4 tests
test lexer::classification_tests::comparison_operators ... ok
test lexer::classification_tests::literals ... ok
test lexer::classification_tests::statement_starters ... ok
test lexer::classification_tests::keyword_categories ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why classify tokens in methods instead of match arms?</summary>

Classification methods centralize the definition of "what is a comparison operator?" in one place. Without them, the parser would have repeated `matches!` expressions in multiple functions. If you later add a new comparison operator (like `LIKE` or `IN`), you update one method instead of hunting through every parser function.

This is the "define errors out of existence" principle applied to code organization. By making classification a method of the type, you cannot accidentally forget to include `NotEquals` in a comparison check — you write the check once and reuse it.

</details>

---

## Exercise 3: Implement the Lexer

**Goal:** Build the core lexer that scans a SQL string character by character and produces a `Vec<Token>`. This is the heart of the chapter.

### Step 1: Define the Lexer struct

```rust
/// The SQL lexer. Converts a string of SQL into a vector of tokens.
pub struct Lexer {
    /// The input characters
    chars: Vec<char>,
    /// Current position in the input
    pos: usize,
}

impl Lexer {
    /// Create a new lexer for the given input.
    pub fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// Peek at the current character without advancing.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Peek at the next character (one ahead of current).
    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    /// Advance the position and return the current character.
    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    /// Skip whitespace characters.
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }
}
```

### Step 2: Implement the tokenize method

```rust
impl Lexer {
    /// Tokenize the entire input into a vector of tokens.
    pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
        let mut lexer = Lexer::new(input);
        let mut tokens = Vec::new();

        loop {
            lexer.skip_whitespace();

            match lexer.peek() {
                None => {
                    tokens.push(Token::EOF);
                    break;
                }
                Some(ch) => {
                    let token = match ch {
                        // Single-character tokens
                        '+' => { lexer.advance(); Token::Plus }
                        '*' => { lexer.advance(); Token::Star }
                        '/' => { lexer.advance(); Token::Slash }
                        '(' => { lexer.advance(); Token::LeftParen }
                        ')' => { lexer.advance(); Token::RightParen }
                        ',' => { lexer.advance(); Token::Comma }
                        ';' => { lexer.advance(); Token::Semicolon }

                        // Minus or negative number
                        '-' => {
                            lexer.advance();
                            Token::Minus
                        }

                        // Comparison operators (may be two characters)
                        '=' => { lexer.advance(); Token::Equals }

                        '!' => {
                            lexer.advance();
                            if lexer.peek() == Some('=') {
                                lexer.advance();
                                Token::NotEquals
                            } else {
                                return Err(format!(
                                    "Unexpected character '!' at position {} — did you mean '!='?",
                                    lexer.pos - 1
                                ));
                            }
                        }

                        '<' => {
                            lexer.advance();
                            match lexer.peek() {
                                Some('=') => { lexer.advance(); Token::LessOrEqual }
                                Some('>') => { lexer.advance(); Token::NotEquals } // SQL <> operator
                                _ => Token::LessThan
                            }
                        }

                        '>' => {
                            lexer.advance();
                            if lexer.peek() == Some('=') {
                                lexer.advance();
                                Token::GreaterOrEqual
                            } else {
                                Token::GreaterThan
                            }
                        }

                        // String literals (single-quoted)
                        '\'' => lexer.scan_string()?,

                        // Numbers
                        c if c.is_ascii_digit() => lexer.scan_number()?,

                        // Identifiers and keywords
                        c if c.is_alphabetic() || c == '_' => lexer.scan_identifier(),

                        // Unknown character
                        _ => {
                            return Err(format!(
                                "Unexpected character '{}' at position {}",
                                ch, lexer.pos
                            ));
                        }
                    };

                    tokens.push(token);
                }
            }
        }

        Ok(tokens)
    }
}
```

### Step 3: Implement the scanning methods

```rust
impl Lexer {
    /// Scan a string literal: 'hello world'
    /// SQL strings are single-quoted. Escaped quotes are '' (two single quotes).
    fn scan_string(&mut self) -> Result<Token, String> {
        let start = self.pos;
        self.advance(); // consume opening quote

        let mut value = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(format!(
                        "Unterminated string starting at position {}",
                        start
                    ));
                }
                Some('\'') => {
                    self.advance(); // consume the quote
                    // Check for escaped quote ('')
                    if self.peek() == Some('\'') {
                        value.push('\'');
                        self.advance();
                    } else {
                        // End of string
                        return Ok(Token::Str(value));
                    }
                }
                Some(ch) => {
                    value.push(ch);
                    self.advance();
                }
            }
        }
    }

    /// Scan a number: 42, 3, 1000
    fn scan_number(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let mut num_str = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let n: i64 = num_str.parse().map_err(|e| {
            format!("Invalid number '{}' at position {}: {}", num_str, start, e)
        })?;

        Ok(Token::Number(n))
    }

    /// Scan an identifier or keyword: name, SELECT, user_id
    fn scan_identifier(&mut self) -> Token {
        let mut ident = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if the identifier is actually a keyword
        match Keyword::from_str(&ident) {
            Some(kw) => Token::Keyword(kw),
            None => Token::Ident(ident),
        }
    }
}
```

### Step 4: Test the lexer

```rust
#[cfg(test)]
mod lexer_tests {
    use super::*;

    #[test]
    fn simple_select() {
        let tokens = Lexer::tokenize("SELECT * FROM users").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select),
            Token::Star,
            Token::Keyword(Keyword::From),
            Token::Ident("users".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn select_with_where() {
        let tokens = Lexer::tokenize("SELECT name FROM users WHERE id = 42").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select),
            Token::Ident("name".to_string()),
            Token::Keyword(Keyword::From),
            Token::Ident("users".to_string()),
            Token::Keyword(Keyword::Where),
            Token::Ident("id".to_string()),
            Token::Equals,
            Token::Number(42),
            Token::EOF,
        ]);
    }

    #[test]
    fn insert_statement() {
        let tokens = Lexer::tokenize(
            "INSERT INTO users (name, age) VALUES ('Alice', 30)"
        ).unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Insert),
            Token::Keyword(Keyword::Into),
            Token::Ident("users".to_string()),
            Token::LeftParen,
            Token::Ident("name".to_string()),
            Token::Comma,
            Token::Ident("age".to_string()),
            Token::RightParen,
            Token::Keyword(Keyword::Values),
            Token::LeftParen,
            Token::Str("Alice".to_string()),
            Token::Comma,
            Token::Number(30),
            Token::RightParen,
            Token::EOF,
        ]);
    }

    #[test]
    fn case_insensitive_keywords() {
        let upper = Lexer::tokenize("SELECT").unwrap();
        let lower = Lexer::tokenize("select").unwrap();
        let mixed = Lexer::tokenize("SeLeCt").unwrap();

        assert_eq!(upper, lower);
        assert_eq!(lower, mixed);
    }

    #[test]
    fn comparison_operators() {
        let tokens = Lexer::tokenize("a = b != c < d > e <= f >= g <> h").unwrap();

        let ops: Vec<&Token> = tokens.iter()
            .filter(|t| t.is_comparison())
            .collect();

        assert_eq!(ops.len(), 6);
        assert_eq!(ops[0], &Token::Equals);
        assert_eq!(ops[1], &Token::NotEquals);
        assert_eq!(ops[2], &Token::LessThan);
        assert_eq!(ops[3], &Token::GreaterThan);
        assert_eq!(ops[4], &Token::LessOrEqual);
        assert_eq!(ops[5], &Token::GreaterOrEqual);
    }

    #[test]
    fn empty_input() {
        let tokens = Lexer::tokenize("").unwrap();
        assert_eq!(tokens, vec![Token::EOF]);
    }

    #[test]
    fn whitespace_only() {
        let tokens = Lexer::tokenize("   \t\n  ").unwrap();
        assert_eq!(tokens, vec![Token::EOF]);
    }

    #[test]
    fn semicolon_terminated() {
        let tokens = Lexer::tokenize("SELECT 1;").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select),
            Token::Number(1),
            Token::Semicolon,
            Token::EOF,
        ]);
    }

    #[test]
    fn display_round_trip() {
        // Every token's Display output should be recognizable
        let tokens = vec![
            Token::Keyword(Keyword::Select),
            Token::Number(42),
            Token::Str("hello".to_string()),
            Token::Plus,
            Token::Equals,
        ];

        let display: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
        assert_eq!(display, vec!["SELECT", "42", "'hello'", "+", "="]);
    }
}
```

```
$ cargo test lexer_tests
running 9 tests
test lexer::lexer_tests::simple_select ... ok
test lexer::lexer_tests::select_with_where ... ok
test lexer::lexer_tests::insert_statement ... ok
test lexer::lexer_tests::case_insensitive_keywords ... ok
test lexer::lexer_tests::comparison_operators ... ok
test lexer::lexer_tests::empty_input ... ok
test lexer::lexer_tests::whitespace_only ... ok
test lexer::lexer_tests::semicolon_terminated ... ok
test lexer::lexer_tests::display_round_trip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: If your lexer produces Ident("SELECT") instead of Keyword(Select)</summary>

The `scan_identifier` method reads the identifier first, then checks if it is a keyword. Make sure your `Keyword::from_str` converts the input to uppercase before matching. SQL keywords are case-insensitive, so `select`, `SELECT`, and `SeLeCt` should all produce `Keyword::Select`.

Check that the method reads:
```rust
match s.to_uppercase().as_str() {
```

Not:
```rust
match s {
```

</details>

---

## Exercise 4: Handle Edge Cases

**Goal:** A lexer that only handles happy paths is not a lexer — it is a demo. Real SQL has escaped quotes, identifiers with underscores, arithmetic expressions, and plenty of ways to be malformed. Handle them all.

### Step 1: String edge cases

```rust
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn string_with_escaped_quote() {
        // SQL escapes single quotes by doubling them: 'it''s'
        let tokens = Lexer::tokenize("'it''s a test'").unwrap();
        assert_eq!(tokens, vec![
            Token::Str("it's a test".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn empty_string() {
        let tokens = Lexer::tokenize("''").unwrap();
        assert_eq!(tokens, vec![
            Token::Str("".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn string_with_spaces_and_special_chars() {
        let tokens = Lexer::tokenize("'hello world! @#$%'").unwrap();
        assert_eq!(tokens, vec![
            Token::Str("hello world! @#$%".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn unterminated_string_is_error() {
        let result = Lexer::tokenize("'hello");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unterminated string"), "Error was: {}", err);
    }
}
```

### Step 2: Number edge cases

```rust
    #[test]
    fn large_number() {
        let tokens = Lexer::tokenize("SELECT 9999999999").unwrap();
        assert_eq!(tokens[1], Token::Number(9999999999));
    }

    #[test]
    fn zero() {
        let tokens = Lexer::tokenize("0").unwrap();
        assert_eq!(tokens, vec![Token::Number(0), Token::EOF]);
    }

    #[test]
    fn negative_as_minus_then_number() {
        // The lexer produces Minus + Number — the parser combines them
        let tokens = Lexer::tokenize("-42").unwrap();
        assert_eq!(tokens, vec![
            Token::Minus,
            Token::Number(42),
            Token::EOF,
        ]);
    }
```

### Step 3: Identifier edge cases

```rust
    #[test]
    fn identifier_with_underscore() {
        let tokens = Lexer::tokenize("user_name").unwrap();
        assert_eq!(tokens, vec![
            Token::Ident("user_name".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn identifier_starting_with_underscore() {
        let tokens = Lexer::tokenize("_private").unwrap();
        assert_eq!(tokens, vec![
            Token::Ident("_private".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn identifier_with_numbers() {
        let tokens = Lexer::tokenize("table1").unwrap();
        assert_eq!(tokens, vec![
            Token::Ident("table1".to_string()),
            Token::EOF,
        ]);
    }

    #[test]
    fn keywords_are_not_identifiers() {
        // "select" is a keyword, "selection" is an identifier
        let tokens = Lexer::tokenize("select selection").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select),
            Token::Ident("selection".to_string()),
            Token::EOF,
        ]);
    }
```

### Step 4: Error cases

```rust
    #[test]
    fn unknown_character_is_error() {
        let result = Lexer::tokenize("SELECT @ FROM users");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unexpected character '@'"), "Error was: {}", err);
    }

    #[test]
    fn bare_exclamation_is_error() {
        let result = Lexer::tokenize("!");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("did you mean '!='"), "Error was: {}", err);
    }
```

### Step 5: A complex real-world query

```rust
    #[test]
    fn complex_query() {
        let sql = "SELECT name, age FROM users WHERE age >= 18 AND name != 'admin';";
        let tokens = Lexer::tokenize(sql).unwrap();

        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select),
            Token::Ident("name".to_string()),
            Token::Comma,
            Token::Ident("age".to_string()),
            Token::Keyword(Keyword::From),
            Token::Ident("users".to_string()),
            Token::Keyword(Keyword::Where),
            Token::Ident("age".to_string()),
            Token::GreaterOrEqual,
            Token::Number(18),
            Token::Keyword(Keyword::And),
            Token::Ident("name".to_string()),
            Token::NotEquals,
            Token::Str("admin".to_string()),
            Token::Semicolon,
            Token::EOF,
        ]);
    }

    #[test]
    fn create_table_statement() {
        let sql = "CREATE TABLE users (id, name, active)";
        let tokens = Lexer::tokenize(sql).unwrap();

        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Create),
            Token::Keyword(Keyword::Table),
            Token::Ident("users".to_string()),
            Token::LeftParen,
            Token::Ident("id".to_string()),
            Token::Comma,
            Token::Ident("name".to_string()),
            Token::Comma,
            Token::Ident("active".to_string()),
            Token::RightParen,
            Token::EOF,
        ]);
    }

    #[test]
    fn arithmetic_expression() {
        let tokens = Lexer::tokenize("1 + 2 * 3 - 4 / 2").unwrap();
        assert_eq!(tokens, vec![
            Token::Number(1),
            Token::Plus,
            Token::Number(2),
            Token::Star,
            Token::Number(3),
            Token::Minus,
            Token::Number(4),
            Token::Slash,
            Token::Number(2),
            Token::EOF,
        ]);
    }
}
```

```
$ cargo test edge_case_tests
running 14 tests
test lexer::edge_case_tests::string_with_escaped_quote ... ok
test lexer::edge_case_tests::empty_string ... ok
test lexer::edge_case_tests::string_with_spaces_and_special_chars ... ok
test lexer::edge_case_tests::unterminated_string_is_error ... ok
test lexer::edge_case_tests::large_number ... ok
test lexer::edge_case_tests::zero ... ok
test lexer::edge_case_tests::negative_as_minus_then_number ... ok
test lexer::edge_case_tests::identifier_with_underscore ... ok
test lexer::edge_case_tests::identifier_starting_with_underscore ... ok
test lexer::edge_case_tests::identifier_with_numbers ... ok
test lexer::edge_case_tests::keywords_are_not_identifiers ... ok
test lexer::edge_case_tests::unknown_character_is_error ... ok
test lexer::edge_case_tests::bare_exclamation_is_error ... ok
test lexer::edge_case_tests::complex_query ... ok
test lexer::edge_case_tests::create_table_statement ... ok
test lexer::edge_case_tests::arithmetic_expression ... ok

test result: ok. 14 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: How to handle negative numbers</summary>

Our lexer produces `Token::Minus` followed by `Token::Number(42)` for the input `-42`. The parser (Chapter 7) will combine these into a negative number during expression parsing. This is the standard approach — most SQL parsers treat `-` as a unary operator, not part of the number literal.

Why? Consider `5 - 3`. If the lexer tried to parse `-3` as a negative number, it would need to look backwards to see if there is a preceding value. That is context-sensitive parsing, which belongs in the parser, not the lexer. The lexer should be context-free: each token is determined only by the characters ahead.

</details>

---

## Rust Gym

### Drill 1: Enum With Data and Match

Define an enum `Shape` with three variants: `Circle(f64)` (radius), `Rectangle(f64, f64)` (width, height), and `Triangle(f64, f64, f64)` (three sides). Write a function `area(shape: &Shape) -> f64` using match:

```rust
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

The `match` destructures each variant and binds the inner values to local variables. For `Circle(r)`, `r` is the radius. For `Rectangle(w, h)`, `w` and `h` are width and height. The compiler ensures every variant is handled.

</details>

### Drill 2: Implement Display for a Multi-Variant Enum

Implement `Display` for a `LogLevel` enum so that `format!("{}", level)` produces human-readable output:

```rust
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

`Display` is the trait that `println!("{}", x)` uses. `Debug` (via `{:?}`) is for developers; `Display` (via `{}`) is for users. You derive `Debug` but implement `Display` by hand, because the format is domain-specific.

</details>

### Drill 3: Build a Simple Calculator Tokenizer

Build a tokenizer for arithmetic expressions. Given `"(3 + 42) * 7"`, produce:

```
[LeftParen, Number(3), Plus, Number(42), RightParen, Star, Number(7)]
```

Define your own `CalcToken` enum and a `tokenize` function. Keep it simple — only handle digits, `+`, `-`, `*`, `/`, `(`, `)`, and whitespace.

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

Notice how similar this is to the SQL lexer — same structure, same peek-advance pattern, same match-based dispatch. Tokenizers are all the same shape. Once you have built one, you can build any of them.

</details>

---

## DSA in Context: The Lexer as a Finite State Automaton

A lexer is a finite state automaton (FSA) — a machine with a fixed set of states that transitions between them based on input characters. Our lexer has four main states:

```
         ┌─────────┐
    ─────┤  START   │
         └────┬────┘
              │
    ┌─────────┼─────────┬────────────┐
    │         │         │            │
    ▼         ▼         ▼            ▼
┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│InIdent │ │InNumber│ │InString│ │  Emit  │
│        │ │        │ │        │ │ single │
└───┬────┘ └───┬────┘ └───┬────┘ └────────┘
    │          │          │
    ▼          ▼          ▼
  Emit       Emit       Emit
  token      token      token
```

**START:** Look at the current character and decide which state to enter.
- Letter or `_` -> InIdent
- Digit -> InNumber
- `'` -> InString
- Operator character -> Emit single token

**InIdent:** Keep reading while the character is alphanumeric or `_`. When a non-identifier character appears, emit the accumulated string as either a Keyword or an Ident token.

**InNumber:** Keep reading while the character is a digit. When a non-digit appears, parse the accumulated string as an i64 and emit a Number token.

**InString:** Keep reading until an unescaped `'` appears. Handle `''` as an escaped quote. Emit the accumulated string as a Str token. If EOF is reached first, emit an error.

### Regular expressions vs hand-written lexers

Many lexer generators (like flex, or Rust's `logos` crate) compile regular expressions into state machines. You write patterns like:

```
SELECT    -> Keyword(Select)
[0-9]+    -> Number(parse)
'[^']*'   -> String(extract)
[a-z_]+   -> Ident
```

The tool generates the state machine automatically. This is faster to write but harder to customize. Hand-written lexers are more work but give you complete control over error messages, edge cases, and performance.

Our SQL lexer is hand-written because:
1. The grammar is small enough to manage manually
2. We want excellent error messages ("`!` at position 5 — did you mean `!=`?")
3. It is a learning exercise — understanding the state machine is the point

### Time complexity

A lexer is O(N) where N is the input length. Each character is examined exactly once (peek is constant time, advance moves forward). There is no backtracking — once we decide we are in the InNumber state, we stay there until a non-digit appears. This is a key property of deterministic finite automata (DFA): each input character causes exactly one state transition.

---

## System Design Corner: Language Processing Pipelines

In a system design interview, explaining how a database processes a SQL query shows deep understanding. The pipeline has five stages:

```
SQL string
    │
    ▼
┌────────┐     "SELECT name FROM users WHERE id = 42"
│ Lexer  │
└───┬────┘
    │          [SELECT, name, FROM, users, WHERE, id, =, 42]
    ▼
┌────────┐
│ Parser │
└───┬────┘
    │          AST: Select { columns: [name], table: users, where: id = 42 }
    ▼
┌────────────┐
│ Optimizer  │
└───┬────────┘
    │          Plan: IndexScan(users.id = 42) -> Project(name)
    ▼
┌────────────┐
│ Executor   │
└───┬────────┘
    │          Result: [("Alice",)]
    ▼
   Client
```

**Lexer** (this chapter): Characters to tokens. O(N) where N is query length. Cannot fail on valid SQL syntax — only on invalid characters or unterminated strings.

**Parser** (Chapter 7): Tokens to Abstract Syntax Tree (AST). O(N) where N is token count. Detects syntax errors like `SELECT FROM` (missing column list) or `WHERE AND` (missing left operand).

**Optimizer** (Chapter 9): AST to execution plan. Chooses between table scan and index lookup, reorders joins, pushes filters down. This is where query performance is determined — a bad optimizer turns a 10ms query into a 10-minute query.

**Executor** (Chapter 10): Runs the plan against the storage engine. Returns rows. May use iterators (Volcano model) or batch processing (vectorized execution).

Each stage is a separate module with a clean interface. The lexer does not know about tables. The parser does not know about indexes. The optimizer does not know about disk I/O. This separation of concerns is what makes databases tractable — you can reason about each stage independently.

> **Interview talking point:** *"Our query processing pipeline has four stages: lexer, parser, optimizer, and executor. The lexer converts SQL text to tokens in O(N) time. The parser builds an AST and catches syntax errors. The optimizer generates an execution plan — choosing between sequential scans, index lookups, and join strategies based on statistics. The executor runs the plan using the Volcano iterator model, where each operator pulls rows from its children on demand. This lazy evaluation means we never materialize intermediate results larger than necessary."*

---

## Design Insight: Strategic vs Tactical Programming

In *A Philosophy of Software Design*, Ousterhout distinguishes between **tactical** and **strategic** programming:

**Tactical:** "Just make it work." The programmer is focused on shipping the current feature. They take shortcuts — copy-paste, hardcoded values, skipped abstractions. Each shortcut is small, but they accumulate into a tangled mess.

**Strategic:** "Make it right, so future work is easier." The programmer invests time in clean abstractions, even when the immediate task does not require them.

Building a proper lexer is a strategic investment. We could have skipped it:

```rust
// Tactical approach: parse SQL with string splitting
fn parse_select(sql: &str) -> Vec<String> {
    let parts: Vec<&str> = sql.split_whitespace().collect();
    // "SELECT" should be first...
    // Column names until "FROM"...
    // Table name after "FROM"...
    // Oh wait, what about "WHERE id = 42"?
    // What about string literals with spaces: 'hello world'?
    // What about parentheses: (a, b, c)?
    // ...this approach collapses.
    todo!()
}
```

The tactical approach works for `SELECT name FROM users` but breaks on anything with spaces in strings, parentheses, or operators without spaces (`id=42`). You would patch each case with more `split` and `trim` calls until the code is unreadable.

The strategic approach — building a proper lexer that handles every token type — takes more time now but pays off in every future chapter. The parser (Chapter 7) will consume tokens, not raw strings. It will never worry about whitespace, escaped quotes, or keyword casing. All of that complexity is handled once, in the lexer, and hidden from everything downstream.

This is the fundamental tradeoff Ousterhout identifies: tactical programming is faster for the current task but slower for the project. Strategic programming is slower for the current task but faster for the project. The total cost of tactical programming increases over time (each patch makes the next one harder). The total cost of strategic programming decreases over time (each abstraction makes the next feature easier).

The lexer is 150 lines of code. It will be used by every SQL feature for the rest of the book — `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `JOIN`, and anything else we add. That is a strategic investment.

---

## What You Built

In this chapter, you:

1. **Defined a Token enum** — 20 variants covering keywords, identifiers, literals, operators, and punctuation, each with appropriate data payloads
2. **Defined a Keyword enum** — 17 SQL keywords with case-insensitive lookup and classification methods
3. **Built a character-by-character lexer** — peek/advance scanning with state transitions for identifiers, numbers, strings, and operators
4. **Handled edge cases** — escaped quotes, multi-character operators (`<=`, `!=`, `<>`), unterminated strings, unknown characters, and identifiers that look like keywords

Your database can now read SQL. Not understand it — that comes next — but read it. Given `SELECT name FROM users WHERE id = 42`, it produces a clean stream of typed tokens that the parser can work with. No more raw strings.

Chapter 7 builds the parser that converts this token stream into an Abstract Syntax Tree (AST) — the structured representation of what the SQL query means. The enum and pattern matching skills you practiced here will be everywhere in the parser, because an AST is just a bigger, nested enum.

---

### DS Deep Dive

Our hand-written lexer is fine for toydb's small grammar, but production databases like PostgreSQL use more sophisticated techniques. This deep dive explores regular expression compilation to DFAs, the Thompson NFA construction, and how lexer generators like `logos` achieve zero-copy tokenization in Rust.

**-> [Lexer Theory & Automata -- "The Character Assembly Line"](../ds-narratives/ch06-lexer-automata.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/lexer.rs` — `Token` enum | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) — `Token` enum |
| `src/lexer.rs` — `Keyword` enum | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) — `Keyword` enum |
| `src/lexer.rs` — `Lexer::tokenize()` | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) — `Lexer::scan()` |
| Edge case tests | [`src/sql/parser/lexer.rs` tests](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) |
