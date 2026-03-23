## Exercise 1: Define the AST Types

**Goal:** Define the type system that represents parsed SQL -- the `Expression`, `Statement`, `Value`, and `Operator` types that form the Abstract Syntax Tree.

### Step 1: Create the parser module

Create a new file `src/parser.rs` and register it in `src/lib.rs`:

```rust
// src/lib.rs
pub mod lexer;
pub mod parser;
```

This tells Rust that there is a module called `parser` and its code lives in `src/parser.rs`.

### Step 2: Define the Value type

SQL has several kinds of literal values: strings like `'hello'`, integers like `42`, booleans like `TRUE`, and the special value `NULL`. We need a Rust type to represent these:

```rust
// src/parser.rs
use crate::lexer::{Keyword, Token};
use std::fmt;

/// A SQL value -- the leaves of expression trees.
/// Think of these as the "atoms" of SQL -- the simplest possible values.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A string literal: 'hello'
    String(String),
    /// An integer literal: 42
    Integer(i64),
    /// A boolean literal: TRUE or FALSE
    Boolean(bool),
    /// The NULL literal
    Null,
}
```

Let us break down what `#[derive(Debug, Clone, PartialEq)]` does. These are three traits (abilities) we are asking the compiler to automatically implement for our type:

- **Debug** -- lets us print the value with `{:?}` for debugging. Without it, `println!("{:?}", value)` would not compile.
- **Clone** -- lets us make copies of the value with `.clone()`. Without it, we could not duplicate AST nodes.
- **PartialEq** -- lets us compare two values with `==`. Without it, our tests could not use `assert_eq!`.

Now let us add a `Display` implementation so we can print values in a human-readable format:

```rust
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "'{}'", s),
            Value::Integer(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Value::Null => write!(f, "NULL"),
        }
    }
}
```

> **What just happened?**
>
> `Display` is a trait that controls how a type is printed with `{}` (as opposed to `{:?}` which uses `Debug`). The `write!` macro works like `println!` but writes to a formatter instead of the console. The `*b` in the Boolean arm dereferences the reference -- `b` is `&bool`, and `if` needs a `bool`, so we use `*` to get the value behind the reference.

### Step 3: Define the Operator type

Operators connect expressions. They come in three flavors:

- **Arithmetic** operators combine numbers: `+`, `-`, `*`, `/`
- **Comparison** operators compare values and produce booleans: `=`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical** operators combine booleans: `AND`, `OR`, `NOT`

```rust
/// Operators in SQL expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    // Arithmetic
    Add,      // +
    Sub,      // -
    Mul,      // *
    Div,      // /
    // Comparison
    Eq,       // =
    NotEq,    // != or <>
    Lt,       // <
    Gt,       // >
    LtEq,     // <=
    GtEq,     // >=
    // Logical
    And,      // AND
    Or,       // OR
    Not,      // NOT
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operator::Add => write!(f, "+"),
            Operator::Sub => write!(f, "-"),
            Operator::Mul => write!(f, "*"),
            Operator::Div => write!(f, "/"),
            Operator::Eq => write!(f, "="),
            Operator::NotEq => write!(f, "!="),
            Operator::Lt => write!(f, "<"),
            Operator::Gt => write!(f, ">"),
            Operator::LtEq => write!(f, "<="),
            Operator::GtEq => write!(f, ">="),
            Operator::And => write!(f, "AND"),
            Operator::Or => write!(f, "OR"),
            Operator::Not => write!(f, "NOT"),
        }
    }
}
```

### Step 4: Define the Expression type

This is where `Box` enters. An expression can contain other expressions. For example, `age > 18 AND name = 'Alice'` is an AND of two comparisons, each of which is itself a comparison of a column and a literal. The nesting can go as deep as needed.

```rust
/// A SQL expression -- the core recursive type of the AST.
///
/// Think of this as a tree. Columns and Literals are leaves (they have
/// no children). BinaryOp and UnaryOp are branches (they contain
/// child expressions).
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A column reference: name, age, users.id
    Column(String),
    /// A literal value: 42, 'hello', TRUE, NULL
    Literal(Value),
    /// A binary operation: left op right
    /// Examples: age > 18, price * quantity, name = 'Alice'
    BinaryOp {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },
    /// A unary operation: op expr
    /// Examples: NOT active, -price
    UnaryOp {
        op: Operator,
        expr: Box<Expression>,
    },
}
```

Let us trace through how `age > 18` is represented:

```
BinaryOp
├── left:  Column("age")     -- a leaf
├── op:    Gt                 -- greater than
└── right: Literal(Integer(18)) -- a leaf
```

And `age > 18 AND name = 'Alice'`:

```
BinaryOp (AND)
├── left:  BinaryOp (>)
│          ├── left:  Column("age")
│          └── right: Literal(Integer(18))
└── right: BinaryOp (=)
           ├── left:  Column("name")
           └── right: Literal(String("Alice"))
```

Notice how the tree captures the structure that a flat list of tokens cannot. The AND connects two comparisons. Each comparison connects a column and a value. The tree makes this hierarchy explicit.

### Step 5: Define the column definition type

For `CREATE TABLE`, we need to describe what columns a table has:

```rust
/// A column definition in a CREATE TABLE statement.
/// Example: name TEXT, age INTEGER
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
}

/// SQL data types for column definitions.
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Integer,
    Text,
    Boolean,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Integer => write!(f, "INTEGER"),
            DataType::Text => write!(f, "TEXT"),
            DataType::Boolean => write!(f, "BOOLEAN"),
        }
    }
}
```

