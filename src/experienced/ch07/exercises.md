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
