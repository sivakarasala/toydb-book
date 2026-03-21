# Chapter 7: SQL Parser — Building the AST

A stream of tokens is not a query. `[SELECT, name, FROM, users, WHERE, age, >, 18]` is a flat list — it tells you what words appear but not how they relate. Is `age > 18` a WHERE clause filter, or is it an expression inside a SELECT? Does `name` refer to a column being selected, or a table being queried? The tokens carry no structure. Structure is what a parser adds.

This chapter transforms the flat token stream from Chapter 6 into an Abstract Syntax Tree (AST) — a tree-shaped data structure where each node represents a meaningful piece of the SQL statement. `SELECT name FROM users WHERE age > 18` becomes a `Statement::Select` node with a column list `[name]`, a table `users`, and a WHERE clause that is itself a tree: `BinaryOp { left: Column("age"), op: Gt, right: Literal(Integer(18)) }`. The parser enforces grammar rules: it accepts `SELECT name FROM users` and rejects `SELECT FROM name users`. It is the difference between recognizing English words and understanding an English sentence.

By the end of this chapter, you will have:

- An `Expression` enum that uses `Box` for recursive variants like `BinaryOp` and `UnaryOp`
- A `Statement` enum covering SELECT, INSERT, UPDATE, DELETE, and CREATE TABLE
- A `Parser` struct with `peek()`, `advance()`, and `expect()` methods that consume tokens
- Precedence climbing (Pratt parsing) for correct operator precedence in WHERE clauses
- A complete test suite parsing real SQL into verified ASTs

---

## Spotlight: Recursive Types & Box

Every chapter has one spotlight concept. This chapter's spotlight is **recursive types and `Box<T>`** — the mechanism Rust uses to make tree-shaped data structures possible.

### The infinite size problem

Consider this attempt at an expression type:

```rust
// This does NOT compile
enum Expression {
    Literal(i64),
    Add {
        left: Expression,   // How big is this?
        right: Expression,  // And this?
    },
}
```

Rust stores enum values inline on the stack. To do that, it needs to know the size of every variant at compile time. The size of an enum is the size of its largest variant. `Literal(i64)` is 8 bytes. But `Add`? It contains two `Expression` values, each of which could themselves be `Add`, which contains two more `Expression` values, which could be `Add`, which... this is infinite recursion. The compiler cannot calculate a finite size.

```
error[E0072]: recursive type `Expression` has infinite size
 --> src/lib.rs:1:1
  |
1 | enum Expression {
  | ^^^^^^^^^^^^^^^
2 |     Literal(i64),
3 |     Add {
4 |         left: Expression,
  |               ---------- recursive without indirection
  |
help: insert some indirection (e.g., a `Box`, `Rc`, or `&`) to break the cycle
```

The error message even tells you the fix.

### Box: heap allocation breaks the cycle

`Box<T>` is a pointer to a heap-allocated value. It is always the same size — 8 bytes on a 64-bit system — regardless of what `T` is. By putting the recursive field behind a `Box`, you replace an inline value of unknown size with a pointer of known size:

```rust
enum Expression {
    Literal(i64),           // 8 bytes
    Add {
        left: Box<Expression>,   // 8 bytes (pointer)
        right: Box<Expression>,  // 8 bytes (pointer)
    },
}
```

Now Rust can calculate the size: `Add` is 16 bytes (two pointers), `Literal` is 8 bytes, so `Expression` is 16 bytes plus the enum discriminant. The actual `Expression` values that `left` and `right` point to live on the heap, and they can be as deeply nested as you want.

### Creating and matching boxed values

Creating a boxed value:

```rust
let expr = Expression::Add {
    left: Box::new(Expression::Literal(1)),
    right: Box::new(Expression::Add {
        left: Box::new(Expression::Literal(2)),
        right: Box::new(Expression::Literal(3)),
    }),
};
// Represents: 1 + (2 + 3)
```

Pattern matching on boxed values works the same as unboxed — you just dereference with `*`:

```rust
fn evaluate(expr: &Expression) -> i64 {
    match expr {
        Expression::Literal(n) => *n,
        Expression::Add { left, right } => {
            evaluate(left) + evaluate(right)
        }
    }
}
```

Inside the `Add` arm, `left` and `right` are `&Box<Expression>`. But `Box<T>` implements `Deref<Target = T>`, so you can pass them directly to a function expecting `&Expression`. Rust auto-dereferences the `Box` for you.

### Why not just use references?

You might wonder: why not `left: &Expression` instead of `Box<Expression>`? References borrow data that exists somewhere else. But when building an AST, the parser *creates* the nodes — there is no "somewhere else" to borrow from. `Box` is for owned, heap-allocated data. References are for temporary borrows of existing data. The AST owns its nodes, so it uses `Box`.

### Tree-shaped data is naturally recursive

An AST is a tree. Trees are recursive by definition: a tree is either a leaf or a node with children that are themselves trees. In Rust, "a type that contains itself" requires heap indirection. This is not a limitation — it is the type system making explicit what other languages hide. Every tree in JavaScript, Python, and Go also heap-allocates its children. Rust just makes you say so.

> **Coming from other languages?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Recursive type | Objects nest freely | Objects nest freely | `*Node` pointer | `Box<Node>` |
> | Heap allocation | Automatic (all objects) | Automatic (all objects) | Explicit (`new()`) | Explicit (`Box::new()`) |
> | Stack vs heap | No control | No control | Compiler decides | You decide |
> | Size of a pointer | Hidden | Hidden | 8 bytes | 8 bytes (`Box<T>`) |
> | Deref to inner type | N/A | N/A | Automatic (`*p`) | Automatic (`Deref` trait) |
>
> The key difference: in JavaScript and Python, *everything* is heap-allocated, so recursive types "just work" — but you pay the allocation cost for every value, even simple integers. In Rust, you only heap-allocate when you need to (recursive types, dynamic dispatch, large values). `Box::new()` is Rust saying "I know this needs to go on the heap."

---

## Exercise 1: Define the AST Types

**Goal:** Define the type system that represents parsed SQL — the `Expression`, `Statement`, `Value`, and `Operator` types that form the Abstract Syntax Tree.

### Step 1: Create the parser module

Create `src/parser.rs` and register it in `src/lib.rs`:

```rust
// src/lib.rs
pub mod lexer;
pub mod parser;
```

### Step 2: Define the Value type

SQL has several literal value types. Define them:

```rust
// src/parser.rs
use crate::lexer::{Keyword, Token};
use std::fmt;

/// A SQL value — the leaves of expression trees.
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

### Step 3: Define the Operator type

Operators connect expressions. Arithmetic operators combine numbers; comparison operators produce booleans; logical operators combine booleans:

```rust
/// Operators in SQL expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    // Comparison
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    // Logical
    And,
    Or,
    Not,
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

This is where `Box` enters. An expression can contain other expressions — `age > 18 AND name = 'Alice'` is an AND of two comparisons, each of which is a comparison of a column and a literal. The nesting is arbitrary: `(a + b) * (c - d) > e AND f = g`.

