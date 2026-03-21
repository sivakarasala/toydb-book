# Chapter 6: SQL Lexer — Tokenization

Your database has a storage engine, serialization, and MVCC transactions. But the interface is still Rust function calls -- `txn.set("name", Value::String("Alice"))`. No one wants to write Rust code to query a database. They want to write SQL: `INSERT INTO users (name) VALUES ('Alice')`.

This chapter begins the bridge between human-readable SQL and the internal operations you have already built.

The first step in understanding any language -- SQL, Python, English -- is breaking the input into pieces. The sentence "SELECT name FROM users WHERE id = 42" is just a string of characters. Before you can understand its meaning, you need to identify the pieces: `SELECT` is a keyword, `name` is an identifier, `42` is a number, `=` is an operator. This process is called **lexing** (or tokenization), and you will build one from scratch.

> **Analogy: Reading a sentence and circling each word**
>
> Imagine you are a teacher grading a student's essay. Before you can understand the meaning, you first circle each word, underline each number, and put a box around each punctuation mark. You are not interpreting the essay yet -- you are just identifying the pieces.
>
> That is exactly what a lexer does. It reads the raw characters of a SQL string and identifies each piece: "this is a keyword," "this is a number," "this is a comma." The parser (next chapter) will take these labeled pieces and figure out what they mean together.

By the end of this chapter, you will have:

- A `Token` enum with variants for keywords, identifiers, numbers, strings, and operators
- A `Keyword` enum covering the SQL subset your database will support
- A `Lexer` struct that scans a string character-by-character and emits typed tokens
- Proper error handling for unterminated strings, unknown characters, and invalid input
- A deep understanding of Rust enums with data, exhaustive pattern matching, and iterators

---

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

## Exercise 1: Define the Token and Keyword Enums

**Goal:** Define the `Token` and `Keyword` enums that represent every piece of SQL your database will understand.

### Step 1: Create the lexer module

Create a new file `src/lexer.rs`. Start with the `Keyword` enum:

```rust,ignore
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

This is every SQL keyword our database will understand. Each variant is a simple label with no data -- the word "SELECT" becomes `Keyword::Select`, the word "FROM" becomes `Keyword::From`, etc.

We derive `Debug` (for printing), `Clone` (for copying), and `PartialEq` (for comparing in tests with `assert_eq!`).

### Step 2: Define the Token enum

Now define the main `Token` enum that wraps keywords with all other token types:

```rust,ignore
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

Let's understand the design choices:

**`Keyword(Keyword)`** -- A keyword token wraps the `Keyword` enum. This lets us distinguish `Token::Keyword(Keyword::Select)` from `Token::Ident("name".to_string())`. Keywords and identifiers look the same in SQL (both are words), but they have different meanings.

**`Ident(String)`** -- An identifier is a user-defined name like a table name or column name. It carries the actual name as a `String`.

**`Number(i64)`** -- A numeric literal carries the parsed number as an `i64`.

**`Str(String)`** -- A string literal. We name it `Str` instead of `String` to avoid conflicting with Rust's `std::string::String`. When your domain concept clashes with a standard library name, pick a short alias.

**Operator tokens (`Plus`, `Equals`, etc.)** -- These carry no data. The variant name is all the information needed. `+` is always `+`.

**`NotEquals`** -- Handles both SQL notations: `!=` and `<>`. Both mean "not equal."

**`EOF`** -- End of file/input. The lexer emits this as the last token to signal that there is no more input.

> **What just happened?**
>
> We defined the vocabulary of our SQL language. Every possible piece of SQL maps to exactly one `Token` variant. The SQL string `"SELECT * FROM users WHERE id = 42"` will become:
>
> ```
> [Keyword(Select), Star, Keyword(From), Ident("users"), Keyword(Where),
>  Ident("id"), Equals, Number(42), EOF]
> ```
>
> The lexer's job is to perform this conversion.

### Step 3: Implement Display for readable output

We want tokens to display in a human-readable format. Implement `Display` for both enums:

```rust,ignore
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
```

