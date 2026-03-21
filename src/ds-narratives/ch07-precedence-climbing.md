# Precedence Climbing — "1 + 2 * 3 is 7, not 9"

Every calculator app you have ever built has a bug. Or rather, every calculator app you built *before* learning precedence climbing has a bug. Type `1 + 2 * 3` and it says 9 instead of 7. Type `3 - 1 - 1` and it says 3 instead of 1. Type `-5 * 2` and it crashes.

The bug is not in the arithmetic. It is in the parsing. Left-to-right evaluation treats all operators equally, but they are not equal. Multiplication binds tighter than addition. Subtraction is left-associative, meaning `3 - 1 - 1` is `(3 - 1) - 1`, not `3 - (1 - 1)`. Unary minus binds tighter than anything.

Precedence climbing (also called Pratt parsing) handles all of this with a single recursive function and a small table. It is elegant, fast, and -- once you see the pattern -- surprisingly simple.

---

## The Naive Way

You might try a multi-pass approach: first handle all multiplications, then additions.

```rust
fn main() {
    // Parse "3 + 4 * 2 - 1" by handling * first, then + and -
    let expr = "3 + 4 * 2 - 1";
    let tokens: Vec<&str> = expr.split_whitespace().collect();

    // Pass 1: find and evaluate all multiplications
    // But how? The tokens are strings. You would need to:
    // 1. Scan for * and /
    // 2. Replace "4 * 2" with "8" in the token list
    // 3. Rescan for + and -
    // 4. Handle parentheses by recursive descent into sub-expressions
    // 5. Handle unary minus as a special prefix operator

    // This quickly becomes a mess of index manipulation
    println!("Multi-pass parsing is fragile and error-prone.");
    println!("Precedence climbing does it in one clean pass.");
}
```

The multi-pass approach works for two precedence levels but becomes unmanageable with three or more. Add comparison operators (`>`, `<=`), boolean operators (`AND`, `OR`), and unary operators (`NOT`, `-`), and you have six passes, each with its own edge cases. Precedence climbing handles them all uniformly.

---

## The Insight

Imagine you are reading a math problem aloud, but with a special rule: you must finish reading a "tight" operation before you can continue a "loose" one. When you see `1 + 2 * 3`, you read `1`, then see `+` (loose). You look right and see `2`. Then you see `*` (tight). Because `*` is tighter than `+`, you finish the multiplication first: `2 * 3 = 6`. Only then do you complete the addition: `1 + 6 = 7`.

The algorithm is:

1. Read the first operand (a number or a parenthesized sub-expression)
2. Look at the next operator
3. If the operator binds tighter than what called you, handle it (recurse right)
4. If it binds looser or the same, stop and return what you have

That is it. The recursion naturally groups tight operators before loose ones. The precedence table drives all the decisions. Here is the table for a typical expression language:

```text
Precedence Level   Operators         Associativity
6 (tightest)       unary -, NOT      right (prefix)
5                  *, /              left
4                  +, -              left
3                  =, !=, <, >, <=   left
2                  AND               left
1 (loosest)        OR                left
```

---

## The Build

### Tokens

First, a minimal token type for our expression parser:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus, Minus, Star, Slash,
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    LParen, RParen,
    And, Or, Not,
    Eof,
}
```

### The Tokenizer

A simple tokenizer that splits an expression string into tokens:

```rust
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => { i += 1; }
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '=' => { tokens.push(Token::Eq); i += 1; }
            '!' if i + 1 < chars.len() && chars[i+1] == '=' => {
                tokens.push(Token::NotEq); i += 2;
            }
            '<' => {
                if i + 1 < chars.len() && chars[i+1] == '=' {
                    tokens.push(Token::LtEq); i += 2;
                } else { tokens.push(Token::Lt); i += 1; }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i+1] == '=' {
                    tokens.push(Token::GtEq); i += 2;
                } else { tokens.push(Token::Gt); i += 1; }
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::Number(s.parse().unwrap()));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.to_uppercase().as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "NOT" => tokens.push(Token::Not),
                    _ => panic!("unknown identifier: {}", word),
                }
            }
            c => panic!("unexpected character: {}", c),
        }
    }
    tokens.push(Token::Eof);
    tokens
}
```

### The AST

The parser builds an expression tree:

```rust
#[derive(Debug, Clone)]
enum Expr {
    Number(f64),
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
}