Here we use a `struct` instead of an `enum`. A struct bundles multiple fields together -- every column definition has both a name and a data type, always. An enum is for "one of several possibilities." A struct is for "all of these things together."

### Step 6: Define the Statement type

Each kind of SQL statement becomes a variant of the `Statement` enum:

```rust
/// A parsed SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// SELECT columns FROM table [WHERE condition]
    Select {
        /// The columns to return. Each can be a simple column name
        /// or a computed expression like age + 1.
        columns: Vec<Expression>,
        /// The table to read from.
        from: String,
        /// An optional filter condition.
        where_clause: Option<Expression>,
    },

    /// INSERT INTO table (columns) VALUES (values)
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Expression>,
    },

    /// CREATE TABLE name (column_definitions)
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
}
```

Two things to notice here:

**Why `Vec<Expression>` for SELECT columns instead of `Vec<String>`?** Because `SELECT age + 1, name FROM users` is valid SQL. The first column is an arithmetic expression, not just a column name. By using `Vec<Expression>`, we can represent both simple columns (`Expression::Column("name")`) and computed expressions (`Expression::BinaryOp { ... }`).

**What is `Option<Expression>`?** The `Option` type means "this value might or might not exist." A WHERE clause is optional -- `SELECT * FROM users` has no WHERE clause, while `SELECT * FROM users WHERE age > 18` does. `Option<Expression>` is `None` when there is no WHERE clause, and `Some(expr)` when there is one.

> **What just happened?**
>
> We defined the entire type system for our SQL AST using four key Rust features:
> - **Enums** to represent "one of several alternatives" (Value, Operator, Expression, Statement)
> - **Structs** to bundle related fields together (ColumnDef)
> - **Box** to allow recursive types (Expression contains Expression)
> - **Option** to represent optional values (WHERE clause might not exist)
>
> These types compile but do not do anything yet. They are the vocabulary our parser will use to describe parsed SQL.

### Step 7: Test that the types work

Let us write tests that build AST nodes by hand. This verifies our types can represent real SQL:

```rust
#[cfg(test)]
mod ast_tests {
    use super::*;

    #[test]
    fn simple_select_ast() {
        // Represent: SELECT name FROM users WHERE age > 18
        let ast = Statement::Select {
            columns: vec![Expression::Column("name".to_string())],
            from: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            }),
        };

        // Verify the structure
        if let Statement::Select { columns, from, where_clause } = &ast {
            assert_eq!(columns.len(), 1);
            assert_eq!(from, "users");
            assert!(where_clause.is_some());
        } else {
            panic!("Expected Select statement");
        }
    }

    #[test]
    fn nested_expression() {
        // Represent: age > 18 AND name = 'Alice'
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Column("age".to_string())),
                op: Operator::Gt,
                right: Box::new(Expression::Literal(Value::Integer(18))),
            }),
            op: Operator::And,
            right: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Column("name".to_string())),
                op: Operator::Eq,
                right: Box::new(Expression::Literal(
                    Value::String("Alice".to_string()),
                )),
            }),
        };

        // The top-level is an AND
        if let Expression::BinaryOp { op, .. } = &expr {
            assert_eq!(op, &Operator::And);
        } else {
            panic!("Expected BinaryOp");
        }
    }

    #[test]
    fn insert_ast() {
        // Represent: INSERT INTO users (name, age) VALUES ('Bob', 25)
        let ast = Statement::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            values: vec![
                Expression::Literal(Value::String("Bob".to_string())),
                Expression::Literal(Value::Integer(25)),
            ],
        };

        if let Statement::Insert { table, columns, values } = &ast {
            assert_eq!(table, "users");
            assert_eq!(columns.len(), 2);
            assert_eq!(values.len(), 2);
        }
    }

    #[test]
    fn value_display() {
        assert_eq!(Value::Integer(42).to_string(), "42");
        assert_eq!(
            Value::String("hello".to_string()).to_string(),
            "'hello'"
        );
        assert_eq!(Value::Boolean(true).to_string(), "TRUE");
        assert_eq!(Value::Null.to_string(), "NULL");
    }
}
```

Run the tests:

```
$ cargo test ast_tests
running 4 tests
test parser::ast_tests::simple_select_ast ... ok
test parser::ast_tests::nested_expression ... ok
test parser::ast_tests::insert_ast ... ok
test parser::ast_tests::value_display ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

> **Common Mistakes**
>
> 1. **Forgetting `.to_string()`**: String literals like `"hello"` are `&str` (borrowed), but our types use `String` (owned). You need `.to_string()` to convert. If you forget, the compiler will say: "expected `String`, found `&str`."
>
> 2. **Forgetting `Box::new()`** in BinaryOp: The fields `left` and `right` are `Box<Expression>`, not `Expression`. Wrap each child with `Box::new(...)`.
>
> 3. **Missing `&` in pattern matching**: When matching on `&ast` (a reference), the destructured fields are also references. Use `&` in the pattern or compare with references.

---

## Exercise 2: Build the Parser Foundation

**Goal:** Build the `Parser` struct with token navigation methods, and parse the simplest possible query: `SELECT * FROM table`.

### Step 1: Define the Parser struct

The parser needs two things: the list of tokens (from the lexer) and a position tracking where we are in that list.

```rust
/// The SQL parser. Converts a token stream into an AST.
///
/// Think of the parser as someone reading a book with their finger.
/// The finger (position) moves forward one word (token) at a time.
/// At each step, the parser looks at the current word to decide
/// what to do next.
pub struct Parser {
    /// The tokens to parse (produced by the lexer)
    tokens: Vec<Token>,
    /// Current position in the token stream (the "finger")
    position: usize,
}
```

This structure should feel familiar -- our lexer had a similar design with `chars` and `position`. Parsing is the same activity at a higher level: the lexer reads characters and produces tokens; the parser reads tokens and produces AST nodes.

### Step 2: Implement navigation methods

The parser needs several ways to interact with the token stream. Let us build them one at a time.

**Peek** -- look at the current token without moving forward:

```rust
impl Parser {
    /// Create a new parser for the given tokens.
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, position: 0 }
    }

    /// Look at the current token without consuming it.
    /// If we are past the end, return EOF.
    fn peek(&self) -> &Token {
        self.tokens
            .get(self.position)
            .unwrap_or(&Token::EOF)
    }