Notice how every variant is listed -- the compiler would reject this code if we forgot one. That is exhaustive matching at work.

```rust,ignore
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

**`Token::Keyword(kw) => write!(f, "{}", kw)`** -- For keyword tokens, we delegate to the `Keyword`'s `Display` implementation. The `{}` formatter calls `Display::fmt` on `kw`, which prints "SELECT", "FROM", etc.

**`Token::Str(s) => write!(f, "'{}'", s)`** -- String literals are wrapped in single quotes, matching SQL syntax.

### Step 4: Add keyword lookup

SQL keywords are case-insensitive: `SELECT`, `select`, and `SeLeCt` are all the same keyword. We need a function that checks whether a string is a keyword:

```rust,ignore
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

**`s.to_uppercase()`** -- Converts the input to uppercase. This makes the lookup case-insensitive: "select", "SELECT", and "SeLeCt" all become "SELECT" before matching.

**`.as_str()`** -- Converts the `String` (returned by `to_uppercase`) to a `&str` so we can match on string literals.

**Return type `Option<Keyword>`** -- Returns `Some(keyword)` if the string is a keyword, `None` if it is not. "select" returns `Some(Keyword::Select)`. "users" returns `None` -- it is an identifier, not a keyword.

> **What just happened?**
>
> We built a lookup function that distinguishes keywords from identifiers. This is crucial because in SQL, `SELECT name FROM users`, the words "SELECT" and "FROM" are keywords (they have special meaning), while "name" and "users" are identifiers (user-defined names). They look the same -- all are sequences of letters -- but the lexer must tell them apart.

### Step 5: Register the module and test

In `src/lib.rs`:

```rust,ignore
pub mod lexer;
```

Write a quick test to make sure everything compiles:

```rust,ignore
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

---

## Exercise 2: Add Token Classification Methods

**Goal:** Add methods to `Token` and `Keyword` that classify tokens into categories. The parser (next chapter) will use these to ask questions like "is this token an operator?" or "is this token a literal value?"

### Step 1: Add classification methods to Token

```rust,ignore
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

Each method uses the `matches!` macro to check if `self` is one of several variants. The `|` separates alternatives.

**Why `Token::Number(_)` uses an underscore:** The `_` means "any value." We do not care *which* number it is -- just that it is a `Number` variant. If we wrote `Token::Number(42)`, it would only match the specific number 42.

**Why `Token::Keyword(Keyword::True)` appears in `is_literal`:** In SQL, `TRUE`, `FALSE`, and `NULL` are literal values, even though they are keywords. They can appear anywhere a value is expected: `WHERE active = TRUE`.

### Step 2: Add classification methods to Keyword

```rust,ignore
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

**DML vs DDL:** SQL commands are divided into categories. **DML** (Data Manipulation Language) operates on data: SELECT, INSERT, UPDATE, DELETE. **DDL** (Data Definition Language) operates on the structure: CREATE TABLE, ALTER TABLE. This classification will help the parser route each statement to the right handler.

### Step 3: Test the classification methods

```rust,ignore
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

> **What just happened?**
>
> Classification methods centralize the definition of "what is a comparison operator?" in one place. Without them, the parser would have repeated `matches!` expressions in multiple functions. If you later add a new comparison operator (like `LIKE` or `IN`), you update one method instead of hunting through every parser function.

---

## Exercise 3: Build the Lexer

**Goal:** Build the core lexer that scans a SQL string character by character and produces a `Vec<Token>`. This is the heart of the chapter.

### Step 1: Define the Lexer struct

```rust,ignore
/// The SQL lexer. Converts a string of SQL into a vector of tokens.
pub struct Lexer {
    /// The input characters
    chars: Vec<char>,
    /// Current position in the input
    pos: usize,
}
```

**`chars: Vec<char>`** -- We store the input as a vector of characters. Rust strings are UTF-8 encoded, which means characters can be 1-4 bytes long. By converting to `Vec<char>`, we can index individual characters with `self.chars[self.pos]`. This is simpler than working with raw UTF-8 bytes.