#[derive(Debug, Clone)]
enum BinOp { Add, Sub, Mul, Div, Eq, NotEq, Lt, LtEq, Gt, GtEq, And, Or }

#[derive(Debug, Clone)]
enum UnaryOp { Negate, Not }
```

### The Precedence Table

Each operator has a precedence (binding power) and associativity:

```rust
fn infix_binding_power(token: &Token) -> Option<(u8, u8)> {
    // Returns (left_bp, right_bp)
    // Left-associative: left_bp < right_bp (left binds tighter)
    // Right-associative: left_bp > right_bp
    match token {
        Token::Or =>                    Some((1, 2)),
        Token::And =>                   Some((3, 4)),
        Token::Eq | Token::NotEq |
        Token::Lt | Token::LtEq |
        Token::Gt | Token::GtEq =>     Some((5, 6)),
        Token::Plus | Token::Minus =>   Some((7, 8)),
        Token::Star | Token::Slash =>   Some((9, 10)),
        _ => None,
    }
}

fn prefix_binding_power(token: &Token) -> Option<u8> {
    match token {
        Token::Minus => Some(11), // tighter than any infix op
        Token::Not => Some(11),
        _ => None,
    }
}
```

The asymmetry between left and right binding powers is how we encode associativity. For left-associative `+`, left power is 7 and right power is 8. This means: when we are inside a `+` and see another `+` on the right, the right `+` has left power 7, which is NOT greater than our current right power 8. So we stop. The first `+` claims the operand. This gives us left-to-right grouping: `1 + 2 + 3` becomes `(1 + 2) + 3`.

For right-associative operators, we would flip it: left power 8, right power 7.

### The Parser

The heart of precedence climbing -- one recursive function:

```rust
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) {
        let tok = self.advance();
        assert_eq!(&tok, expected, "expected {:?}, got {:?}", expected, tok);
    }

    /// Parse an expression with minimum binding power `min_bp`
    fn parse_expr(&mut self, min_bp: u8) -> Expr {
        // Step 1: Parse the left-hand side (prefix position)
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Number(n),

            Token::LParen => {
                let expr = self.parse_expr(0); // reset precedence inside parens
                self.expect(&Token::RParen);
                expr
            }

            // Prefix operators
            ref tok if prefix_binding_power(tok).is_some() => {
                let bp = prefix_binding_power(tok).unwrap();
                let operand = self.parse_expr(bp);
                let op = match tok {
                    Token::Minus => UnaryOp::Negate,
                    Token::Not => UnaryOp::Not,
                    _ => unreachable!(),
                };
                Expr::UnaryOp {
                    op,
                    operand: Box::new(operand),
                }
            }

            tok => panic!("unexpected token in prefix position: {:?}", tok),
        };

        // Step 2: Handle infix operators
        loop {
            let tok = self.peek().clone();

            // Check if this token is an infix operator
            let (left_bp, right_bp) = match infix_binding_power(&tok) {
                Some(bp) => bp,
                None => break, // not an operator -- stop
            };

            // If this operator binds looser than our minimum, stop
            if left_bp < min_bp {
                break;
            }

            // Consume the operator
            self.advance();

            // Parse the right-hand side with the right binding power
            let rhs = self.parse_expr(right_bp);

            // Build the binary operation node
            let op = match tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Eq => BinOp::Eq,
                Token::NotEq => BinOp::NotEq,
                Token::Lt => BinOp::Lt,
                Token::LtEq => BinOp::LtEq,
                Token::Gt => BinOp::Gt,
                Token::GtEq => BinOp::GtEq,
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                _ => unreachable!(),
            };

            lhs = Expr::BinOp {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }

        lhs
    }
}
```

Read `parse_expr` carefully. It is the entire parser. The recursion happens in two places: when handling prefix operators (which recurse with the prefix binding power) and when handling the right side of infix operators (which recurse with the right binding power). The `min_bp` parameter is the key -- it controls when the recursion stops and returns, which determines how operators group.

### Evaluation

A simple recursive evaluator for the resulting AST:

```rust
fn eval(expr: &Expr) -> f64 {
    match expr {
        Expr::Number(n) => *n,
        Expr::UnaryOp { op, operand } => {
            let val = eval(operand);
            match op {
                UnaryOp::Negate => -val,
                UnaryOp::Not => if val == 0.0 { 1.0 } else { 0.0 },
            }
        }
        Expr::BinOp { op, left, right } => {
            let l = eval(left);
            let r = eval(right);
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => if r != 0.0 { l / r } else { f64::NAN },
                BinOp::Eq => if l == r { 1.0 } else { 0.0 },
                BinOp::NotEq => if l != r { 1.0 } else { 0.0 },
                BinOp::Lt => if l < r { 1.0 } else { 0.0 },
                BinOp::LtEq => if l <= r { 1.0 } else { 0.0 },
                BinOp::Gt => if l > r { 1.0 } else { 0.0 },
                BinOp::GtEq => if l >= r { 1.0 } else { 0.0 },
                BinOp::And => if l != 0.0 && r != 0.0 { 1.0 } else { 0.0 },
                BinOp::Or => if l != 0.0 || r != 0.0 { 1.0 } else { 0.0 },
            }
        }
    }
}
```

---

## The Payoff

Here is the full, runnable implementation:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64), Plus, Minus, Star, Slash,
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    LParen, RParen, And, Or, Not, Eof,
}

#[derive(Debug, Clone)]
enum Expr {
    Number(f64),
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
}
#[derive(Debug, Clone)] enum BinOp { Add, Sub, Mul, Div, Eq, NotEq, Lt, LtEq, Gt, GtEq, And, Or }
#[derive(Debug, Clone)] enum UnaryOp { Negate, Not }

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' '|'\t' => { i += 1; }
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '=' => { tokens.push(Token::Eq); i += 1; }
            '!' if i+1 < chars.len() && chars[i+1]=='=' => { tokens.push(Token::NotEq); i += 2; }
            '<' if i+1 < chars.len() && chars[i+1]=='=' => { tokens.push(Token::LtEq); i += 2; }
            '<' => { tokens.push(Token::Lt); i += 1; }
            '>' if i+1 < chars.len() && chars[i+1]=='=' => { tokens.push(Token::GtEq); i += 2; }
            '>' => { tokens.push(Token::Gt); i += 1; }
            c if c.is_ascii_digit()||c=='.' => {
                let s = i;
                while i < chars.len() && (chars[i].is_ascii_digit()||chars[i]=='.') { i += 1; }
                let n: String = chars[s..i].iter().collect();
                tokens.push(Token::Number(n.parse().unwrap()));
            }
            c if c.is_ascii_alphabetic() => {
                let s = i;
                while i < chars.len() && chars[i].is_ascii_alphanumeric() { i += 1; }
                let w: String = chars[s..i].iter().collect();
                match w.to_uppercase().as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "NOT" => tokens.push(Token::Not),
                    _ => panic!("unknown: {}", w),
                }
            }
            c => panic!("unexpected: {}", c),
        }
    }
    tokens.push(Token::Eof); tokens
}

fn infix_bp(t: &Token) -> Option<(u8,u8)> {
    match t {
        Token::Or => Some((1,2)), Token::And => Some((3,4)),
        Token::Eq|Token::NotEq|Token::Lt|Token::LtEq|Token::Gt|Token::GtEq => Some((5,6)),
        Token::Plus|Token::Minus => Some((7,8)),
        Token::Star|Token::Slash => Some((9,10)),
        _ => None,
    }
}
fn prefix_bp(t: &Token) -> Option<u8> {
    match t { Token::Minus|Token::Not => Some(11), _ => None }
}

struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
    fn new(t: Vec<Token>) -> Self { Parser { tokens: t, pos: 0 } }
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    fn parse_expr(&mut self, min_bp: u8) -> Expr {
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Number(n),
            Token::LParen => { let e = self.parse_expr(0); self.advance(); e } // skip RParen
            ref t if prefix_bp(t).is_some() => {
                let bp = prefix_bp(t).unwrap();
                let operand = self.parse_expr(bp);
                let op = match t { Token::Minus => UnaryOp::Negate, _ => UnaryOp::Not };
                Expr::UnaryOp { op, operand: Box::new(operand) }
            }
            t => panic!("unexpected: {:?}", t),
        };
        loop {
            let tok = self.peek().clone();
            let (lbp, rbp) = match infix_bp(&tok) { Some(bp) => bp, None => break };
            if lbp < min_bp { break; }
            self.advance();
            let rhs = self.parse_expr(rbp);
            let op = match tok {
                Token::Plus => BinOp::Add, Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul, Token::Slash => BinOp::Div,
                Token::Eq => BinOp::Eq, Token::NotEq => BinOp::NotEq,
                Token::Lt => BinOp::Lt, Token::LtEq => BinOp::LtEq,
                Token::Gt => BinOp::Gt, Token::GtEq => BinOp::GtEq,
                Token::And => BinOp::And, Token::Or => BinOp::Or,
                _ => unreachable!(),
            };
            lhs = Expr::BinOp { op, left: Box::new(lhs), right: Box::new(rhs) };
        }
        lhs
    }
}

fn eval(e: &Expr) -> f64 {
    match e {
        Expr::Number(n) => *n,
        Expr::UnaryOp { op, operand } => {
            let v = eval(operand);
            match op { UnaryOp::Negate => -v, UnaryOp::Not => if v == 0.0 { 1.0 } else { 0.0 } }
        }
        Expr::BinOp { op, left, right } => {
            let (l, r) = (eval(left), eval(right));
            match op {
                BinOp::Add => l+r, BinOp::Sub => l-r, BinOp::Mul => l*r,
                BinOp::Div => if r!=0.0 { l/r } else { f64::NAN },
                BinOp::Eq => if l==r {1.0} else {0.0},
                BinOp::NotEq => if l!=r {1.0} else {0.0},
                BinOp::Lt => if l<r {1.0} else {0.0},
                BinOp::LtEq => if l<=r {1.0} else {0.0},
                BinOp::Gt => if l>r {1.0} else {0.0},
                BinOp::GtEq => if l>=r {1.0} else {0.0},
                BinOp::And => if l!=0.0 && r!=0.0 {1.0} else {0.0},
                BinOp::Or => if l!=0.0 || r!=0.0 {1.0} else {0.0},
            }
        }
    }
}

fn pretty(e: &Expr, d: usize) -> String {
    let p = "  ".repeat(d);
    match e {
        Expr::Number(n) => format!("{}{}", p, n),
        Expr::UnaryOp { op, operand } => format!("{}{:?}\n{}", p, op, pretty(operand, d+1)),
        Expr::BinOp { op, left, right } =>
            format!("{}{:?}\n{}\n{}", p, op, pretty(left, d+1), pretty(right, d+1)),
    }
}

fn main() {
    let tests = vec![
        ("1 + 2 * 3", 7.0),
        ("(1 + 2) * 3", 9.0),
        ("3 - 1 - 1", 1.0),        // left-associative
        ("-5 * 2", -10.0),          // unary minus
        ("2 + 3 > 4", 1.0),        // comparison (true = 1.0)
        ("1 + 2 * 3 - 4 / 2", 5.0),
        ("10 > 5 AND 3 < 7", 1.0), // boolean
        ("-(3 + 4)", -7.0),        // unary on parenthesized
    ];

    for (input, expected) in tests {
        let tokens = tokenize(input);
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_expr(0);
        let result = eval(&ast);

        let status = if (result - expected).abs() < 1e-10 { "OK" } else { "FAIL" };
        println!("[{}] {} = {} (expected {})", status, input, result, expected);
    }

    // Show AST structure for 1 + 2 * 3
    println!("\n=== AST for '1 + 2 * 3' ===");
    let tokens = tokenize("1 + 2 * 3");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_expr(0);
    println!("{}", pretty(&ast, 0));
}
```