```

`peek()` uses `.get()` instead of indexing (`self.tokens[self.position]`) because `.get()` returns `Option<&Token>` -- it is safe even if the position is past the end. `.unwrap_or(&Token::EOF)` says "if there is no token at this position, pretend we see EOF (end of file)."

> **What just happened?**
>
> `Vec::get(index)` returns `Option<&T>` -- it returns `Some(&item)` if the index is valid, or `None` if it is out of bounds. This is safer than `vec[index]`, which would panic if the index is out of bounds. `unwrap_or` provides a fallback value when the Option is None.

**Advance** -- consume the current token and move to the next:

```rust
    /// Consume the current token and advance to the next.
    fn advance(&mut self) -> &Token {
        let token = self.tokens
            .get(self.position)
            .unwrap_or(&Token::EOF);
        self.position += 1;
        token
    }
```

Notice this takes `&mut self` (a mutable reference) because it changes `self.position`. `peek()` only takes `&self` (an immutable reference) because it does not change anything.

**Expect** -- consume the current token, but only if it matches what we expect:

```rust
    /// Consume the current token if it matches the expected token.
    /// Returns an error if it does not match.
    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let actual = self.peek().clone();
        if actual == *expected {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "Expected {}, found {} at position {}",
                expected, actual, self.position
            ))
        }
    }
```

This returns `Result<(), String>`. The `Result` type is how Rust handles operations that can fail:
- `Ok(())` means "success, nothing to return"
- `Err(message)` means "something went wrong, here is why"

We use `.clone()` on `self.peek()` because `peek()` returns a reference (`&Token`), but we need an owned `Token` to compare and potentially include in the error message. Cloning creates an independent copy.

**Helper methods** -- shortcuts for common patterns:

```rust
    /// Consume the current token if it is the expected keyword.
    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), String> {
        self.expect(&Token::Keyword(keyword))
    }

    /// Check if the current token matches, and advance if so.
    /// Returns true if it matched, false otherwise.
    fn match_token(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Check if the current token is the given keyword (without consuming it).
    fn check_keyword(&self, keyword: Keyword) -> bool {
        *self.peek() == Token::Keyword(keyword)
    }

    /// Consume the current token and return it as an identifier string.
    /// Returns an error if the current token is not an identifier.
    fn expect_ident(&mut self) -> Result<String, String> {
        match self.peek().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(format!(
                "Expected identifier, found {} at position {}",
                other, self.position
            )),
        }
    }
}
```

Each method serves a specific purpose:

| Method | Does what | Use when |
|--------|-----------|----------|
| `peek()` | Look without consuming | Making decisions: "what comes next?" |
| `advance()` | Consume unconditionally | You already know what the token is |
| `expect()` | Consume and validate | Required syntax: FROM *must* follow column list |
| `match_token()` | Consume conditionally | Optional syntax: WHERE *might* be present |
| `expect_ident()` | Consume and extract name | You need a table or column name |

> **Common Mistakes**
>
> 1. **Calling `advance()` when you meant `peek()`**: This consumes the token, and you cannot go back. If you just want to look, use `peek()`.
>
> 2. **Forgetting the `?` operator on `expect()`**: `expect()` returns a `Result`. If you forget `?`, you will get a warning about an unused `Result` and the parser will continue even on errors.
>
> 3. **Off-by-one errors**: After `advance()`, the position is one past the token you just consumed. If you need the token you consumed, use the return value of `advance()`, not `peek()`.

### Step 3: The main parse entry point

Now we connect the lexer to the parser:

```rust
impl Parser {
    /// Parse a SQL string into a Statement.
    /// This is the main entry point -- give it SQL, get back an AST.
    pub fn parse(input: &str) -> Result<Statement, String> {
        use crate::lexer::Lexer;

        // Step 1: Lex the input into tokens
        let tokens = Lexer::tokenize(input)?;

        // Step 2: Parse the tokens into an AST
        let mut parser = Parser::new(tokens);
        let statement = parser.parse_statement()?;

        // Step 3: Consume optional trailing semicolon
        parser.match_token(&Token::Semicolon);

        // Step 4: Make sure we consumed ALL tokens
        if *parser.peek() != Token::EOF {
            return Err(format!(
                "Unexpected token {} after statement at position {}",
                parser.peek(), parser.position
            ));
        }

        Ok(statement)
    }