**`pos: usize`** -- Our current position in the character array. The lexer advances through the input one character at a time, like a cursor moving across a line of text.

### Step 2: Implement helper methods

The lexer needs three basic operations: look at the current character, look at the next character, and advance to the next character.

```rust,ignore
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

Let's understand each method:

**`input.chars().collect()`** -- Converts the string into a vector of individual characters. The string "SELECT" becomes `['S', 'E', 'L', 'E', 'C', 'T']`.

**`peek(&self)`** -- Looks at the current character without moving the position. Returns `None` if we are past the end of the input. The `.copied()` converts `Option<&char>` to `Option<char>` (copies the character value instead of returning a reference).

**`advance(&mut self)`** -- Returns the current character AND moves the position forward by one. This is the "consume a character" operation.

**`skip_whitespace`** -- Advances past spaces, tabs, newlines. SQL ignores whitespace between tokens: `SELECT*FROM` and `SELECT * FROM` are the same (though the second is much more readable).

**`while let Some(ch) = self.peek()`** -- This is a loop that continues as long as `peek()` returns `Some(character)`. When `peek()` returns `None` (end of input), the loop stops. The `while let` syntax combines a loop with pattern matching.

> **What just happened?**
>
> We set up the lexer's internal machinery. The three core operations -- peek, peek_next, and advance -- are the building blocks for everything else. Peek lets us look ahead without committing. Advance lets us consume a character. Together, they let us make decisions character by character:
>
> "Is this a `<`? Peek at the next character. If it's `=`, this is a `<=` operator (advance twice). If it's `>`, this is a `<>` operator (advance twice). Otherwise, it's just `<` (advance once)."

### Step 3: Implement the main tokenize method

This is the main entry point. It creates a `Lexer`, loops until all input is consumed, and returns the list of tokens:

```rust,ignore
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

                        // Minus (we treat it as an operator; the parser
                        // handles negative numbers)
                        '-' => {
                            lexer.advance();
                            Token::Minus
                        }

                        // Single-character comparison
                        '=' => { lexer.advance(); Token::Equals }

                        // Two-character operators that start with !
                        '!' => {
                            lexer.advance();
                            if lexer.peek() == Some('=') {
                                lexer.advance();
                                Token::NotEquals
                            } else {
                                return Err(format!(
                                    "Unexpected character '!' at position {} \
                                     -- did you mean '!='?",
                                    lexer.pos - 1
                                ));
                            }
                        }

                        // Two-character operators that start with <
                        '<' => {
                            lexer.advance();
                            match lexer.peek() {
                                Some('=') => {
                                    lexer.advance();
                                    Token::LessOrEqual
                                }
                                Some('>') => {
                                    lexer.advance();
                                    Token::NotEquals  // SQL <> operator
                                }
                                _ => Token::LessThan
                            }
                        }

                        // Two-character operators that start with >
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
                        c if c.is_alphabetic() || c == '_' => {
                            lexer.scan_identifier()
                        }

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

This is a big function, so let's break it down section by section:

**The loop:** We loop forever, processing one token per iteration. The loop only breaks when we reach the end of input (`None` from `peek()`), at which point we push `Token::EOF` and stop.

**Single-character tokens:** Characters like `+`, `*`, `(`, `)` always map to exactly one token. We advance past the character and return the token.

**Multi-character operators:** Characters like `<` might be the start of `<`, `<=`, or `<>`. We advance past the first character, then peek at the next character to decide which token to produce:

```rust,ignore
'<' => {
    lexer.advance();           // consume the '<'
    match lexer.peek() {
        Some('=') => {         // next char is '='
            lexer.advance();   // consume the '='
            Token::LessOrEqual // it was '<='
        }
        Some('>') => {         // next char is '>'
            lexer.advance();   // consume the '>'
            Token::NotEquals   // it was '<>' (SQL's not-equals)
        }
        _ => Token::LessThan   // just '<' by itself
    }
}
```

**The `!` character:** In SQL, `!` by itself is not valid -- it must be followed by `=` to form `!=`. If we see `!` without `=`, we return a helpful error message.