```rust
/// A SQL expression — the core recursive type of the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A column reference: name, age, users.id
    Column(String),
    /// A literal value: 42, 'hello', TRUE, NULL
    Literal(Value),
    /// A binary operation: left op right
    BinaryOp {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },
    /// A unary operation: NOT expr, -expr
    UnaryOp {
        op: Operator,
        expr: Box<Expression>,
    },
}
```

Look at `BinaryOp`: it has `left: Box<Expression>` and `right: Box<Expression>`. Without `Box`, this would be an infinite-size type — `Expression` contains `Expression` contains `Expression`... With `Box`, each child is a heap pointer (8 bytes). The total size of `Expression` is fixed at compile time.

`Column` and `Literal` are the *leaves* of the tree — they do not contain other expressions. `BinaryOp` and `UnaryOp` are the *internal nodes* — they contain child expressions. This leaf/node distinction is fundamental to all tree data structures.

### Step 5: Define the column definition type

For `CREATE TABLE`, we need to describe columns:

```rust
/// A column definition in a CREATE TABLE statement.
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

### Step 6: Define the Statement type

Each SQL statement type becomes a variant of the `Statement` enum:

```rust
/// A parsed SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// SELECT columns FROM table [WHERE condition]
    Select {
        columns: Vec<Expression>,
        from: String,
        where_clause: Option<Expression>,
    },
    /// INSERT INTO table (columns) VALUES (values)
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Expression>,
    },
    /// UPDATE table SET assignments [WHERE condition]
    Update {
        table: String,
        assignments: Vec<(String, Expression)>,
        where_clause: Option<Expression>,
    },
    /// DELETE FROM table [WHERE condition]
    Delete {
        table: String,
        where_clause: Option<Expression>,
    },
    /// CREATE TABLE name (column_definitions)
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
}
```

Notice that `columns` in `Select` is `Vec<Expression>`, not `Vec<String>`. This is because `SELECT age + 1 FROM users` is valid — the column list can contain expressions, not just column names. We model this correctly from the start.

The `where_clause` fields are `Option<Expression>` — `None` when there is no WHERE clause, `Some(expr)` when there is one. This uses Rust's type system to make the optionality explicit.

### Step 7: Test that the types compile and express real SQL

```rust
#[cfg(test)]
mod ast_tests {
    use super::*;

    #[test]
    fn simple_select_ast() {
        // SELECT name FROM users WHERE age > 18
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
        // age > 18 AND name = 'Alice'
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
                right: Box::new(Expression::Literal(Value::String("Alice".to_string()))),
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
        // INSERT INTO users (name, age) VALUES ('Bob', 25)
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
        assert_eq!(Value::String("hello".to_string()).to_string(), "'hello'");
        assert_eq!(Value::Boolean(true).to_string(), "TRUE");
        assert_eq!(Value::Null.to_string(), "NULL");
    }
}
```

```
$ cargo test ast_tests
running 4 tests
test parser::ast_tests::simple_select_ast ... ok
test parser::ast_tests::nested_expression ... ok
test parser::ast_tests::insert_ast ... ok
test parser::ast_tests::value_display ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why Vec&lt;Expression&gt; for SELECT columns instead of Vec&lt;String&gt;?</summary>

Consider `SELECT age + 1, name FROM users`. The first column is an arithmetic expression, not just a column name. If we used `Vec<String>`, we could not represent this. `Vec<Expression>` handles both simple columns (`Expression::Column("name")`) and computed expressions (`Expression::BinaryOp { ... }`).

This is the strategic approach from Chapter 6's design insight — we model the general case from the start, so we do not have to refactor later when we want to support expressions in SELECT lists.

</details>

---

## Exercise 2: Build the Parser Foundation

**Goal:** Build the `Parser` struct with token navigation methods, and parse the simplest possible query: `SELECT * FROM table`.

### Step 1: Define the Parser struct

```rust
/// The SQL parser. Converts a token stream into an AST.
pub struct Parser {
    /// The tokens to parse (produced by the lexer)
    tokens: Vec<Token>,
    /// Current position in the token stream
    position: usize,
}
```

The parser mirrors the lexer's structure — both hold a sequence (characters vs tokens) and a position. Both have peek/advance methods. This is the same pattern because parsing at any level is the same activity: examine the next element, decide what to do, advance.

### Step 2: Implement navigation methods

```rust
impl Parser {
    /// Create a new parser for the given tokens.
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, position: 0 }
    }

    /// Look at the current token without consuming it.
    fn peek(&self) -> &Token {
        self.tokens
            .get(self.position)
            .unwrap_or(&Token::EOF)
    }

    /// Consume the current token and advance to the next.
    fn advance(&mut self) -> &Token {
        let token = self.tokens
            .get(self.position)
            .unwrap_or(&Token::EOF);
        self.position += 1;
        token
    }

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

    /// Consume the current token if it is the expected keyword.
    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), String> {
        self.expect(&Token::Keyword(keyword))
    }

    /// Check if the current token matches, and advance if so.
    /// Returns true if it matched.
    fn match_token(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Check if the current token is the given keyword.
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

Each method serves a specific purpose in the parsing flow:
- `peek()` looks ahead without consuming — used for decisions ("should I parse a WHERE clause?")
- `advance()` consumes unconditionally — used when you know what the token is
- `expect()` consumes and validates — used for required syntax (`FROM` must follow the column list)
- `match_token()` consumes conditionally — used for optional syntax (`WHERE` may or may not be present)
- `expect_ident()` consumes and extracts — used when you need the identifier's name

### Step 3: Implement the main parse entry point

```rust
impl Parser {
    /// Parse a SQL string into a Statement.
    pub fn parse(input: &str) -> Result<Statement, String> {
        use crate::lexer::Lexer;

        let tokens = Lexer::tokenize(input)?;
        let mut parser = Parser::new(tokens);
        let statement = parser.parse_statement()?;

        // Consume optional semicolon
        parser.match_token(&Token::Semicolon);

        // Ensure we consumed all tokens
        if *parser.peek() != Token::EOF {
            return Err(format!(
                "Unexpected token {} after statement at position {}",
                parser.peek(), parser.position
            ));
        }

        Ok(statement)
    }

    /// Parse a single statement by dispatching on the first keyword.
    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek().clone() {
            Token::Keyword(Keyword::Select) => self.parse_select(),
            Token::Keyword(Keyword::Insert) => self.parse_insert(),
            Token::Keyword(Keyword::Update) => self.parse_update(),
            Token::Keyword(Keyword::Delete) => self.parse_delete(),
            Token::Keyword(Keyword::Create) => self.parse_create_table(),
            other => Err(format!(
                "Expected a statement (SELECT, INSERT, UPDATE, DELETE, CREATE), found {}",
                other
            )),
        }
    }
}
```

The `parse()` method is the public API — give it a SQL string, get back a `Statement`. Internally it delegates to `parse_statement()`, which looks at the first keyword and dispatches to the appropriate parse method. This is **recursive descent parsing**: the parser has a function for each grammar rule, and each function calls others as needed.

### Step 4: Parse SELECT statements

Start simple — parse `SELECT * FROM table` and `SELECT col1, col2 FROM table`:

```rust
impl Parser {
    /// Parse: SELECT columns FROM table [WHERE condition]
    fn parse_select(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Select)?;

