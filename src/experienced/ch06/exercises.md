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