**String literals:** When we see a single quote `'`, we call `scan_string()` (which we will implement next). The `?` after the call propagates any errors (like unterminated strings).

**Numbers:** When we see a digit, we call `scan_number()`.

**Identifiers/keywords:** When we see a letter or underscore, we call `scan_identifier()`. This method reads the full word and then checks if it is a keyword or an identifier.

**Guard patterns (`c if c.is_ascii_digit()`):** The `if` after the pattern is called a **guard**. It adds an extra condition: "match this arm only if the character is an ASCII digit." This is how we dispatch to different scanning methods based on the character type.

> **What just happened?**
>
> The tokenize method is a big `match` that looks at the current character and decides which type of token to produce. Single characters map directly to tokens. Multi-character tokens (like `<=` or `!=`) require peeking at the next character. Complex tokens (strings, numbers, identifiers) delegate to specialized scanning methods.

### Step 4: Implement the scanning methods

These methods handle tokens that span multiple characters:

```rust,ignore
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
}
```

Let's trace through the string `'it''s done'`:

1. `self.advance()` -- consume the opening `'`
2. `value.push('i')`, advance -- read `i`
3. `value.push('t')`, advance -- read `t`
4. See `'`, advance -- might be end of string
5. Peek: next character is `'` -- it is an escaped quote! Push `'` to value, advance
6. `value.push('s')`, advance -- read `s`
7. `value.push(' ')`, advance -- read space
8. `value.push('d')`, advance -- read `d`
9. `value.push('o')`, advance -- read `o`
10. `value.push('n')`, advance -- read `n`
11. `value.push('e')`, advance -- read `e`
12. See `'`, advance -- peek shows no more `'`, so this is the end
13. Return `Token::Str("it's done")`

**Unterminated strings:** If we reach the end of input (`None` from peek) before finding a closing quote, we return an error with the position of the opening quote. This helps the user find their mistake.

**Escaped quotes:** In SQL, a single quote inside a string is escaped by doubling it: `'it''s'` means the string `it's`. When we see a quote, we peek at the next character. If it is another quote, we add a literal quote to the value and continue. If it is anything else, the string is done.

```rust,ignore
impl Lexer {
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
}
```

**`while let Some(ch) = self.peek()`** -- Keep reading characters as long as they are digits. When we hit a non-digit (or end of input), stop.

**`num_str.parse()`** -- Converts the accumulated string of digits into an `i64`. This can fail for very large numbers, so it returns a `Result`. The `.map_err(...)` converts the parse error into our error format.

```rust,ignore
impl Lexer {
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

**Reading the identifier:** We collect characters that are letters, digits, or underscores. So `user_id_2` is a valid identifier, but `user-id` would stop at the `-` (producing the identifier `user`, then a `Minus` token, then the identifier `id`).

**Keyword vs identifier:** After reading the complete word, we check if it is a keyword using `Keyword::from_str`. If "select" is a keyword, we return `Token::Keyword(Keyword::Select)`. If "users" is not a keyword, we return `Token::Ident("users".to_string())`.

> **What just happened?**
>
> We built three specialized scanners:
> - **`scan_string`** -- reads everything between single quotes, handling escaped quotes (`''`)
> - **`scan_number`** -- reads consecutive digits and parses them as an `i64`
> - **`scan_identifier`** -- reads a word (letters, digits, underscores) and checks if it is a keyword
>
> Each scanner advances the position past all the characters it consumes. When it returns, the lexer's position is at the character after the token, ready for the next iteration.

### Step 5: Test the lexer

```rust,ignore
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
    fn empty_input() {
        let tokens = Lexer::tokenize("").unwrap();
        assert_eq!(tokens, vec![Token::EOF]);
    }

    #[test]
    fn whitespace_only() {
        let tokens = Lexer::tokenize("   \t\n  ").unwrap();
        assert_eq!(tokens, vec![Token::EOF]);
    }
}
```

```
$ cargo test lexer_tests
running 6 tests
test lexer::lexer_tests::simple_select ... ok
test lexer::lexer_tests::select_with_where ... ok
test lexer::lexer_tests::insert_statement ... ok
test lexer::lexer_tests::case_insensitive_keywords ... ok
test lexer::lexer_tests::empty_input ... ok
test lexer::lexer_tests::whitespace_only ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