        // Parse column list
        let columns = self.parse_select_columns()?;

        // FROM clause
        self.expect_keyword(Keyword::From)?;
        let from = self.expect_ident()?;

        // Optional WHERE clause
        let where_clause = if self.match_token(&Token::Keyword(Keyword::Where)) {
            Some(self.parse_expression(0)?)
        } else {
            None
        };

        Ok(Statement::Select {
            columns,
            from,
            where_clause,
        })
    }

    /// Parse the column list in a SELECT statement.
    /// Handles * (all columns) and comma-separated expressions.
    fn parse_select_columns(&mut self) -> Result<Vec<Expression>, String> {
        // SELECT * means "all columns"
        if self.match_token(&Token::Star) {
            return Ok(vec![Expression::Column("*".to_string())]);
        }

        let mut columns = Vec::new();
        columns.push(self.parse_expression(0)?);

        while self.match_token(&Token::Comma) {
            columns.push(self.parse_expression(0)?);
        }

        Ok(columns)
    }
}
```

Notice the `parse_expression(0)` call — we have not implemented it yet. That comes in Exercise 3. For now, let us add a minimal version that handles columns and literals, so we can test the SELECT parsing:

```rust
impl Parser {
    /// Parse an expression. The min_precedence parameter controls
    /// operator precedence (explained in Exercise 3).
    /// For now, this handles only atoms (columns and literals).
    fn parse_expression(&mut self, _min_precedence: u8) -> Result<Expression, String> {
        self.parse_atom()
    }

    /// Parse an atomic expression (no operators).
    fn parse_atom(&mut self) -> Result<Expression, String> {
        match self.peek().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(Expression::Column(name))
            }
            Token::Number(n) => {
                self.advance();
                Ok(Expression::Literal(Value::Integer(n)))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Expression::Literal(Value::String(s)))
            }
            Token::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression::Literal(Value::Boolean(true)))
            }
            Token::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression::Literal(Value::Boolean(false)))
            }
            Token::Keyword(Keyword::Null) => {
                self.advance();
                Ok(Expression::Literal(Value::Null))
            }
            other => Err(format!(
                "Expected expression, found {} at position {}",
                other, self.position
            )),
        }
    }
}
```

### Step 5: Test basic SELECT parsing

```rust
#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parse_select_star() {
        let ast = Parser::parse("SELECT * FROM users").unwrap();
        assert_eq!(ast, Statement::Select {
            columns: vec![Expression::Column("*".to_string())],
            from: "users".to_string(),
            where_clause: None,
        });
    }

    #[test]
    fn parse_select_columns() {
        let ast = Parser::parse("SELECT name, age FROM users").unwrap();
        assert_eq!(ast, Statement::Select {
            columns: vec![
                Expression::Column("name".to_string()),
                Expression::Column("age".to_string()),
            ],
            from: "users".to_string(),
            where_clause: None,
        });
    }

    #[test]
    fn parse_select_with_semicolon() {
        let ast = Parser::parse("SELECT * FROM users;").unwrap();
        assert_eq!(ast, Statement::Select {
            columns: vec![Expression::Column("*".to_string())],
            from: "users".to_string(),
            where_clause: None,
        });
    }

    #[test]
    fn error_missing_from() {
        let err = Parser::parse("SELECT name").unwrap_err();
        assert!(err.contains("Expected FROM"), "Error was: {}", err);
    }

    #[test]
    fn error_empty_input() {
        let err = Parser::parse("").unwrap_err();
        assert!(err.contains("Expected a statement"), "Error was: {}", err);
    }

    #[test]
    fn error_garbage_after_statement() {
        let err = Parser::parse("SELECT * FROM users garbage").unwrap_err();
        assert!(err.contains("Unexpected token"), "Error was: {}", err);
    }
}
```

```
$ cargo test parser_tests
running 6 tests
test parser::parser_tests::parse_select_star ... ok
test parser::parser_tests::parse_select_columns ... ok
test parser::parser_tests::parse_select_with_semicolon ... ok
test parser::parser_tests::error_missing_from ... ok
test parser::parser_tests::error_empty_input ... ok
test parser::parser_tests::error_garbage_after_statement ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why does peek() return &Token instead of Token?</summary>

Returning a reference avoids cloning the token every time you look at it. Most peeks are just checking which variant the token is — you do not need to own it for that. We clone only when we need to extract data from the token (like the string inside `Ident`), which is a deliberate choice to minimize allocations.

You will notice that we call `self.peek().clone()` in many match statements. This is because `match self.peek()` borrows `self` immutably, but the match arms need to call `self.advance()` which borrows `self` mutably. Cloning the token releases the immutable borrow before the mutable borrow begins. This is a common Rust pattern when working with structs that need to both read and mutate in the same method.

</details>

---

## Exercise 3: Parse Expressions with Precedence

**Goal:** Replace the minimal `parse_expression` from Exercise 2 with a full precedence-climbing parser that correctly handles `WHERE age > 18 AND name = 'Alice'`.

### The precedence problem

Consider `age > 18 AND name = 'Alice' OR active = TRUE`. How should this be grouped?

- `(age > 18 AND name = 'Alice') OR active = TRUE` — AND binds tighter than OR
- `age > 18 AND (name = 'Alice' OR active = TRUE)` — OR binds tighter than AND

SQL follows mathematical convention: AND binds tighter than OR, just as multiplication binds tighter than addition. The correct grouping is the first one. Our parser must enforce this.

### Precedence levels

We assign a numeric precedence to each operator. Higher numbers bind tighter:

```
Level 1: OR              (loosest)
Level 2: AND
Level 3: = != < > <= >=  (comparison)
Level 4: + -             (addition)
Level 5: * /             (multiplication)
Level 6: NOT, unary -    (tightest)
```

### Step 1: Define the precedence function

```rust
impl Operator {
    /// Return the precedence level of this operator.
    /// Higher means tighter binding.
    fn precedence(&self) -> u8 {
        match self {
            Operator::Or => 1,
            Operator::And => 2,
            Operator::Eq | Operator::NotEq
            | Operator::Lt | Operator::Gt
            | Operator::LtEq | Operator::GtEq => 3,
            Operator::Add | Operator::Sub => 4,
            Operator::Mul | Operator::Div => 5,
            Operator::Not => 6,
        }
    }
}
```

### Step 2: Map tokens to operators

The parser needs to know which tokens are binary operators and what `Operator` they correspond to:

```rust
impl Parser {
    /// Try to convert the current token to a binary operator.
    fn peek_binary_op(&self) -> Option<Operator> {
        match self.peek() {
            Token::Plus => Some(Operator::Add),
            Token::Minus => Some(Operator::Sub),
            Token::Star => Some(Operator::Mul),
            Token::Slash => Some(Operator::Div),
            Token::Equals => Some(Operator::Eq),
            Token::NotEquals => Some(Operator::NotEq),
            Token::LessThan => Some(Operator::Lt),
            Token::GreaterThan => Some(Operator::Gt),
            Token::LessOrEqual => Some(Operator::LtEq),
            Token::GreaterOrEqual => Some(Operator::GtEq),
            Token::Keyword(Keyword::And) => Some(Operator::And),
            Token::Keyword(Keyword::Or) => Some(Operator::Or),
            _ => None,
        }
    }
}
```

### Step 3: Implement precedence climbing

Replace the placeholder `parse_expression` with the real implementation:

```rust
impl Parser {
    /// Parse an expression using precedence climbing (Pratt parsing).
    ///
    /// The algorithm:
    /// 1. Parse an atom (column, literal, unary op, parenthesized expr)
    /// 2. While the next token is a binary operator with precedence >= min_precedence:
    ///    a. Consume the operator
    ///    b. Parse the right-hand side with min_precedence = op_precedence + 1
    ///    c. Combine left, op, right into a BinaryOp node
    /// 3. Return the accumulated expression
    fn parse_expression(&mut self, min_precedence: u8) -> Result<Expression, String> {
        // Step 1: Parse the left-hand side (an atom)
        let mut left = self.parse_unary()?;

        // Step 2: Consume binary operators at or above min_precedence
        while let Some(op) = self.peek_binary_op() {
            let prec = op.precedence();
            if prec < min_precedence {
                break;
            }

            // Consume the operator token
            self.advance();

            // Step 2b: Parse the right-hand side at higher precedence
            // Using prec + 1 makes operators left-associative:
            // a + b + c  =>  (a + b) + c
            let right = self.parse_expression(prec + 1)?;

            // Step 2c: Combine into a BinaryOp
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse a unary expression: NOT expr, -expr, or an atom.
    fn parse_unary(&mut self) -> Result<Expression, String> {
        // NOT prefix
        if self.match_token(&Token::Keyword(Keyword::Not)) {
            let expr = self.parse_expression(Operator::Not.precedence())?;
            return Ok(Expression::UnaryOp {
                op: Operator::Not,
                expr: Box::new(expr),
            });
        }

        // Unary minus: -expr
        if self.match_token(&Token::Minus) {
            let expr = self.parse_expression(Operator::Not.precedence())?;
            return Ok(Expression::UnaryOp {
                op: Operator::Sub,
                expr: Box::new(expr),
            });
        }

        // Parenthesized expression: (expr)
        if self.match_token(&Token::LeftParen) {
            let expr = self.parse_expression(0)?;
            self.expect(&Token::RightParen)?;
            return Ok(expr);
        }

        self.parse_atom()
    }
}
```

This is **Pratt parsing** (also called precedence climbing). The key insight is the `min_precedence` parameter. When we parse the right side of `+` (precedence 4), we use `min_precedence = 5`. This means the right-side parse will only consume operators with precedence 5 or higher (multiplication and division). Addition and subtraction at precedence 4 will *not* be consumed — they will be left for the outer call to handle. This naturally produces left-associative, precedence-correct trees.

Let us trace through `1 + 2 * 3`:

1. `parse_expression(0)` — parse atom `1`, see `+` (prec 4 >= 0), consume it
2. Parse right side: `parse_expression(5)` — parse atom `2`, see `*` (prec 5 >= 5), consume it
3. Parse right side: `parse_expression(6)` — parse atom `3`, see EOF (no operator), return `3`
4. Combine: `2 * 3`
5. Back in step 2: no more operators with prec >= 5, return `2 * 3`
6. Back in step 1: combine `1 + (2 * 3)`

Result: `Add { left: 1, right: Mul { left: 2, right: 3 } }` — multiplication binds tighter, as expected.

### Step 4: Test expression parsing

```rust
#[cfg(test)]
mod expression_tests {
    use super::*;

    /// Helper: parse an expression from a SQL WHERE clause.
    fn parse_expr(sql: &str) -> Expression {
        // Wrap in a SELECT so the parser has a complete statement
        let full = format!("SELECT x FROM t WHERE {}", sql);
        match Parser::parse(&full).unwrap() {
            Statement::Select { where_clause: Some(expr), .. } => expr,
            _ => panic!("Expected SELECT with WHERE"),
        }
    }

    #[test]
    fn simple_comparison() {
        let expr = parse_expr("age > 18");
        assert_eq!(expr, Expression::BinaryOp {
            left: Box::new(Expression::Column("age".to_string())),
            op: Operator::Gt,
            right: Box::new(Expression::Literal(Value::Integer(18))),
        });
    }

    #[test]
    fn and_expression() {
        let expr = parse_expr("age > 18 AND name = 'Alice'");
        // Top-level should be AND
        if let Expression::BinaryOp { op, left, right } = &expr {
            assert_eq!(op, &Operator::And);

            // Left: age > 18
            if let Expression::BinaryOp { op, .. } = left.as_ref() {
                assert_eq!(op, &Operator::Gt);
            } else {
                panic!("Expected BinaryOp for left side of AND");
            }

            // Right: name = 'Alice'
            if let Expression::BinaryOp { op, .. } = right.as_ref() {
                assert_eq!(op, &Operator::Eq);
            } else {
                panic!("Expected BinaryOp for right side of AND");
            }
        } else {
            panic!("Expected AND at top level");
        }
    }

    #[test]
    fn precedence_and_or() {
        // AND binds tighter than OR
        // a = 1 OR b = 2 AND c = 3  =>  a = 1 OR (b = 2 AND c = 3)
        let expr = parse_expr("a = 1 OR b = 2 AND c = 3");

        if let Expression::BinaryOp { op, left, right } = &expr {
            // Top-level is OR (lower precedence)
            assert_eq!(op, &Operator::Or);

            // Left: a = 1
            if let Expression::BinaryOp { op, .. } = left.as_ref() {
                assert_eq!(op, &Operator::Eq);
            }

            // Right: b = 2 AND c = 3
            if let Expression::BinaryOp { op, .. } = right.as_ref() {
                assert_eq!(op, &Operator::And);
            }
        } else {
            panic!("Expected OR at top level");
        }
    }

    #[test]
    fn precedence_arithmetic() {
        // 1 + 2 * 3  =>  1 + (2 * 3)
        let expr = parse_expr("1 + 2 * 3");

        if let Expression::BinaryOp { op, right, .. } = &expr {
            assert_eq!(op, &Operator::Add);
            if let Expression::BinaryOp { op, .. } = right.as_ref() {
                assert_eq!(op, &Operator::Mul);
            } else {
                panic!("Expected Mul on right side of Add");
            }
        } else {
            panic!("Expected Add at top level");
        }
    }

    #[test]
    fn left_associativity() {
        // 1 - 2 - 3  =>  (1 - 2) - 3
        let expr = parse_expr("1 - 2 - 3");

        if let Expression::BinaryOp { op, left, right } = &expr {
            assert_eq!(op, &Operator::Sub);
            // Right should be a literal 3 (not a subtraction)
            assert_eq!(right.as_ref(), &Expression::Literal(Value::Integer(3)));
            // Left should be 1 - 2
            if let Expression::BinaryOp { op, .. } = left.as_ref() {
                assert_eq!(op, &Operator::Sub);
            }
        }
    }

    #[test]
    fn parenthesized_expression() {
        // (1 + 2) * 3  =>  Mul { left: Add { 1, 2 }, right: 3 }
        let expr = parse_expr("(1 + 2) * 3");

        if let Expression::BinaryOp { op, left, .. } = &expr {
            assert_eq!(op, &Operator::Mul);
            if let Expression::BinaryOp { op, .. } = left.as_ref() {
                assert_eq!(op, &Operator::Add);
            }
        }
    }

    #[test]
    fn not_expression() {
        let expr = parse_expr("NOT active");
        assert_eq!(expr, Expression::UnaryOp {
            op: Operator::Not,
            expr: Box::new(Expression::Column("active".to_string())),
        });
    }

    #[test]
    fn unary_minus() {
        let expr = parse_expr("-42");
        assert_eq!(expr, Expression::UnaryOp {
            op: Operator::Sub,
            expr: Box::new(Expression::Literal(Value::Integer(42))),
        });
    }

    #[test]
    fn boolean_literal() {
        let expr = parse_expr("active = TRUE");
        if let Expression::BinaryOp { right, .. } = &expr {
            assert_eq!(right.as_ref(), &Expression::Literal(Value::Boolean(true)));
        }
    }

    #[test]
    fn null_literal() {
        let expr = parse_expr("name = NULL");
        if let Expression::BinaryOp { right, .. } = &expr {
            assert_eq!(right.as_ref(), &Expression::Literal(Value::Null));
        }
    }
}
```