    /// Parse a single statement by looking at the first keyword.
    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek().clone() {
            Token::Keyword(Keyword::Select) => self.parse_select(),
            Token::Keyword(Keyword::Insert) => self.parse_insert(),
            Token::Keyword(Keyword::Create) => self.parse_create_table(),
            other => Err(format!(
                "Expected a statement (SELECT, INSERT, CREATE), found {}",
                other
            )),
        }
    }
}
```

The `?` operator is a shortcut for error handling. When you write `Lexer::tokenize(input)?`, it means: "call `tokenize`. If it returns `Ok(tokens)`, unwrap the value and continue. If it returns `Err(e)`, immediately return `Err(e)` from the current function." This saves you from writing repetitive `match` blocks.

> **What just happened?**
>
> We built a two-stage pipeline: SQL string goes into the lexer, tokens come out; tokens go into the parser, an AST comes out. The `parse()` function orchestrates both stages. `parse_statement()` uses a technique called **recursive descent parsing** -- it looks at the first token to decide which kind of statement to parse, then calls a specialized function for that statement type. We will write `parse_select()`, `parse_insert()`, and `parse_create_table()` next.

### Step 4: Parse SELECT statements

Let us start with the most important SQL statement. We will handle three forms:

- `SELECT * FROM users`
- `SELECT name, age FROM users`
- `SELECT name FROM users WHERE age > 18`

```rust
impl Parser {
    /// Parse: SELECT columns FROM table [WHERE condition]
    fn parse_select(&mut self) -> Result<Statement, String> {
        // Consume the SELECT keyword (we already peeked at it)
        self.expect_keyword(Keyword::Select)?;

        // Parse the column list
        let columns = self.parse_select_columns()?;

        // Expect and consume FROM
        self.expect_keyword(Keyword::From)?;

        // Get the table name
        let from = self.expect_ident()?;

        // Optionally parse WHERE clause
        let where_clause = if self.match_token(
            &Token::Keyword(Keyword::Where)
        ) {
            // WHERE keyword was present and consumed
            Some(self.parse_expression(0)?)
        } else {
            // No WHERE keyword -- that is fine
            None
        };

        Ok(Statement::Select {
            columns,
            from,
            where_clause,
        })
    }
```

Notice the pattern: consume expected tokens, extract values, return the AST node. The WHERE clause is optional -- `match_token` returns `true` and consumes the WHERE token if present, or returns `false` and does nothing if absent.

Now parse the column list:

```rust
    /// Parse the columns after SELECT.
    /// Handles: * or col1, col2, col3
    fn parse_select_columns(&mut self) -> Result<Vec<Expression>, String> {
        // Check for SELECT *
        if self.match_token(&Token::Star) {
            // For now, represent * as a special column name
            return Ok(vec![Expression::Column("*".to_string())]);
        }

        // Parse comma-separated column expressions
        let mut columns = Vec::new();

        // Parse the first column (there must be at least one)
        columns.push(self.parse_expression(0)?);

        // Parse additional columns separated by commas
        while self.match_token(&Token::Comma) {
            columns.push(self.parse_expression(0)?);
        }

        Ok(columns)
    }
}
```

The comma-parsing pattern is one you will use over and over: parse the first item, then loop on "comma followed by another item." This naturally handles lists of any length: `name` (one column), `name, age` (two columns), `name, age, email` (three columns), and so on.

> **What just happened?**
>
> We parsed a SELECT statement by breaking it into pieces: first the keyword, then the columns, then FROM, then the table name, then optionally WHERE. Each piece is handled by a small, focused method. This divide-and-conquer approach is the essence of recursive descent parsing.

### Step 5: Write a test for basic SELECT

Let us verify our parser works so far:

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parse_select_star() {
        let stmt = Parser::parse("SELECT * FROM users").unwrap();
        match stmt {
            Statement::Select { columns, from, where_clause } => {
                assert_eq!(columns.len(), 1);
                assert_eq!(from, "users");
                assert!(where_clause.is_none());
            }
            _ => panic!("Expected SELECT statement"),
        }
    }
}
```

We are calling `.unwrap()` in tests -- this will panic with an error message if parsing fails, which is exactly what we want in a test. If the parse succeeds, we destructure the `Statement::Select` and verify each piece.

Do not run this test yet -- we still need `parse_expression()`, which we will build in the next exercise.

---

## Exercise 3: Parse Expressions

**Goal:** Build an expression parser that handles literals, column names, and binary operations with correct operator precedence.

### Step 1: What is operator precedence?

Consider the expression `2 + 3 * 4`. Should this be `(2 + 3) * 4 = 20` or `2 + (3 * 4) = 14`? Mathematics says multiplication has higher precedence than addition, so the answer is 14. Our parser needs to know this too.

We assign each operator a **precedence level** -- a number indicating how tightly it binds. Higher numbers bind tighter:

```rust
impl Operator {
    /// Return the precedence level of this operator.
    /// Higher numbers bind tighter.
    fn precedence(&self) -> u8 {
        match self {
            Operator::Or => 1,         // lowest -- binds loosest
            Operator::And => 2,
            Operator::Eq | Operator::NotEq => 3,
            Operator::Lt | Operator::Gt
                | Operator::LtEq | Operator::GtEq => 4,
            Operator::Add | Operator::Sub => 5,
            Operator::Mul | Operator::Div => 6,  // highest -- binds tightest
            Operator::Not => 7,
        }
    }
}
```

Think of precedence like glue strength. `*` has stronger glue than `+`, so `3 * 4` sticks together before `2 + ...` can grab the `3`. In `age > 18 AND name = 'Alice'`, the comparisons (`>` and `=`) have stronger glue than `AND`, so they form their own groups first.

### Step 2: Parse primary expressions (the leaves)

Before we handle operators, let us parse the simplest expressions -- the leaves of the tree:

```rust
impl Parser {
    /// Parse a primary expression: a literal, column name, or
    /// parenthesized expression.
    fn parse_primary(&mut self) -> Result<Expression, String> {
        match self.peek().clone() {
            // Integer literal: 42
            Token::Integer(n) => {
                self.advance();
                Ok(Expression::Literal(Value::Integer(n)))
            }

            // String literal: 'hello'
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Expression::Literal(Value::String(s)))
            }

            // Boolean literals: TRUE, FALSE
            Token::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression::Literal(Value::Boolean(true)))
            }
            Token::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression::Literal(Value::Boolean(false)))
            }

            // NULL literal
            Token::Keyword(Keyword::Null) => {
                self.advance();
                Ok(Expression::Literal(Value::Null))
            }

            // NOT expression (unary operator)
            Token::Keyword(Keyword::Not) => {
                self.advance();
                let expr = self.parse_expression(
                    Operator::Not.precedence()
                )?;
                Ok(Expression::UnaryOp {
                    op: Operator::Not,
                    expr: Box::new(expr),
                })
            }

            // Negative number: -42
            Token::Minus => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expression::UnaryOp {
                    op: Operator::Sub,
                    expr: Box::new(expr),
                })
            }

            // Parenthesized expression: (age + 1)
            Token::LeftParen => {
                self.advance(); // consume (
                let expr = self.parse_expression(0)?;
                self.expect(&Token::RightParen)?; // consume )
                Ok(expr)
            }

            // Identifier: column name like "age" or "name"
            Token::Ident(name) => {
                self.advance();
                Ok(Expression::Column(name))
            }

            other => Err(format!(
                "Expected expression, found {} at position {}",
                other, self.position
            )),
        }
    }
```

This is a big `match` block, but each arm is simple: peek at the current token, decide what kind of expression it starts, consume the token, and return the corresponding AST node.

The parenthesized expression case is interesting -- it uses recursion. `(age + 1)` is parsed by consuming `(`, then calling `parse_expression(0)` to parse everything inside, then consuming `)`. This is why the technique is called "recursive descent" -- the parser calls itself to handle nested structures.

> **What just happened?**
>
> `parse_primary()` handles the "atoms" of expressions -- the things that cannot be broken down further. Literals and column names are straightforward. Parenthesized expressions use recursion: the parser calls `parse_expression()` to handle whatever is inside the parentheses. NOT and negation are unary operators -- they take a single operand, so we parse one expression after them and wrap it in `UnaryOp`.

### Step 3: Parse binary expressions with precedence

Now the main expression parser. This is called **precedence climbing** -- we climb up from low-precedence operators to high-precedence ones:

```rust
    /// Parse an expression, respecting operator precedence.
    ///
    /// min_precedence: only parse operators with at least this
    /// precedence level. This is how we handle "tighter" operators first.
    fn parse_expression(
        &mut self,
        min_precedence: u8,
    ) -> Result<Expression, String> {
        // Start by parsing the left-hand side (a primary expression)
        let mut left = self.parse_primary()?;

        // Now look for binary operators
        loop {
            // Try to get the operator from the current token
            let op = match self.peek_operator() {
                Some(op) => op,
                None => break, // No operator -- we are done
            };

            // Check if this operator's precedence is high enough
            let prec = op.precedence();
            if prec < min_precedence {
                break; // This operator binds too loosely -- let the caller handle it
            }

            // Consume the operator token
            self.advance();

            // Parse the right-hand side with HIGHER minimum precedence.
            // This ensures that tighter-binding operators on the right
            // are handled first.
            let right = self.parse_expression(prec + 1)?;

            // Combine into a BinaryOp node
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Try to interpret the current token as a binary operator.
    /// Returns None if the current token is not an operator.
    fn peek_operator(&self) -> Option<Operator> {
        match self.peek() {
            Token::Plus => Some(Operator::Add),
            Token::Minus => Some(Operator::Sub),
            Token::Star => Some(Operator::Mul),
            Token::Slash => Some(Operator::Div),
            Token::Eq => Some(Operator::Eq),
            Token::NotEq => Some(Operator::NotEq),
            Token::Lt => Some(Operator::Lt),
            Token::Gt => Some(Operator::Gt),
            Token::LtEq => Some(Operator::LtEq),
            Token::GtEq => Some(Operator::GtEq),
            Token::Keyword(Keyword::And) => Some(Operator::And),
            Token::Keyword(Keyword::Or) => Some(Operator::Or),
            _ => None,
        }
    }
}
```

Let us trace through `2 + 3 * 4` to see how precedence works:

1. `parse_expression(0)` is called
2. `parse_primary()` returns `Literal(2)`
3. We see `+` (precedence 5). Is `5 >= 0`? Yes. Consume `+`.
4. Recurse: `parse_expression(6)` (precedence 5 + 1 = 6)
   - `parse_primary()` returns `Literal(3)`
   - We see `*` (precedence 6). Is `6 >= 6`? Yes. Consume `*`.
   - Recurse: `parse_expression(7)`
     - `parse_primary()` returns `Literal(4)`
     - No more operators. Return `Literal(4)`.
   - Combine: `BinaryOp(3, Mul, 4)`. Return this.
5. Back in step 4: no more operators with precedence >= 6. Return `BinaryOp(3, Mul, 4)`.
6. Combine: `BinaryOp(2, Add, BinaryOp(3, Mul, 4))`. Return this.

The result is `2 + (3 * 4)` -- multiplication binds tighter, just like math.

> **What just happened?**
>
> Precedence climbing works by recursive calls with increasing minimum precedence. When we see `+` (precedence 5), we recurse with `min_precedence = 6`. This means the recursive call will only "eat" operators with precedence >= 6 (like `*`). Operators with precedence < 6 (like another `+`) are left for the outer call to handle. This naturally groups high-precedence operations first.