> **What just happened?**
>
> We tested the lexer with real SQL statements. Each test verifies that the input string produces exactly the expected sequence of tokens. The `INSERT` test is particularly interesting -- it shows how parentheses, commas, strings, and numbers all get properly tokenized.
>
> The case-insensitivity test proves that `SELECT`, `select`, and `SeLeCt` all produce the same token.

> **Common mistake: Producing `Ident("SELECT")` instead of `Keyword(Select)`**
>
> If your keywords are not being recognized, check that `Keyword::from_str` converts to uppercase before matching:
>
> ```rust,ignore
> // WRONG -- only matches uppercase input
> match s {
>     "SELECT" => Some(Keyword::Select),
>     ...
> }
>
> // RIGHT -- converts to uppercase first
> match s.to_uppercase().as_str() {
>     "SELECT" => Some(Keyword::Select),
>     ...
> }
> ```

---

## Exercise 4: Handle Edge Cases

**Goal:** A lexer that only handles happy paths is not a lexer -- it is a demo. Real SQL has escaped quotes, identifiers with underscores, multi-character operators, and plenty of ways to be malformed. Handle them all.

### Step 1: String edge cases

```rust,ignore
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

> **What just happened?**
>
> We tested four string edge cases:
> 1. **Escaped quotes:** `'it''s a test'` should produce the string `it's a test`. The doubled quote `''` is SQL's escape sequence for a literal single quote.
> 2. **Empty strings:** `''` is a valid SQL string containing zero characters.
> 3. **Special characters:** Strings can contain anything except unescaped quotes.
> 4. **Unterminated strings:** `'hello` without a closing quote should produce a helpful error.

### Step 2: Number and operator edge cases

```rust,ignore
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
        // The lexer produces Minus + Number -- the parser combines them
        let tokens = Lexer::tokenize("-42").unwrap();
        assert_eq!(tokens, vec![
            Token::Minus,
            Token::Number(42),
            Token::EOF,
        ]);
    }

    #[test]
    fn comparison_operators() {
        let tokens = Lexer::tokenize("a = b != c < d > e <= f >= g <> h").unwrap();

        let ops: Vec<&Token> = tokens.iter()
            .filter(|t| t.is_comparison())
            .collect();

        assert_eq!(ops.len(), 6);
        assert_eq!(ops[0], &Token::Equals);
        assert_eq!(ops[1], &Token::NotEquals);  // !=
        assert_eq!(ops[2], &Token::LessThan);
        assert_eq!(ops[3], &Token::GreaterThan);
        assert_eq!(ops[4], &Token::LessOrEqual);
        assert_eq!(ops[5], &Token::GreaterOrEqual);
    }
```

> **What just happened?**
>
> The negative number test is important: `-42` produces `Minus` then `Number(42)`, not `Number(-42)`. The lexer does not handle negative numbers directly -- it is the parser's job (next chapter) to combine `Minus` and `Number` into a negative value. This is the standard approach: the lexer is context-free, meaning each token is determined only by the characters ahead, not by what came before.
>
> The `<>` operator test verifies that the SQL "not equals" syntax `<>` produces the same token as `!=`.

### Step 3: Identifier edge cases

```rust,ignore
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

The last test is subtle and important: `select` is a keyword, but `selection` is an identifier. The lexer reads the full word before checking if it is a keyword. `selection` contains `select` as a prefix, but the full word is not a keyword.

### Step 4: Error cases

```rust,ignore
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

Good error messages are important. Instead of "syntax error," we tell the user exactly which character was unexpected and suggest a fix ("did you mean `!=`?").

### Step 5: A complex real-world query