```
$ cargo test expression_tests
running 10 tests
test parser::expression_tests::simple_comparison ... ok
test parser::expression_tests::and_expression ... ok
test parser::expression_tests::precedence_and_or ... ok
test parser::expression_tests::precedence_arithmetic ... ok
test parser::expression_tests::left_associativity ... ok
test parser::expression_tests::parenthesized_expression ... ok
test parser::expression_tests::not_expression ... ok
test parser::expression_tests::unary_minus ... ok
test parser::expression_tests::boolean_literal ... ok
test parser::expression_tests::null_literal ... ok

test result: ok. 10 passed; 0 failed; 0 ignored
```

<details>
<summary>Hint: Why prec + 1 for left-associativity?</summary>

When parsing the right side of a binary operator, we call `parse_expression(prec + 1)`. This means the right-side parse will only consume operators with *strictly higher* precedence. An operator with the *same* precedence will not be consumed — it will be left for the outer loop to handle, which puts it on top of the tree (i.e., the earlier occurrence is deeper in the tree, meaning it evaluates first).

If we used `prec` instead of `prec + 1`, operators at the same precedence would be consumed into the right side, making them *right-associative*: `1 - 2 - 3` would parse as `1 - (2 - 3) = 2` instead of `(1 - 2) - 3 = -4`.

For exponentiation (which is right-associative in mathematics), you would use `prec` instead of `prec + 1`. Our SQL subset does not have exponentiation, so left-associativity is correct everywhere.

</details>

---

## Exercise 4: Parse All Statement Types

**Goal:** Complete the parser with INSERT, UPDATE, DELETE, and CREATE TABLE statements. Then test with complex, realistic queries.

### Step 1: Parse INSERT

```rust
impl Parser {
    /// Parse: INSERT INTO table (columns) VALUES (values)
    fn parse_insert(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Insert)?;
        self.expect_keyword(Keyword::Into)?;

        let table = self.expect_ident()?;

        // Parse column list: (col1, col2, col3)
        self.expect(&Token::LeftParen)?;
        let columns = self.parse_ident_list()?;
        self.expect(&Token::RightParen)?;

        // VALUES keyword
        self.expect_keyword(Keyword::Values)?;

        // Parse value list: (val1, val2, val3)
        self.expect(&Token::LeftParen)?;
        let values = self.parse_expression_list()?;
        self.expect(&Token::RightParen)?;

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    /// Parse a comma-separated list of identifiers.
    fn parse_ident_list(&mut self) -> Result<Vec<String>, String> {
        let mut idents = Vec::new();
        idents.push(self.expect_ident()?);

        while self.match_token(&Token::Comma) {
            idents.push(self.expect_ident()?);
        }

        Ok(idents)
    }

    /// Parse a comma-separated list of expressions.
    fn parse_expression_list(&mut self) -> Result<Vec<Expression>, String> {
        let mut exprs = Vec::new();
        exprs.push(self.parse_expression(0)?);

        while self.match_token(&Token::Comma) {
            exprs.push(self.parse_expression(0)?);
        }

        Ok(exprs)
    }
}
```

The `parse_ident_list` and `parse_expression_list` methods follow the same pattern: parse one item, then loop while there are commas. This "first, then comma-separated rest" pattern appears in almost every parser for any language. It naturally handles both single-element lists (`(name)`) and multi-element lists (`(name, age, email)`).

### Step 2: Parse UPDATE

```rust
impl Parser {
    /// Parse: UPDATE table SET col = val, col = val [WHERE condition]
    fn parse_update(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Update)?;
        let table = self.expect_ident()?;

        self.expect_keyword(Keyword::Set)?;

        // Parse assignments: col = val, col = val
        let mut assignments = Vec::new();
        loop {
            let column = self.expect_ident()?;
            self.expect(&Token::Equals)?;
            let value = self.parse_expression(0)?;
            assignments.push((column, value));

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        // Optional WHERE clause
        let where_clause = if self.match_token(&Token::Keyword(Keyword::Where)) {
            Some(self.parse_expression(0)?)
        } else {
            None
        };

        Ok(Statement::Update {
            table,
            assignments,
            where_clause,
        })
    }
}
```

The assignment list uses `Vec<(String, Expression)>` — a vector of tuples. Each tuple pairs a column name with its new value. This is a natural representation: `SET name = 'Bob', age = 30` becomes `[("name", Literal("Bob")), ("age", Literal(30))]`.

### Step 3: Parse DELETE

```rust
impl Parser {
    /// Parse: DELETE FROM table [WHERE condition]
    fn parse_delete(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Delete)?;
        self.expect_keyword(Keyword::From)?;

        let table = self.expect_ident()?;

        // Optional WHERE clause
        let where_clause = if self.match_token(&Token::Keyword(Keyword::Where)) {
            Some(self.parse_expression(0)?)
        } else {
            None
        };

        Ok(Statement::Delete {
            table,
            where_clause,
        })
    }
}
```

DELETE is the simplest statement — just a table name and an optional WHERE clause. Notice how the WHERE clause parsing is identical across SELECT, UPDATE, and DELETE. We could extract it into a helper method, but the two-line pattern is simple enough that the duplication is acceptable. Ousterhout calls this the "somewhat deep" tradeoff — an abstraction is only worth it when the repeated code is complex enough that the abstraction reduces cognitive load.