Every test passes. `1 + 2 * 3` gives 7. `3 - 1 - 1` gives 1. `-5 * 2` gives -10. All from one recursive function that consults a precedence table.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Tokenize input of length n | O(n) | O(n) | Single pass |
| Parse n tokens | O(n) | O(d) stack | d = nesting depth |
| Evaluate tree of n nodes | O(n) | O(d) stack | One visit per node |
| Adding a new operator | O(1) | O(1) | Add one line to binding power table |
| Total (tokenize + parse + eval) | O(n) | O(n) | Linear in input size |

The killer feature: adding a new operator requires exactly one line in the binding power table and one match arm for the operator token. No restructuring, no new grammar rules, no new functions. This is why Pratt parsing is the go-to technique for expression parsing in database and programming language implementations.

---

## Where This Shows Up in Our Database

In Chapter 7, we use precedence climbing to parse SQL expressions:

```rust,ignore
// WHERE age + 1 > 30 AND name != 'admin' OR role = 'superuser'
//
// Precedence climbing correctly produces:
//   OR
//   ├── AND
//   │   ├── >
//   │   │   ├── +
//   │   │   │   ├── Col(age)
//   │   │   │   └── Int(1)
//   │   │   └── Int(30)
//   │   └── !=
//   │       ├── Col(name)
//   │       └── Str("admin")
//   └── =
//       ├── Col(role)
//       └── Str("superuser")
```