> **Common Mistakes**
>
> 1. **Using `prec` instead of `prec + 1` in the recursive call**: This would make operators left-associative for same-precedence operators, which is usually correct (a + b + c = (a + b) + c). But if you use just `prec`, you might get infinite recursion with some parser designs.
>
> 2. **Forgetting to handle missing operators**: Without the `None => break` case, the parser would panic when it reaches a non-operator token like `FROM` or `)`.

---

## Exercise 4: Parse INSERT and CREATE TABLE

**Goal:** Extend the parser to handle INSERT and CREATE TABLE statements.

### Step 1: Parse INSERT statements

INSERT has the form: `INSERT INTO table (col1, col2) VALUES (val1, val2)`

```rust
impl Parser {
    /// Parse: INSERT INTO table (columns) VALUES (values)
    fn parse_insert(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Insert)?;
        self.expect_keyword(Keyword::Into)?;

        // Get table name
        let table = self.expect_ident()?;

        // Parse column list: (col1, col2, ...)
        self.expect(&Token::LeftParen)?;
        let mut columns = Vec::new();
        columns.push(self.expect_ident()?);
        while self.match_token(&Token::Comma) {
            columns.push(self.expect_ident()?);
        }
        self.expect(&Token::RightParen)?;

        // Parse VALUES keyword
        self.expect_keyword(Keyword::Values)?;

        // Parse value list: (val1, val2, ...)
        self.expect(&Token::LeftParen)?;
        let mut values = Vec::new();
        values.push(self.parse_expression(0)?);
        while self.match_token(&Token::Comma) {
            values.push(self.parse_expression(0)?);
        }
        self.expect(&Token::RightParen)?;

        // Verify column count matches value count
        if columns.len() != values.len() {
            return Err(format!(
                "Column count ({}) does not match value count ({})",
                columns.len(),
                values.len()
            ));
        }

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }
}
```

Notice the pattern for parsing comma-separated lists inside parentheses:
1. Consume `(`
2. Parse the first item
3. Loop: if the next token is `,`, consume it and parse another item
4. Consume `)`

This is the same pattern we used for SELECT columns. You will see it again in CREATE TABLE.

### Step 2: Parse CREATE TABLE statements

CREATE TABLE has the form: `CREATE TABLE name (col1 TYPE, col2 TYPE, ...)`

```rust
impl Parser {
    /// Parse: CREATE TABLE name (col1 TYPE, col2 TYPE, ...)
    fn parse_create_table(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Create)?;
        self.expect_keyword(Keyword::Table)?;

        // Get table name
        let name = self.expect_ident()?;

        // Parse column definitions
        self.expect(&Token::LeftParen)?;

        let mut columns = Vec::new();
        columns.push(self.parse_column_def()?);
        while self.match_token(&Token::Comma) {
            columns.push(self.parse_column_def()?);
        }

        self.expect(&Token::RightParen)?;

        Ok(Statement::CreateTable { name, columns })
    }

    /// Parse a single column definition: name TYPE
    fn parse_column_def(&mut self) -> Result<ColumnDef, String> {
        let name = self.expect_ident()?;
        let data_type = self.parse_data_type()?;
        Ok(ColumnDef { name, data_type })
    }

    /// Parse a data type keyword: INTEGER, TEXT, BOOLEAN
    fn parse_data_type(&mut self) -> Result<DataType, String> {
        match self.peek().clone() {
            Token::Keyword(Keyword::Integer) => {
                self.advance();
                Ok(DataType::Integer)
            }
            Token::Keyword(Keyword::Text) => {
                self.advance();
                Ok(DataType::Text)
            }
            Token::Keyword(Keyword::Boolean) => {
                self.advance();
                Ok(DataType::Boolean)
            }
            other => Err(format!(
                "Expected data type (INTEGER, TEXT, BOOLEAN), found {}",
                other
            )),
        }
    }
}
```

> **What just happened?**
>
> CREATE TABLE parsing follows the same recursive descent pattern: consume keywords, extract identifiers, parse sub-structures (column definitions). Each column definition is itself a small parsing task (name followed by type), so we extract it into `parse_column_def()`. This keeps each method focused on one thing.