### Step 4: Parse CREATE TABLE

```rust
impl Parser {
    /// Parse: CREATE TABLE name (col1 TYPE, col2 TYPE, ...)
    fn parse_create_table(&mut self) -> Result<Statement, String> {
        self.expect_keyword(Keyword::Create)?;
        self.expect_keyword(Keyword::Table)?;

        let name = self.expect_ident()?;

        // Parse column definitions
        self.expect(&Token::LeftParen)?;
        let mut columns = Vec::new();

        loop {
            let col_name = self.expect_ident()?;
            let data_type = self.parse_data_type()?;
            columns.push(ColumnDef {
                name: col_name,
                data_type,
            });

            if !self.match_token(&Token::Comma) {
                break;
            }
        }

        self.expect(&Token::RightParen)?;

        Ok(Statement::CreateTable { name, columns })
    }

    /// Parse a data type: INTEGER, TEXT, BOOLEAN.
    fn parse_data_type(&mut self) -> Result<DataType, String> {
        let token = self.peek().clone();
        match token {
            Token::Ident(ref s) => {
                let dt = match s.to_uppercase().as_str() {
                    "INTEGER" | "INT" => DataType::Integer,
                    "TEXT" | "STRING" | "VARCHAR" => DataType::Text,
                    "BOOLEAN" | "BOOL" => DataType::Boolean,
                    _ => return Err(format!(
                        "Unknown data type '{}' at position {}",
                        s, self.position
                    )),
                };
                self.advance();
                Ok(dt)
            }
            _ => Err(format!(
                "Expected data type, found {} at position {}",
                token, self.position
            )),
        }
    }
}
```

Data types are identifiers, not keywords — `INTEGER` is not in our `Keyword` enum. This is a deliberate design choice. Adding every SQL data type to the keyword enum would pollute it and make keyword lookup slower. Instead, we parse data types as identifiers and match them case-insensitively. We also accept common aliases: `INT` for `INTEGER`, `VARCHAR` for `TEXT`, `BOOL` for `BOOLEAN`.

### Step 5: Test all statement types

```rust
#[cfg(test)]
mod full_parser_tests {
    use super::*;

    #[test]
    fn parse_insert() {
        let ast = Parser::parse(
            "INSERT INTO users (name, age) VALUES ('Alice', 30)"
        ).unwrap();

        assert_eq!(ast, Statement::Insert {
            table: "users".to_string(),
            columns: vec!["name".to_string(), "age".to_string()],
            values: vec![
                Expression::Literal(Value::String("Alice".to_string())),
                Expression::Literal(Value::Integer(30)),
            ],
        });
    }

    #[test]
    fn parse_insert_single_value() {
        let ast = Parser::parse(
            "INSERT INTO flags (name) VALUES ('active')"
        ).unwrap();

        assert_eq!(ast, Statement::Insert {
            table: "flags".to_string(),
            columns: vec!["name".to_string()],
            values: vec![
                Expression::Literal(Value::String("active".to_string())),
            ],
        });
    }

    #[test]
    fn parse_update_with_where() {
        let ast = Parser::parse(
            "UPDATE users SET name = 'Bob' WHERE id = 1"
        ).unwrap();

        assert_eq!(ast, Statement::Update {
            table: "users".to_string(),
            assignments: vec![
                ("name".to_string(), Expression::Literal(Value::String("Bob".to_string()))),
            ],
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("id".to_string())),
                op: Operator::Eq,
                right: Box::new(Expression::Literal(Value::Integer(1))),
            }),
        });
    }

    #[test]
    fn parse_update_multiple_assignments() {
        let ast = Parser::parse(
            "UPDATE users SET name = 'Bob', age = 25 WHERE id = 1"
        ).unwrap();

        if let Statement::Update { assignments, .. } = &ast {
            assert_eq!(assignments.len(), 2);
            assert_eq!(assignments[0].0, "name");
            assert_eq!(assignments[1].0, "age");
        } else {
            panic!("Expected Update");
        }
    }

    #[test]
    fn parse_update_without_where() {
        let ast = Parser::parse(
            "UPDATE users SET active = FALSE"
        ).unwrap();

        if let Statement::Update { where_clause, .. } = &ast {
            assert!(where_clause.is_none());
        } else {
            panic!("Expected Update");
        }
    }

    #[test]
    fn parse_delete_with_where() {
        let ast = Parser::parse("DELETE FROM users WHERE id = 42").unwrap();

        assert_eq!(ast, Statement::Delete {
            table: "users".to_string(),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column("id".to_string())),
                op: Operator::Eq,
                right: Box::new(Expression::Literal(Value::Integer(42))),
            }),
        });
    }

    #[test]
    fn parse_delete_without_where() {
        let ast = Parser::parse("DELETE FROM users").unwrap();

        assert_eq!(ast, Statement::Delete {
            table: "users".to_string(),
            where_clause: None,
        });
    }

    #[test]
    fn parse_create_table() {
        let ast = Parser::parse(
            "CREATE TABLE users (id INTEGER, name TEXT, active BOOLEAN)"
        ).unwrap();

        assert_eq!(ast, Statement::CreateTable {
            name: "users".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), data_type: DataType::Integer },
                ColumnDef { name: "name".to_string(), data_type: DataType::Text },
                ColumnDef { name: "active".to_string(), data_type: DataType::Boolean },
            ],
        });
    }

    #[test]
    fn parse_create_table_type_aliases() {
        let ast = Parser::parse(
            "CREATE TABLE t (a INT, b VARCHAR, c BOOL)"
        ).unwrap();

        if let Statement::CreateTable { columns, .. } = &ast {
            assert_eq!(columns[0].data_type, DataType::Integer);
            assert_eq!(columns[1].data_type, DataType::Text);
            assert_eq!(columns[2].data_type, DataType::Boolean);
        }
    }

    #[test]
    fn parse_complex_where() {
        let ast = Parser::parse(
            "SELECT name, age FROM users WHERE age >= 18 AND age < 65 AND active = TRUE"
        ).unwrap();

        if let Statement::Select { columns, from, where_clause } = &ast {
            assert_eq!(columns.len(), 2);
            assert_eq!(from, "users");
            assert!(where_clause.is_some());

            // The WHERE clause should be a nested AND tree
            let expr = where_clause.as_ref().unwrap();
            if let Expression::BinaryOp { op, .. } = expr {
                assert_eq!(op, &Operator::And);
            } else {
                panic!("Expected AND at top level of WHERE");
            }
        }
    }

    #[test]
    fn error_insert_missing_values() {
        let err = Parser::parse("INSERT INTO users (name)").unwrap_err();
        assert!(err.contains("Expected VALUES"), "Error was: {}", err);
    }

    #[test]
    fn error_create_table_unknown_type() {
        let err = Parser::parse(
            "CREATE TABLE t (x BIGINT)"
        ).unwrap_err();
        assert!(err.contains("Unknown data type"), "Error was: {}", err);
    }

    #[test]
    fn error_missing_right_paren() {
        let err = Parser::parse(
            "INSERT INTO users (name, age VALUES ('Alice', 30)"
        ).unwrap_err();
        assert!(err.contains("Expected )"), "Error was: {}", err);
    }
}
```