Pratt parsing is used in nearly every SQL parser:
- **SQLite** uses a hand-written recursive descent parser with precedence climbing for expressions
- **CockroachDB** uses a Pratt parser for SQL expression parsing
- **DuckDB** uses precedence climbing in its expression parser
- **Programming languages** like Rust, Swift, and JavaScript all use variants of Pratt parsing

The algorithm has been rediscovered multiple times since Vaughan Pratt's 1973 paper. Its simplicity and extensibility make it the natural choice for any system that needs to parse operator expressions correctly.

---

## Try It Yourself

### Exercise 1: Power Operator

Add a right-associative exponentiation operator `^` with precedence higher than `*` but lower than unary minus. `2 ^ 3 ^ 2` should evaluate to `2 ^ (3 ^ 2) = 2 ^ 9 = 512` (right-associative), not `(2 ^ 3) ^ 2 = 8 ^ 2 = 64`.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token { Number(f64), Plus, Minus, Star, Slash, Caret, LParen, RParen, Eof }

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Bin { op: char, left: Box<Expr>, right: Box<Expr> },
    Neg(Box<Expr>),
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut t = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' => { i += 1; }
            '+' => { t.push(Token::Plus); i += 1; }
            '-' => { t.push(Token::Minus); i += 1; }
            '*' => { t.push(Token::Star); i += 1; }
            '/' => { t.push(Token::Slash); i += 1; }
            '^' => { t.push(Token::Caret); i += 1; }
            '(' => { t.push(Token::LParen); i += 1; }
            ')' => { t.push(Token::RParen); i += 1; }
            d if d.is_ascii_digit() || d == '.' => {
                let s = i;
                while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
                let n: String = c[s..i].iter().collect();
                t.push(Token::Number(n.parse().unwrap()));
            }
            x => panic!("unexpected: {}", x),
        }
    }
    t.push(Token::Eof); t
}