```rust,ignore
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
test lexer::edge_case_tests::comparison_operators ... ok
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

> **What just happened?**
>
> We tested the lexer against increasingly complex inputs. The complex query test (`SELECT name, age FROM users WHERE age >= 18 AND name != 'admin';`) exercises almost every token type: keywords, identifiers, commas, comparison operators, numbers, logical operators, strings, and semicolons. If this test passes, the lexer handles real-world SQL.

---

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

## What You Built

In this chapter, you:

1. **Defined a Token enum** -- 20 variants covering keywords, identifiers, literals, operators, and punctuation, each with appropriate data payloads
2. **Defined a Keyword enum** -- 17 SQL keywords with case-insensitive lookup and classification methods
3. **Built a character-by-character lexer** -- peek/advance scanning with state transitions for identifiers, numbers, strings, and operators
4. **Handled edge cases** -- escaped quotes, multi-character operators (`<=`, `!=`, `<>`), unterminated strings, unknown characters, and identifiers that look like keywords
5. **Learned Rust enums with data** -- variants that carry different types, exhaustive pattern matching, `if let`, and `matches!`

Your database can now read SQL. Not understand it -- that comes next -- but read it. Given `SELECT name FROM users WHERE id = 42`, it produces a clean stream of typed tokens that the parser can work with. No more raw strings.

Chapter 7 builds the parser that converts this token stream into an Abstract Syntax Tree (AST) -- the structured representation of what the SQL query means. The enum and pattern matching skills you practiced here will be everywhere in the parser, because an AST is just a bigger, nested enum.

---

## Exercises

**Exercise 6.1: Add float number support**

Modify `scan_number` to handle decimal numbers like `3.14` and `0.5`. When a number contains a dot, parse it as `f64` and return a new `Token::Float(f64)` variant.

<details>
<summary>Hint</summary>

After reading the initial digits, check if the next character is a `.` followed by a digit. If so, continue reading digits after the dot. Then parse the full string with `.parse::<f64>()`. You will need to add a `Float(f64)` variant to the `Token` enum.

</details>

**Exercise 6.2: Add `BETWEEN`, `LIKE`, and `IS` keywords**

Add three new keywords to the `Keyword` enum. Update `from_str`, `Display`, and add them to relevant classification methods.

<details>
<summary>Hint</summary>

Add the variants to `Keyword`, add arms in the `match` statements of `from_str` and `Display`, and decide which classification category each belongs to. `LIKE` might be considered a comparison operator.

</details>

**Exercise 6.3: Position tracking**

Modify the `Token` enum (or create a `SpannedToken` wrapper) to include the start and end position of each token. This helps produce better error messages in the parser: "expected ')' at position 34, found ','" instead of "expected ')' found ','".

<details>
<summary>Hint</summary>

Create a struct `SpannedToken { token: Token, start: usize, end: usize }`. In each scanning method, record `self.pos` before and after scanning to capture the span.

</details>

---

## Key Takeaways

- **Rust enums carry data.** Each variant can hold different types, making them much more powerful than enums in most languages.
- **Exhaustive matching** forces you to handle every case. Add a new variant and the compiler shows you every `match` that needs updating.
- **`if let`** is concise for single-variant matching. **`matches!`** is concise for boolean checks.
- **A lexer** converts a string of characters into a sequence of typed tokens. It is the first stage of any language processor.
- **Peek/advance** is the fundamental pattern for character-by-character scanning. Peek to decide, advance to consume.
- **Keywords vs identifiers** look the same (both are words) but have different meanings. The lexer reads the full word, then checks if it is a keyword.
- **Error messages should be helpful.** Include the position and suggest fixes ("did you mean `!=`?").
- **The lexer is context-free.** Each token is determined only by the characters ahead. Context-sensitive decisions (like negative numbers) belong in the parser.

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/lexer.rs` -- `Token` enum | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) -- `Token` enum |
| `src/lexer.rs` -- `Keyword` enum | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) -- `Keyword` enum |
| `src/lexer.rs` -- `Lexer::tokenize()` | [`src/sql/parser/lexer.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) -- `Lexer::scan()` |
| Edge case tests | [`src/sql/parser/lexer.rs` tests](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/lexer.rs) |