```
$ cargo test full_parser_tests
running 13 tests
test parser::full_parser_tests::parse_insert ... ok
test parser::full_parser_tests::parse_insert_single_value ... ok
test parser::full_parser_tests::parse_update_with_where ... ok
test parser::full_parser_tests::parse_update_multiple_assignments ... ok
test parser::full_parser_tests::parse_update_without_where ... ok
test parser::full_parser_tests::parse_delete_with_where ... ok
test parser::full_parser_tests::parse_delete_without_where ... ok
test parser::full_parser_tests::parse_create_table ... ok
test parser::full_parser_tests::parse_create_table_type_aliases ... ok
test parser::full_parser_tests::parse_complex_where ... ok
test parser::full_parser_tests::error_insert_missing_values ... ok
test parser::full_parser_tests::error_create_table_unknown_type ... ok
test parser::full_parser_tests::error_missing_right_paren ... ok

test result: ok. 13 passed; 0 failed; 0 ignored
```

### Step 6: Pretty-print the AST

Add a display function so you can see the parsed structure:

```rust
impl Expression {
    /// Pretty-print the expression tree with indentation.
    pub fn pretty_print(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            Expression::Column(name) => format!("{}Column({})", pad, name),
            Expression::Literal(val) => format!("{}Literal({})", pad, val),
            Expression::BinaryOp { left, op, right } => {
                format!(
                    "{}BinaryOp({})\n{}\n{}",
                    pad, op,
                    left.pretty_print(indent + 1),
                    right.pretty_print(indent + 1),
                )
            }
            Expression::UnaryOp { op, expr } => {
                format!(
                    "{}UnaryOp({})\n{}",
                    pad, op,
                    expr.pretty_print(indent + 1),
                )
            }
        }
    }
}
```

Test with a complex query to see the tree structure:

```rust
    #[test]
    fn pretty_print_ast() {
        let ast = Parser::parse(
            "SELECT name FROM users WHERE age > 18 AND name != 'admin'"
        ).unwrap();

        if let Statement::Select { where_clause: Some(expr), .. } = &ast {
            let output = expr.pretty_print(0);
            println!("{}", output);
            assert!(output.contains("BinaryOp(AND)"));
            assert!(output.contains("BinaryOp(>)"));
            assert!(output.contains("BinaryOp(!=)"));
        }
    }
```

Expected output:

```
BinaryOp(AND)
  BinaryOp(>)
    Column(age)
    Literal(18)
  BinaryOp(!=)
    Column(name)
    Literal('admin')
```

The pretty-printed AST makes the tree structure visible. `AND` is the root. Its left child is `>` (comparing `age` to `18`), and its right child is `!=` (comparing `name` to `'admin'`). This is exactly the structure the query executor (Chapter 10) will traverse to evaluate each row.

<details>
<summary>Hint: If UPDATE SET parsing fails on the = sign</summary>

The `=` token is `Token::Equals`, which is also a comparison operator. In the UPDATE SET context, `=` means assignment, not comparison. The parser knows the difference because of *context* — after `SET column`, the `=` is always assignment. After a column in a WHERE clause, `=` is always comparison. Recursive descent parsers handle this naturally because each parse function knows what grammar rule it is implementing.

If your parser tries to parse `name = 'Bob'` as an expression (calling `parse_expression`), it will consume all three tokens and produce a `BinaryOp(Eq)`. The `parse_update` method avoids this by parsing the column name with `expect_ident()`, consuming the `=` with `expect(&Token::Equals)`, and then parsing only the right-hand side with `parse_expression(0)`. This is deliberate decomposition of the grammar.

</details>

---

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

## DSA in Context: Abstract Syntax Trees

An **Abstract Syntax Tree** (AST) is a tree data structure where:
- **Leaf nodes** are values: column references, literals, constants
- **Internal nodes** are operations: binary operators, unary operators, function calls
- **The root** is the top-level operation

The "abstract" in AST means it discards syntactic details that do not affect meaning. Parentheses, whitespace, and operator tokens are not in the tree — only the structure they imply. `(1 + 2) * 3` and `((1 + 2)) * 3` produce identical ASTs because the extra parentheses change nothing.

### Tree traversal

Processing an AST requires tree traversal — visiting every node. The two fundamental traversals are:

**Pre-order (visit parent before children):**
```
visit(node):
    process(node)
    for child in node.children:
        visit(child)
```

Used for: printing the tree, copying the tree, serializing.

**Post-order (visit children before parent):**
```
visit(node):
    for child in node.children:
        visit(child)
    process(node)
```

Used for: evaluating expressions (you need child values before you can compute the parent), freeing memory (free children before the parent).

Our `eval` function from Drill 1 is a post-order traversal — it evaluates children first (`eval(left)`, `eval(right)`), then combines the results. The `pretty_print` function is a pre-order traversal — it prints the current node's operator first, then recursively prints children.

### Precedence climbing as tree construction

The Pratt parsing algorithm from Exercise 3 builds the AST bottom-up. Higher-precedence operators become deeper nodes (closer to the leaves), and lower-precedence operators become shallower nodes (closer to the root). This is correct because tree evaluation is post-order: deeper nodes evaluate first, and deeper means "higher precedence."

For `1 + 2 * 3`:

```
    Add         <- root (evaluated last)
   /   \
  1    Mul      <- deeper (evaluated first)
      /   \
     2     3
```

The `*` is deeper than `+`, so it evaluates first: `2 * 3 = 6`, then `1 + 6 = 7`. Precedence climbing ensures this tree shape by only allowing higher-precedence operators to be consumed into the right-hand side of lower-precedence operators.

### Time and space complexity

Parsing is O(N) where N is the number of tokens. Each token is consumed exactly once — the parser never backtracks. The resulting AST has O(N) nodes (one leaf per value token, one internal node per operator token). Evaluating the AST is also O(N) — one visit per node.

The space complexity of the AST is O(N) for the nodes plus O(D) stack space for recursive traversal, where D is the depth of the tree. For typical SQL queries, D is small (under 20 levels of nesting). For pathological queries like `a AND b AND c AND ... AND z`, the left-associative tree has depth N, but this is still manageable.

---

## System Design Corner: SQL Parsing in Production

In a system design interview, the parser is one stage in the query processing pipeline. But production SQL parsing has several considerations that our simple parser does not address.

### Prepared statements

When an application runs the same query thousands of times with different parameters:

```sql
SELECT name FROM users WHERE id = 42
SELECT name FROM users WHERE id = 73
SELECT name FROM users WHERE id = 101
```

Parsing each one is wasteful — the structure is identical, only the value changes. **Prepared statements** solve this:

```sql
PREPARE get_user AS SELECT name FROM users WHERE id = $1
EXECUTE get_user(42)
EXECUTE get_user(73)
```