### Step 3: Test INSERT and CREATE TABLE

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;

    // ... (previous tests) ...

    #[test]
    fn parse_insert() {
        let stmt = Parser::parse(
            "INSERT INTO users (name, age) VALUES ('Alice', 30)"
        ).unwrap();

        match stmt {
            Statement::Insert { table, columns, values } => {
                assert_eq!(table, "users");
                assert_eq!(columns, vec!["name", "age"]);
                assert_eq!(values.len(), 2);
                // Check first value is the string 'Alice'
                assert_eq!(
                    values[0],
                    Expression::Literal(Value::String("Alice".to_string()))
                );
                // Check second value is the integer 30
                assert_eq!(
                    values[1],
                    Expression::Literal(Value::Integer(30))
                );
            }
            _ => panic!("Expected INSERT statement"),
        }
    }

    #[test]
    fn parse_create_table() {
        let stmt = Parser::parse(
            "CREATE TABLE users (name TEXT, age INTEGER, active BOOLEAN)"
        ).unwrap();

        match stmt {
            Statement::CreateTable { name, columns } => {
                assert_eq!(name, "users");
                assert_eq!(columns.len(), 3);
                assert_eq!(columns[0].name, "name");
                assert_eq!(columns[0].data_type, DataType::Text);
                assert_eq!(columns[1].name, "age");
                assert_eq!(columns[1].data_type, DataType::Integer);
                assert_eq!(columns[2].name, "active");
                assert_eq!(columns[2].data_type, DataType::Boolean);
            }
            _ => panic!("Expected CREATE TABLE statement"),
        }
    }

    #[test]
    fn parse_select_with_where() {
        let stmt = Parser::parse(
            "SELECT name FROM users WHERE age > 18"
        ).unwrap();

        match stmt {
            Statement::Select { columns, from, where_clause } => {
                assert_eq!(from, "users");
                assert!(where_clause.is_some());
                // The WHERE clause should be: age > 18
                let wc = where_clause.unwrap();
                match wc {
                    Expression::BinaryOp { left, op, right } => {
                        assert_eq!(*left, Expression::Column("age".to_string()));
                        assert_eq!(op, Operator::Gt);
                        assert_eq!(*right, Expression::Literal(Value::Integer(18)));
                    }
                    _ => panic!("Expected BinaryOp in WHERE clause"),
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn parse_complex_where() {
        let stmt = Parser::parse(
            "SELECT name FROM users WHERE age > 18 AND active = TRUE"
        ).unwrap();

        match stmt {
            Statement::Select { where_clause: Some(wc), .. } => {
                // Top level should be AND
                match wc {
                    Expression::BinaryOp { op, .. } => {
                        assert_eq!(op, Operator::And);
                    }
                    _ => panic!("Expected AND at top level"),
                }
            }
            _ => panic!("Expected SELECT with WHERE"),
        }
    }

    #[test]
    fn parse_error_missing_from() {
        let result = Parser::parse("SELECT name WHERE age > 18");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Expected"), "Error: {}", err);
    }

    #[test]
    fn parse_precedence() {
        // 2 + 3 * 4 should be 2 + (3 * 4), not (2 + 3) * 4
        let stmt = Parser::parse("SELECT 2 + 3 * 4 FROM t").unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                match &columns[0] {
                    Expression::BinaryOp { op, right, .. } => {
                        // Top level should be Add
                        assert_eq!(*op, Operator::Add);
                        // Right side should be Mul
                        match right.as_ref() {
                            Expression::BinaryOp { op, .. } => {
                                assert_eq!(*op, Operator::Mul);
                            }
                            _ => panic!("Expected Mul on right side"),
                        }
                    }
                    _ => panic!("Expected BinaryOp"),
                }
            }
            _ => panic!("Expected SELECT"),
        }
    }
}
```

Run the tests:

```
$ cargo test parser_tests
running 6 tests
test parser::parser_tests::parse_select_star ... ok
test parser::parser_tests::parse_insert ... ok
test parser::parser_tests::parse_create_table ... ok
test parser::parser_tests::parse_select_with_where ... ok
test parser::parser_tests::parse_complex_where ... ok
test parser::parser_tests::parse_error_missing_from ... ok
test parser::parser_tests::parse_precedence ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

---

## Exercise 5: Display the AST

**Goal:** Implement `Display` for `Expression` and `Statement` so we can print ASTs in a readable format.

### Step 1: Display for Expression

Being able to print the AST is valuable for debugging. When a query produces wrong results, seeing the parsed AST helps you figure out if the parser misunderstood the SQL.

```rust
impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Column(name) => write!(f, "{}", name),
            Expression::Literal(value) => write!(f, "{}", value),
            Expression::BinaryOp { left, op, right } => {
                write!(f, "({} {} {})", left, op, right)
            }
            Expression::UnaryOp { op, expr } => {
                write!(f, "({} {})", op, expr)
            }
        }
    }
}
```

Notice how `BinaryOp` adds parentheses around the expression. This makes the precedence explicit: `age > 18 AND name = 'Alice'` prints as `((age > 18) AND (name = 'Alice'))`, showing exactly how the tree is structured.

Also notice that `write!(f, "{}", left)` works even though `left` is `Box<Expression>`. This is auto-dereferencing again -- `Box<Expression>` implements `Display` because `Expression` implements `Display`, thanks to the `Deref` trait.

### Step 2: Display for Statement

```rust
impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Select { columns, from, where_clause } => {
                let col_str: Vec<String> = columns
                    .iter()
                    .map(|c| c.to_string())
                    .collect();
                write!(f, "SELECT {} FROM {}", col_str.join(", "), from)?;
                if let Some(wc) = where_clause {
                    write!(f, " WHERE {}", wc)?;
                }
                Ok(())
            }
            Statement::Insert { table, columns, values } => {
                let val_str: Vec<String> = values
                    .iter()
                    .map(|v| v.to_string())
                    .collect();
                write!(
                    f,
                    "INSERT INTO {} ({}) VALUES ({})",
                    table,
                    columns.join(", "),
                    val_str.join(", ")
                )
            }
            Statement::CreateTable { name, columns } => {
                let col_defs: Vec<String> = columns
                    .iter()
                    .map(|c| format!("{} {}", c.name, c.data_type))
                    .collect();
                write!(
                    f,
                    "CREATE TABLE {} ({})",
                    name,
                    col_defs.join(", ")
                )
            }
        }
    }
}
```

This code uses a few iterator methods we have not seen before:

- `.iter()` creates an iterator over references to the items in a `Vec`
- `.map(|c| c.to_string())` transforms each item by calling `.to_string()` on it
- `.collect()` gathers the results into a new `Vec<String>`
- `.join(", ")` combines all strings with `, ` between them

We will learn iterators in depth in the next chapter. For now, just know that this is a common Rust pattern for transforming a list of items into a formatted string.

### Step 3: Test Display

```rust
#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn display_select() {
        let stmt = Parser::parse(
            "SELECT name, age FROM users WHERE age > 18"
        ).unwrap();
        let displayed = stmt.to_string();
        assert!(displayed.contains("SELECT"));
        assert!(displayed.contains("FROM users"));
        assert!(displayed.contains("WHERE"));
    }

    #[test]
    fn display_roundtrip() {
        // Parse SQL, display it, and verify it looks reasonable
        let original = "SELECT name FROM users WHERE age > 18";
        let stmt = Parser::parse(original).unwrap();
        let displayed = stmt.to_string();
        println!("Original:  {}", original);
        println!("Displayed: {}", displayed);
        // The displayed version will have extra parentheses
        // from the expression display, which is fine
        assert!(displayed.contains("name"));
        assert!(displayed.contains("users"));
    }
}
```

---

## Exercise 6: End-to-End Pipeline

**Goal:** Connect the lexer and parser into a complete pipeline and test it with real SQL queries.

### Step 1: The complete pipeline

Our pipeline now looks like this:

```
SQL string: "SELECT name FROM users WHERE age > 18"
                    │
                    ▼
              ┌──────────┐
              │  Lexer    │  Chapter 6
              └────┬─────┘
                   │
                   ▼
    Tokens: [SELECT, name, FROM, users, WHERE, age, >, 18]
                   │
                   ▼
              ┌──────────┐
              │  Parser   │  Chapter 7 (this chapter)
              └────┬─────┘
                   │
                   ▼
              AST: Select {
                columns: [Column("name")],
                from: "users",
                where_clause: Some(BinaryOp {
                    left: Column("age"),
                    op: Gt,
                    right: Literal(Integer(18))
                })
              }
```

The `Parser::parse()` function we built already runs this entire pipeline -- it calls the lexer internally and returns an AST.

### Step 2: Test the pipeline with various SQL

```rust
#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn pipeline_select_star() {
        let ast = Parser::parse("SELECT * FROM users;").unwrap();
        println!("{}", ast);
    }

    #[test]
    fn pipeline_select_where_and() {
        let ast = Parser::parse(
            "SELECT name, email FROM users WHERE age > 18 AND active = TRUE"
        ).unwrap();
        println!("{}", ast);
    }

    #[test]
    fn pipeline_insert() {
        let ast = Parser::parse(
            "INSERT INTO users (name, age) VALUES ('Alice', 30)"
        ).unwrap();
        println!("{}", ast);
    }

    #[test]
    fn pipeline_create_table() {
        let ast = Parser::parse(
            "CREATE TABLE products (name TEXT, price INTEGER, available BOOLEAN)"
        ).unwrap();
        println!("{}", ast);
    }

    #[test]
    fn pipeline_error_handling() {
        // Missing FROM keyword
        assert!(Parser::parse("SELECT name users").is_err());

        // Missing table name
        assert!(Parser::parse("SELECT * FROM").is_err());

        // Mismatched parentheses
        assert!(Parser::parse("SELECT (1 + 2 FROM t").is_err());

        // Unknown statement
        assert!(Parser::parse("DROP TABLE users").is_err());
    }

    #[test]
    fn pipeline_nested_arithmetic() {
        let ast = Parser::parse(
            "SELECT price * quantity + tax FROM orders"
        ).unwrap();

        // price * quantity should bind tighter than + tax
        match ast {
            Statement::Select { columns, .. } => {
                match &columns[0] {
                    Expression::BinaryOp { op, left, .. } => {
                        assert_eq!(*op, Operator::Add);
                        // Left side should be the multiplication
                        match left.as_ref() {
                            Expression::BinaryOp { op, .. } => {
                                assert_eq!(*op, Operator::Mul);
                            }
                            _ => panic!("Expected Mul"),
                        }
                    }
                    _ => panic!("Expected BinaryOp"),
                }
            }
            _ => panic!("Expected SELECT"),
        }
    }
}
```

Run all the tests:

```
$ cargo test
running 17 tests
...
test result: ok. 17 passed; 0 failed; 0 ignored
```

---

## Exercises for Practice

1. **Add UPDATE support**: Implement `parse_update()` for `UPDATE table SET col = val WHERE condition`. Add an `Update` variant to `Statement` with fields for table, assignments (`Vec<(String, Expression)>`), and optional WHERE clause.

   *Hint: Parse assignments as a comma-separated list of `name = expression` pairs. Use the same comma loop pattern from INSERT column parsing.*

2. **Add DELETE support**: Implement `parse_delete()` for `DELETE FROM table WHERE condition`. This is the simplest statement to parse -- it is just `DELETE FROM table` with an optional WHERE clause.

   *Hint: The structure is almost identical to SELECT, but simpler -- no column list needed.*

3. **Add parenthesized expressions in WHERE**: Verify that `SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3` parses correctly with the OR grouped inside parentheses. Write a test that checks the AST structure.

   *Hint: Your `parse_primary()` already handles parentheses -- `LeftParen` recursively calls `parse_expression(0)`, which resets the minimum precedence to 0 inside the parentheses.*

4. **Error message improvement**: Make error messages include the SQL context. Instead of "Expected FROM, found WHERE at position 3," produce "Expected FROM, found WHERE at position 3 in: SELECT name WHERE age > 18."

   *Hint: Store the original SQL string (or a reference to it) in the Parser struct, and include it in error messages.*

5. **AST walker**: Write a function `fn count_columns(expr: &Expression) -> usize` that counts how many `Column` references appear in an expression. For `age > 18 AND name = 'Alice'`, it should return 2 (age and name).

   *Hint: Use recursion -- for a `BinaryOp`, count columns in the left side plus columns in the right side. For a `Column`, return 1. For a `Literal`, return 0.*