fn infix_bp(t: &Token) -> Option<(u8, u8)> {
    match t {
        Token::Plus | Token::Minus => Some((3, 4)),
        Token::Star | Token::Slash => Some((5, 6)),
        // RIGHT-associative: left_bp > right_bp
        // Actually for right-assoc: left_bp should be HIGHER than right_bp
        // No: right-assoc means right binds tighter, so right_bp < left_bp
        // Wait -- the convention: left_bp < right_bp = left-assoc
        //                         left_bp > right_bp = right-assoc
        // For right-associative ^: (8, 7)
        Token::Caret => Some((8, 7)),
        _ => None,
    }
}

struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
    fn new(t: Vec<Token>) -> Self { Parser { tokens: t, pos: 0 } }
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    fn parse(&mut self, min_bp: u8) -> Expr {
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Num(n),
            Token::LParen => { let e = self.parse(0); self.advance(); e }
            Token::Minus => { let o = self.parse(9); Expr::Neg(Box::new(o)) }
            t => panic!("unexpected: {:?}", t),
        };
        loop {
            let tok = self.peek().clone();
            let (lbp, rbp) = match infix_bp(&tok) { Some(bp) => bp, None => break };
            if lbp < min_bp { break; }
            self.advance();
            let rhs = self.parse(rbp);
            let op = match tok {
                Token::Plus => '+', Token::Minus => '-',
                Token::Star => '*', Token::Slash => '/',
                Token::Caret => '^', _ => unreachable!(),
            };
            lhs = Expr::Bin { op, left: Box::new(lhs), right: Box::new(rhs) };
        }
        lhs
    }
}

fn eval(e: &Expr) -> f64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Neg(o) => -eval(o),
        Expr::Bin { op, left, right } => {
            let (l, r) = (eval(left), eval(right));
            match op { '+' => l+r, '-' => l-r, '*' => l*r, '/' => l/r, '^' => l.powf(r), _ => 0.0 }
        }
    }
}