The `PREPARE` step parses once and stores the AST. Each `EXECUTE` substitutes the parameter into the stored AST and skips parsing entirely. This is a major performance win for OLTP workloads where the same queries run millions of times per day.

### Query plan caching

Going further, production databases cache not just the AST but the entire optimized execution plan. PostgreSQL's plan cache stores plans keyed by query string. If the same query arrives again, it skips parsing, planning, and optimization — it jumps straight to execution.

The tradeoff: cached plans may become stale when table statistics change. A plan that was optimal when the table had 100 rows may be terrible when it has 10 million rows. PostgreSQL addresses this with "generic" vs "custom" plans — after a few executions, it compares the generic plan cost against re-planning with current statistics.

### SQL injection and parameterized queries

Parsing is also where SQL injection attacks are prevented. Consider:

```python
# DANGEROUS: string interpolation
query = f"SELECT * FROM users WHERE name = '{user_input}'"
```

If `user_input` is `'; DROP TABLE users; --`, the parser sees:

```sql
SELECT * FROM users WHERE name = ''; DROP TABLE users; --'
```

Three statements. The second one drops the table. **Parameterized queries** prevent this by separating the SQL structure from the data:

```python
# SAFE: parameterized query
query = "SELECT * FROM users WHERE name = $1"
params = [user_input]
```

The parser only sees the template. The parameter value is never parsed as SQL — it is treated as a literal value regardless of its content. This is defense at the architecture level, not at the input validation level.

### Parse error recovery

Production SQL parsers need to handle malformed input gracefully. Our parser returns the first error and stops. PostgreSQL's parser attempts error recovery — it tries to identify where the error is, report a helpful message with a caret pointing to the exact position, and suggest fixes:

```
ERROR:  syntax error at or near "FORM"
LINE 1: SELECT name FORM users
                     ^
HINT:  Did you mean "FROM"?
```

> **Interview talking point:** *"SQL parsing in production goes beyond syntax analysis. Prepared statements amortize parse cost across executions — parse once, execute many. Plan caching extends this to the optimizer, skipping re-planning for repeated queries. Parameterized queries prevent SQL injection at the parsing layer by separating structure from data. And parse error recovery provides actionable diagnostics rather than opaque failures."*

---

## Design Insight: Obvious Code

In *A Philosophy of Software Design*, Ousterhout argues that code should be **obvious** — a reader should understand what the code does without significant effort. The best code reads like prose.

Our parser methods demonstrate this principle. Look at `parse_select`:

```rust
fn parse_select(&mut self) -> Result<Statement, String> {
    self.expect_keyword(Keyword::Select)?;
    let columns = self.parse_select_columns()?;
    self.expect_keyword(Keyword::From)?;
    let from = self.expect_ident()?;
    let where_clause = if self.match_token(&Token::Keyword(Keyword::Where)) {
        Some(self.parse_expression(0)?)
    } else {
        None
    };
    Ok(Statement::Select { columns, from, where_clause })
}
```

Read it out loud: "Expect SELECT. Parse the columns. Expect FROM. Get the table name. If there is a WHERE, parse the expression; otherwise, None." The code *reads like the grammar it implements*. Someone who has never seen this code can tell exactly what it does, because the method names map directly to the concepts.

Contrast this with a non-obvious parser:

```rust
// Non-obvious: what does this do?
fn parse(&mut self) -> Result<Statement, String> {
    let t = self.next();
    let mut cols = vec![];
    loop {
        let c = self.next();
        if c.is_kw("FROM") { break; }
        if !c.is_comma() { cols.push(c); }
    }
    let tbl = self.next();
    // ...50 more lines of index manipulation
}
```

Both parse SELECT statements. The first is obvious. The second requires careful reading to understand. The difference is not cleverness — it is the investment in naming, helper methods, and decomposition.

Ousterhout's principle applies directly to recursive descent parsers. Each grammar rule becomes a method. Each method's name describes the grammar rule. The code's structure mirrors the grammar's structure. When you add a new SQL feature (like JOIN or GROUP BY), you add a new method with a descriptive name. The parser grows in a way that remains readable.

This is why recursive descent is the most popular parsing technique despite being less powerful than table-driven parsers (like LALR or PEG). It is obvious. You can read it, debug it, and extend it without understanding automata theory.

---

## What You Built

In this chapter, you:

1. **Defined the AST types** — `Expression`, `Statement`, `Value`, `Operator`, `ColumnDef`, and `DataType` enums/structs that model SQL as a typed data structure, using `Box<Expression>` for recursive tree nodes
2. **Built the parser foundation** — a `Parser` struct with `peek()`, `advance()`, `expect()`, `match_token()`, and `expect_ident()` navigation methods that mirror the lexer's structure
3. **Implemented precedence climbing** — a Pratt parser that correctly handles operator precedence (`*` before `+`, `AND` before `OR`) and left-associativity, producing correctly shaped ASTs for arbitrarily nested expressions
4. **Parsed all five statement types** — SELECT (with column lists and WHERE), INSERT (with column and value lists), UPDATE (with assignments and WHERE), DELETE (with WHERE), and CREATE TABLE (with typed column definitions)

Your database can now understand SQL. Given `SELECT name FROM users WHERE age > 18 AND active = TRUE`, it produces a `Statement::Select` with a properly nested `Expression` tree for the WHERE clause. The flat token stream from Chapter 6 has become structured meaning.

Chapter 8 builds the query planner — the component that looks at a parsed AST and decides *how* to execute it. Should it scan every row, or use an index? Should it filter first and then project, or project first and then filter? The AST tells the planner *what* the user wants. The planner decides *how* to get it.

---

### DS Deep Dive

Our parser is a simple recursive descent parser that handles SQL's relatively flat grammar. But parsing theory goes much deeper: context-free grammars, LL and LR parsing, ambiguity resolution, and the Chomsky hierarchy that classifies languages by the computational power needed to parse them. This deep dive explores how parser generators like yacc and ANTLR work, why SQL is not quite context-free, and how Pratt parsing relates to operator-precedence grammars.

**-> [Parsing Theory & Grammars -- "From Tokens to Trees"](../ds-narratives/ch07-parsing-theory.md)**

---

### Reference implementation

The files you built in this chapter correspond to these files in the reference codebase:

| Your file | Reference |
|-----------|-----------|
| `src/parser.rs` — `Expression` enum | [`src/sql/parser/ast.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/ast.rs) — `Expression` enum |
| `src/parser.rs` — `Statement` enum | [`src/sql/parser/ast.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/ast.rs) — `Statement` enum |
| `src/parser.rs` — `Parser::parse()` | [`src/sql/parser/mod.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/mod.rs) — `Parser::parse()` |
| `src/parser.rs` — `parse_expression()` (Pratt) | [`src/sql/parser/mod.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/mod.rs) — `parse_expression()` |
| `src/parser.rs` — `parse_select()`, `parse_insert()`, etc. | [`src/sql/parser/mod.rs`](https://github.com/erikgrinaker/toydb/blob/master/src/sql/parser/mod.rs) — individual statement parsers |