fn main() {
    let tests = vec![
        ("2 ^ 3", 8.0),
        ("2 ^ 3 ^ 2", 512.0),   // right-assoc: 2^(3^2) = 2^9 = 512
        ("(2 ^ 3) ^ 2", 64.0),  // explicit left grouping
        ("2 * 3 ^ 2", 18.0),    // ^ binds tighter than *
    ];
    for (input, expected) in tests {
        let tokens = tokenize(input);
        let mut p = Parser::new(tokens);
        let ast = p.parse(0);
        let result = eval(&ast);
        let ok = if (result - expected).abs() < 1e-10 { "OK" } else { "FAIL" };
        println!("[{}] {} = {} (expected {})", ok, input, result, expected);
    }
}
```

</details>

### Exercise 2: Ternary Conditional

Add a ternary operator `?` `:` so that `1 > 0 ? 42 : 99` evaluates to 42. Treat `?` as an infix operator that consumes everything up to `:` as the "then" branch, and the rest as the "else" branch.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token { Number(f64), Plus, Minus, Star, Gt, Lt, Question, Colon, Eof }

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Bin { op: char, l: Box<Expr>, r: Box<Expr> },
    Ternary { cond: Box<Expr>, then: Box<Expr>, otherwise: Box<Expr> },
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut t = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' => i += 1,
            '+' => { t.push(Token::Plus); i += 1; }
            '-' => { t.push(Token::Minus); i += 1; }
            '*' => { t.push(Token::Star); i += 1; }
            '>' => { t.push(Token::Gt); i += 1; }
            '<' => { t.push(Token::Lt); i += 1; }
            '?' => { t.push(Token::Question); i += 1; }
            ':' => { t.push(Token::Colon); i += 1; }
            d if d.is_ascii_digit() => {
                let s = i;
                while i < c.len() && c[i].is_ascii_digit() { i += 1; }
                let n: String = c[s..i].iter().collect();
                t.push(Token::Number(n.parse().unwrap()));
            }
            x => panic!("unexpected: {}", x),
        }
    }
    t.push(Token::Eof); t
}

fn infix_bp(t: &Token) -> Option<(u8, u8)> {
    match t {
        Token::Question => Some((1, 2)),  // lowest precedence, right-assoc
        Token::Plus | Token::Minus => Some((5, 6)),
        Token::Star => Some((7, 8)),
        Token::Gt | Token::Lt => Some((3, 4)),
        _ => None,
    }
}

struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
    fn new(t: Vec<Token>) -> Self { Parser { tokens: t, pos: 0 } }
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    fn parse(&mut self, min_bp: u8) -> Expr {
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Num(n),
            t => panic!("unexpected: {:?}", t),
        };
        loop {
            let tok = self.peek().clone();
            let (lbp, rbp) = match infix_bp(&tok) { Some(bp) => bp, None => break };
            if lbp < min_bp { break; }
            self.advance();

            if tok == Token::Question {
                // Parse the "then" branch up to the colon
                let then = self.parse(0);
                // Expect colon
                assert_eq!(*self.peek(), Token::Colon);
                self.advance();
                // Parse the "else" branch
                let otherwise = self.parse(rbp);
                lhs = Expr::Ternary {
                    cond: Box::new(lhs),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                };
            } else {
                let rhs = self.parse(rbp);
                let op = match tok {
                    Token::Plus => '+', Token::Minus => '-', Token::Star => '*',
                    Token::Gt => '>', Token::Lt => '<',
                    _ => unreachable!(),
                };
                lhs = Expr::Bin { op, l: Box::new(lhs), r: Box::new(rhs) };
            }
        }
        lhs
    }
}

fn eval(e: &Expr) -> f64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Bin { op, l, r } => {
            let (a, b) = (eval(l), eval(r));
            match op { '+'=>a+b, '-'=>a-b, '*'=>a*b, '>'=>if a>b {1.0} else {0.0},
                '<'=>if a<b {1.0} else {0.0}, _=>0.0 }
        }
        Expr::Ternary { cond, then, otherwise } => {
            if eval(cond) != 0.0 { eval(then) } else { eval(otherwise) }
        }
    }
}

fn main() {
    let tests = vec![
        ("1 > 0 ? 42 : 99", 42.0),
        ("0 > 1 ? 42 : 99", 99.0),
        ("1 > 0 ? 2 + 3 : 10", 5.0),
        ("1 > 0 ? 0 > 1 ? 1 : 2 : 3", 2.0), // nested ternary
    ];
    for (input, expected) in tests {
        let t = tokenize(input);
        let mut p = Parser::new(t);
        let ast = p.parse(0);
        let result = eval(&ast);
        let ok = if (result - expected).abs() < 1e-10 { "OK" } else { "FAIL" };
        println!("[{}] {} = {} (expected {})", ok, input, result, expected);
    }
}
```

</details>

### Exercise 3: Postfix Operators

Add a postfix factorial operator `!` so that `5!` evaluates to 120 and `3! + 1` evaluates to 7. Postfix operators are handled in the infix loop -- they have a left binding power but no right operand.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token { Number(f64), Plus, Star, Bang, LParen, RParen, Eof }

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Bin { op: char, l: Box<Expr>, r: Box<Expr> },
    Factorial(Box<Expr>),
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut t = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' => i += 1,
            '+' => { t.push(Token::Plus); i += 1; }
            '*' => { t.push(Token::Star); i += 1; }
            '!' => { t.push(Token::Bang); i += 1; }
            '(' => { t.push(Token::LParen); i += 1; }
            ')' => { t.push(Token::RParen); i += 1; }
            d if d.is_ascii_digit() => {
                let s = i;
                while i < c.len() && c[i].is_ascii_digit() { i += 1; }
                let n: String = c[s..i].iter().collect();
                t.push(Token::Number(n.parse().unwrap()));
            }
            x => panic!("unexpected: {}", x),
        }
    }
    t.push(Token::Eof); t
}

fn infix_bp(t: &Token) -> Option<(u8, u8)> {
    match t {
        Token::Plus => Some((3, 4)),
        Token::Star => Some((5, 6)),
        _ => None,
    }
}

fn postfix_bp(t: &Token) -> Option<u8> {
    match t {
        Token::Bang => Some(7), // tighter than * and +
        _ => None,
    }
}

struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
    fn new(t: Vec<Token>) -> Self { Parser { tokens: t, pos: 0 } }
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    fn parse(&mut self, min_bp: u8) -> Expr {
        let mut lhs = match self.advance() {
            Token::Number(n) => Expr::Num(n),
            Token::LParen => { let e = self.parse(0); self.advance(); e }
            t => panic!("unexpected: {:?}", t),
        };
        loop {
            let tok = self.peek().clone();

            // Check postfix first
            if let Some(bp) = postfix_bp(&tok) {
                if bp < min_bp { break; }
                self.advance();
                lhs = Expr::Factorial(Box::new(lhs));
                continue;
            }

            // Then infix
            let (lbp, rbp) = match infix_bp(&tok) { Some(bp) => bp, None => break };
            if lbp < min_bp { break; }
            self.advance();
            let rhs = self.parse(rbp);
            let op = match tok { Token::Plus => '+', Token::Star => '*', _ => unreachable!() };
            lhs = Expr::Bin { op, l: Box::new(lhs), r: Box::new(rhs) };
        }
        lhs
    }
}

fn factorial(n: f64) -> f64 {
    let n = n as u64;
    (1..=n).product::<u64>() as f64
}

fn eval(e: &Expr) -> f64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Factorial(o) => factorial(eval(o)),
        Expr::Bin { op, l, r } => {
            let (a, b) = (eval(l), eval(r));
            match op { '+' => a+b, '*' => a*b, _ => 0.0 }
        }
    }
}

fn main() {
    let tests = vec![
        ("5!", 120.0),
        ("3! + 1", 7.0),       // 6 + 1
        ("2 * 3!", 12.0),      // 2 * 6, not (2*3)!
        ("(2 * 3)!", 720.0),   // 6! = 720
    ];
    for (input, expected) in tests {
        let t = tokenize(input);
        let mut p = Parser::new(t);
        let ast = p.parse(0);
        let result = eval(&ast);
        let ok = if (result - expected).abs() < 1e-10 { "OK" } else { "FAIL" };
        println!("[{}] {} = {} (expected {})", ok, input, result, expected);
    }
}
```

</details>

---

## Recap

Precedence climbing parses operator expressions correctly with one recursive function and a binding power table. Left binding power controls when an operator yields to the caller. Right binding power controls how tightly the operator grabs its right operand. Left-associative operators have left < right. Right-associative operators have left > right. Adding new operators means adding one line to the table. This elegance is why Pratt parsing appears in virtually every database SQL parser and programming language compiler.
