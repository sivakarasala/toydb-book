# State Machines — "The lexer that reads one character at a time"

The SQL string `SELECT name FROM users WHERE age > 30` arrives at your database as a flat sequence of bytes. 43 characters. No structure, no meaning, just bytes. But buried in that byte stream are keywords (`SELECT`, `FROM`, `WHERE`), identifiers (`name`, `users`, `age`), operators (`>`), and numbers (`30`). Your database needs to pull these apart before it can do anything useful.

You could write a tangle of `if-else` chains and string searches. That works until someone sends `SELECT 'it''s a string' FROM t1` and your parser chokes on the escaped quote. Or `SELECT 123.456` and you cannot tell where the number ends. Or `SELECT >=` and you need to distinguish `>` from `>=` based on lookahead.

A state machine handles all of this cleanly. One character at a time, it transitions between states -- "I am inside a number," "I am inside a string," "I just saw a `>`" -- and emits tokens when it recognizes a complete unit. No backtracking, no lookahead beyond one character, no special cases.

---

## The Naive Way

The brute-force approach: split on whitespace and hope for the best.

```rust
fn main() {
    let sql = "SELECT name FROM users WHERE age > 30";

    let tokens: Vec<&str> = sql.split_whitespace().collect();
    println!("Tokens: {:?}", tokens);
    // ["SELECT", "name", "FROM", "users", "WHERE", "age", ">", "30"]

    // This works for simple cases. But try:
    let sql2 = "SELECT 'hello world' FROM t1";
    let tokens2: Vec<&str> = sql2.split_whitespace().collect();
    println!("Tokens: {:?}", tokens2);
    // ["SELECT", "'hello", "world'", "FROM", "t1"]
    // WRONG! The string 'hello world' got split into two tokens.

    let sql3 = "SELECT name,age FROM users WHERE age>=30";
    let tokens3: Vec<&str> = sql3.split_whitespace().collect();
    println!("Tokens: {:?}", tokens3);
    // ["SELECT", "name,age", "FROM", "users", "WHERE", "age>=30"]
    // WRONG! Commas and operators glued to identifiers.
}
```

Whitespace splitting fails because tokens are not always separated by spaces. Operators can be adjacent to identifiers (`age>=30`), commas separate column lists without spaces, and strings contain spaces that should not be split on. We need something that understands the *structure* of each character, not just whether it is a space.

---

## The Insight

Picture a turnstile at a subway station. It has two states: Locked and Unlocked. When you insert a coin, it transitions from Locked to Unlocked. When you push through, it transitions from Unlocked to Locked. Every input (coin or push) causes a deterministic state transition. The turnstile never "thinks" -- it just follows the rules.

A lexer is the same kind of machine, but with more states. At any moment, the lexer is in one of a small number of states:

- **Start**: waiting for the next token to begin
- **InIdentifier**: accumulating alphabetic characters into a word
- **InNumber**: accumulating digits into a number
- **InString**: accumulating characters inside quotes
- **InOperator**: deciding whether `>` is just `>` or the start of `>=`

Each character causes a transition. An alphabetic character in the Start state transitions to InIdentifier. A digit transitions to InNumber. A quote transitions to InString. When the lexer reaches a character that does not belong to the current state, it *emits* the accumulated token and transitions back to Start.

The beauty is that each state only cares about one question: "Does this next character belong to me?" If yes, consume it. If no, emit what you have and let the Start state handle the new character.

---

## The Build

### Tokens

First, define what the lexer produces:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    Select,
    From,
    Where,
    And,
    Or,
    Insert,
    Into,
    Values,
    Create,
    Table,

    // Literals
    Identifier(String),
    Number(String),       // keep as string to preserve "123" vs "123.0"
    StringLiteral(String),

    // Operators and punctuation
    Equals,        // =
    NotEquals,     // !=
    LessThan,      // <
    LessEqual,     // <=
    GreaterThan,   // >
    GreaterEqual,  // >=
    Plus,
    Minus,
    Star,          // * (also SELECT *)
    Slash,
    Comma,
    LeftParen,
    RightParen,
    Semicolon,
}
```

### States

The lexer's state machine:

```rust
#[derive(Debug, Clone, PartialEq)]
enum LexerState {
    Start,
    InIdentifier,
    InNumber,
    InString,
    InOperator,
}
```

### The Lexer

The lexer holds its input, position, current state, and accumulated buffer:

```rust
struct Lexer {
    input: Vec<char>,
    pos: usize,
    state: LexerState,
    buffer: String,
    tokens: Vec<Token>,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
            state: LexerState::Start,
            buffer: String::new(),
            tokens: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }
}
```

### Keyword Recognition

When we finish an identifier, check if it is actually a keyword:

```rust
fn classify_word(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        "SELECT" => Token::Select,
        "FROM" => Token::From,
        "WHERE" => Token::Where,
        "AND" => Token::And,
        "OR" => Token::Or,
        "INSERT" => Token::Insert,
        "INTO" => Token::Into,
        "VALUES" => Token::Values,
        "CREATE" => Token::Create,
        "TABLE" => Token::Table,
        _ => Token::Identifier(word.to_string()),
    }
}
```

### The Transition Function

This is the core of the state machine. Each state examines the current character and either consumes it (staying in the same state or transitioning) or emits a token (transitioning back to Start):

```rust
impl Lexer {
    fn emit_buffer(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let token = match self.state {
            LexerState::InIdentifier => classify_word(&self.buffer),
            LexerState::InNumber => Token::Number(self.buffer.clone()),
            LexerState::InString => Token::StringLiteral(self.buffer.clone()),
            _ => unreachable!(),
        };

        self.tokens.push(token);
        self.buffer.clear();
    }

    fn tokenize(mut self) -> Vec<Token> {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];

            match self.state {
                LexerState::Start => {
                    if ch.is_ascii_alphabetic() || ch == '_' {
                        self.state = LexerState::InIdentifier;
                        self.buffer.push(ch);
                        self.pos += 1;
                    } else if ch.is_ascii_digit() {
                        self.state = LexerState::InNumber;
                        self.buffer.push(ch);
                        self.pos += 1;
                    } else if ch == '\'' {
                        self.state = LexerState::InString;
                        self.pos += 1; // skip the opening quote
                    } else if ch.is_ascii_whitespace() {
                        self.pos += 1; // skip whitespace
                    } else {
                        // Operators and punctuation
                        self.pos += 1;
                        match ch {
                            '=' => self.tokens.push(Token::Equals),
                            '+' => self.tokens.push(Token::Plus),
                            '-' => self.tokens.push(Token::Minus),
                            '*' => self.tokens.push(Token::Star),
                            '/' => self.tokens.push(Token::Slash),
                            ',' => self.tokens.push(Token::Comma),
                            '(' => self.tokens.push(Token::LeftParen),
                            ')' => self.tokens.push(Token::RightParen),
                            ';' => self.tokens.push(Token::Semicolon),
                            '>' => {
                                if self.peek() == Some('=') {
                                    self.pos += 1;
                                    self.tokens.push(Token::GreaterEqual);
                                } else {
                                    self.tokens.push(Token::GreaterThan);
                                }
                            }
                            '<' => {
                                if self.peek() == Some('=') {
                                    self.pos += 1;
                                    self.tokens.push(Token::LessEqual);
                                } else {
                                    self.tokens.push(Token::LessThan);
                                }
                            }
                            '!' => {
                                if self.peek() == Some('=') {
                                    self.pos += 1;
                                    self.tokens.push(Token::NotEquals);
                                } else {
                                    panic!("unexpected character: !");
                                }
                            }
                            _ => panic!("unexpected character: {}", ch),
                        }
                    }
                }

                LexerState::InIdentifier => {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        self.buffer.push(ch);
                        self.pos += 1;
                    } else {
                        // Character doesn't belong -- emit and return to Start
                        self.emit_buffer();
                        self.state = LexerState::Start;
                        // Do NOT advance pos -- let Start handle this character
                    }
                }

                LexerState::InNumber => {
                    if ch.is_ascii_digit() || ch == '.' {
                        self.buffer.push(ch);
                        self.pos += 1;
                    } else {
                        self.emit_buffer();
                        self.state = LexerState::Start;
                    }
                }

                LexerState::InString => {
                    if ch == '\'' {
                        // Check for escaped quote ('')
                        if self.pos + 1 < self.input.len()
                            && self.input[self.pos + 1] == '\''
                        {
                            self.buffer.push('\'');
                            self.pos += 2; // skip both quotes
                        } else {
                            // End of string
                            self.emit_buffer();
                            self.state = LexerState::Start;
                            self.pos += 1; // skip closing quote
                        }
                    } else {
                        self.buffer.push(ch);
                        self.pos += 1;
                    }
                }

                LexerState::InOperator => {
                    // Handled inline in Start state for simplicity
                    unreachable!();
                }
            }
        }

        // Flush any remaining buffer
        if !self.buffer.is_empty() {
            self.emit_buffer();
        }

        self.tokens
    }
}
```

The critical detail is at the end of `InIdentifier` and `InNumber`: when we hit a character that does not belong, we emit the token but **do not advance** `pos`. The character goes back to the Start state for re-examination. This is how `age>=30` correctly produces three tokens: `age`, `>=`, `30`.

---

## The Payoff

Here is the full, runnable lexer:

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Select, From, Where, And, Or, Insert, Into, Values, Create, Table,
    Identifier(String), Number(String), StringLiteral(String),
    Equals, NotEquals, LessThan, LessEqual, GreaterThan, GreaterEqual,
    Plus, Minus, Star, Slash, Comma, LeftParen, RightParen, Semicolon,
}

fn classify_word(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        "SELECT" => Token::Select, "FROM" => Token::From,
        "WHERE" => Token::Where, "AND" => Token::And, "OR" => Token::Or,
        "INSERT" => Token::Insert, "INTO" => Token::Into,
        "VALUES" => Token::Values, "CREATE" => Token::Create,
        "TABLE" => Token::Table,
        _ => Token::Identifier(word.to_string()),
    }
}

struct Lexer { input: Vec<char>, pos: usize, buffer: String, tokens: Vec<Token>, state: u8 }
// state: 0=Start, 1=Ident, 2=Number, 3=String

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer { input: input.chars().collect(), pos: 0, buffer: String::new(), tokens: Vec::new(), state: 0 }
    }
    fn peek(&self) -> Option<char> { self.input.get(self.pos).copied() }
    fn emit(&mut self) {
        if self.buffer.is_empty() { return; }
        let tok = match self.state {
            1 => classify_word(&self.buffer),
            2 => Token::Number(self.buffer.clone()),
            3 => Token::StringLiteral(self.buffer.clone()),
            _ => unreachable!(),
        };
        self.tokens.push(tok);
        self.buffer.clear();
    }
    fn tokenize(mut self) -> Vec<Token> {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            match self.state {
                0 => { // Start
                    if ch.is_ascii_alphabetic() || ch == '_' {
                        self.state = 1; self.buffer.push(ch); self.pos += 1;
                    } else if ch.is_ascii_digit() {
                        self.state = 2; self.buffer.push(ch); self.pos += 1;
                    } else if ch == '\'' {
                        self.state = 3; self.pos += 1;
                    } else if ch.is_ascii_whitespace() {
                        self.pos += 1;
                    } else {
                        self.pos += 1;
                        match ch {
                            '=' => self.tokens.push(Token::Equals),
                            '+' => self.tokens.push(Token::Plus),
                            '-' => self.tokens.push(Token::Minus),
                            '*' => self.tokens.push(Token::Star),
                            '/' => self.tokens.push(Token::Slash),
                            ',' => self.tokens.push(Token::Comma),
                            '(' => self.tokens.push(Token::LeftParen),
                            ')' => self.tokens.push(Token::RightParen),
                            ';' => self.tokens.push(Token::Semicolon),
                            '>' => {
                                if self.peek() == Some('=') { self.pos += 1; self.tokens.push(Token::GreaterEqual); }
                                else { self.tokens.push(Token::GreaterThan); }
                            }
                            '<' => {
                                if self.peek() == Some('=') { self.pos += 1; self.tokens.push(Token::LessEqual); }
                                else { self.tokens.push(Token::LessThan); }
                            }
                            '!' => {
                                if self.peek() == Some('=') { self.pos += 1; self.tokens.push(Token::NotEquals); }
                                else { panic!("unexpected: !"); }
                            }
                            _ => panic!("unexpected: {}", ch),
                        }
                    }
                }
                1 => { // InIdentifier
                    if ch.is_ascii_alphanumeric() || ch == '_' { self.buffer.push(ch); self.pos += 1; }
                    else { self.emit(); self.state = 0; }
                }
                2 => { // InNumber
                    if ch.is_ascii_digit() || ch == '.' { self.buffer.push(ch); self.pos += 1; }
                    else { self.emit(); self.state = 0; }
                }
                3 => { // InString
                    if ch == '\'' {
                        if self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '\'' {
                            self.buffer.push('\''); self.pos += 2;
                        } else { self.emit(); self.state = 0; self.pos += 1; }
                    } else { self.buffer.push(ch); self.pos += 1; }
                }
                _ => unreachable!(),
            }
        }
        if !self.buffer.is_empty() { self.emit(); }
        self.tokens
    }
}

fn main() {
    let tests = vec![
        "SELECT name, age FROM users WHERE age >= 30",
        "SELECT * FROM orders WHERE total > 100.50",
        "SELECT 'hello world' FROM t1",
        "SELECT 'it''s escaped' FROM t1",
        "INSERT INTO users (name, age) VALUES ('Alice', 30)",
        "SELECT a+b*c FROM math WHERE x!=0",
    ];

    for sql in tests {
        println!("SQL: {}", sql);
        let tokens = Lexer::new(sql).tokenize();
        for tok in &tokens {
            println!("  {:?}", tok);
        }
        println!();
    }
}
```

Every test case produces correct tokens. The escaped quote `it''s` becomes the string `it's`. The expression `a+b*c` splits into five tokens. The operator `>=` is recognized as a single token, not `>` followed by `=`. All from a simple state machine that looks at one character at a time.

---

## Complexity Table

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Tokenize input of length n | O(n) | O(n) | Each character examined exactly once |
| Keyword lookup | O(k) | O(1) | k = keyword length (small constant) |
| Emit token | O(t) | O(t) | t = token length (copy buffer to token) |
| Total tokens produced | O(n) | O(n) | At most n tokens from n characters |
| State transitions | O(n) | O(1) | One transition per character |

The lexer is a **single-pass, linear-time** algorithm. It never backtracks, never re-reads a character, and never looks ahead more than one position. This is the hallmark of a well-designed state machine: every character is processed exactly once, and the decision at each step depends only on the current state and the current character.

---

## Where This Shows Up in Our Database

In Chapter 6, we build the SQL lexer that tokenizes user input before parsing:

```rust,ignore
// The lexer is the first stage of the SQL pipeline:
// SQL string -> Lexer -> Token stream -> Parser -> AST -> Planner -> Executor

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let lexer = Lexer::new(input);
    lexer.tokenize()
}
```

State machines appear far beyond lexers in database systems:
- **Protocol parsers** that read client messages byte by byte (PostgreSQL wire protocol)
- **WAL replay** that transitions through recovery states (initializing, replaying, consistent)
- **Replication state machines** that manage leader/follower transitions (Raft consensus)
- **Connection pool managers** that track connection lifecycle (idle, in-use, broken, draining)

Any time you have a fixed set of modes and well-defined transitions between them, a state machine is the right abstraction.

---

## Try It Yourself

### Exercise 1: Line and Column Tracking

Modify the lexer to track line and column numbers. Each token should carry its position: `Token { kind: TokenKind, line: usize, col: usize }`. Test with a multi-line SQL statement and verify the positions are correct.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    line: usize,
    col: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Select, From, Where,
    Identifier(String), Number(String), StringLiteral(String),
    Star, Comma, GreaterThan, Semicolon,
}

fn classify(word: &str) -> TokenKind {
    match word.to_uppercase().as_str() {
        "SELECT" => TokenKind::Select,
        "FROM" => TokenKind::From,
        "WHERE" => TokenKind::Where,
        _ => TokenKind::Identifier(word.to_string()),
    }
}

struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    token_line: usize,
    token_col: usize,
    buffer: String,
    tokens: Vec<Token>,
    state: u8,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(), pos: 0,
            line: 1, col: 1, token_line: 1, token_col: 1,
            buffer: String::new(), tokens: Vec::new(), state: 0,
        }
    }

    fn emit(&mut self) {
        if self.buffer.is_empty() { return; }
        let kind = match self.state {
            1 => classify(&self.buffer),
            2 => TokenKind::Number(self.buffer.clone()),
            3 => TokenKind::StringLiteral(self.buffer.clone()),
            _ => unreachable!(),
        };
        self.tokens.push(Token { kind, line: self.token_line, col: self.token_col });
        self.buffer.clear();
    }

    fn emit_single(&mut self, kind: TokenKind) {
        self.tokens.push(Token { kind, line: self.line, col: self.col - 1 });
    }

    fn advance(&mut self) -> char {
        let ch = self.input[self.pos];
        self.pos += 1;
        if ch == '\n' { self.line += 1; self.col = 1; }
        else { self.col += 1; }
        ch
    }

    fn tokenize(mut self) -> Vec<Token> {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            match self.state {
                0 => {
                    if ch.is_ascii_alphabetic() || ch == '_' {
                        self.token_line = self.line;
                        self.token_col = self.col;
                        self.state = 1;
                        self.buffer.push(ch); self.advance();
                    } else if ch.is_ascii_digit() {
                        self.token_line = self.line;
                        self.token_col = self.col;
                        self.state = 2;
                        self.buffer.push(ch); self.advance();
                    } else if ch == '\'' {
                        self.token_line = self.line;
                        self.token_col = self.col;
                        self.state = 3;
                        self.advance();
                    } else if ch.is_ascii_whitespace() {
                        self.advance();
                    } else {
                        self.advance();
                        match ch {
                            '*' => self.emit_single(TokenKind::Star),
                            ',' => self.emit_single(TokenKind::Comma),
                            '>' => self.emit_single(TokenKind::GreaterThan),
                            ';' => self.emit_single(TokenKind::Semicolon),
                            _ => {}
                        }
                    }
                }
                1 => {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        self.buffer.push(ch); self.advance();
                    } else { self.emit(); self.state = 0; }
                }
                2 => {
                    if ch.is_ascii_digit() || ch == '.' {
                        self.buffer.push(ch); self.advance();
                    } else { self.emit(); self.state = 0; }
                }
                3 => {
                    if ch == '\'' { self.emit(); self.state = 0; self.advance(); }
                    else { self.buffer.push(ch); self.advance(); }
                }
                _ => unreachable!(),
            }
        }
        if !self.buffer.is_empty() { self.emit(); }
        self.tokens
    }
}

fn main() {
    let sql = "SELECT name, age\nFROM users\nWHERE age > 30;";
    println!("SQL:\n{}\n", sql);

    let tokens = Lexer::new(sql).tokenize();
    for tok in &tokens {
        println!("  [{:>2}:{:<2}] {:?}", tok.line, tok.col, tok.kind);
    }
    // SELECT at 1:1, name at 1:8, comma at 1:12, age at 1:14
    // FROM at 2:1, users at 2:6
    // WHERE at 3:1, age at 3:7, > at 3:11, 30 at 3:13, ; at 3:15
}
```

</details>

### Exercise 2: Block Comments

Add support for block comments `/* ... */` that can span multiple lines. The lexer should skip everything between `/*` and `*/`. Test with nested SQL: `SELECT /* skip this */ name FROM t1`.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Select, From,
    Identifier(String), Number(String),
    Star, Comma,
}

fn classify(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        "SELECT" => Token::Select,
        "FROM" => Token::From,
        _ => Token::Identifier(word.to_string()),
    }
}

struct Lexer { input: Vec<char>, pos: usize, buf: String, tokens: Vec<Token>, state: u8 }
// state: 0=Start, 1=Ident, 2=Number, 4=InBlockComment

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer { input: input.chars().collect(), pos: 0, buf: String::new(), tokens: Vec::new(), state: 0 }
    }
    fn peek(&self) -> Option<char> { self.input.get(self.pos).copied() }
    fn emit(&mut self) {
        if self.buf.is_empty() { return; }
        let tok = match self.state {
            1 => classify(&self.buf),
            2 => Token::Number(self.buf.clone()),
            _ => unreachable!(),
        };
        self.tokens.push(tok); self.buf.clear();
    }
    fn tokenize(mut self) -> Vec<Token> {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            match self.state {
                0 => {
                    if ch == '/' && self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '*' {
                        self.state = 4;
                        self.pos += 2; // skip /*
                    } else if ch.is_ascii_alphabetic() || ch == '_' {
                        self.state = 1; self.buf.push(ch); self.pos += 1;
                    } else if ch.is_ascii_digit() {
                        self.state = 2; self.buf.push(ch); self.pos += 1;
                    } else if ch.is_ascii_whitespace() {
                        self.pos += 1;
                    } else {
                        self.pos += 1;
                        match ch {
                            '*' => self.tokens.push(Token::Star),
                            ',' => self.tokens.push(Token::Comma),
                            _ => {}
                        }
                    }
                }
                1 => {
                    if ch.is_ascii_alphanumeric() || ch == '_' { self.buf.push(ch); self.pos += 1; }
                    else { self.emit(); self.state = 0; }
                }
                2 => {
                    if ch.is_ascii_digit() { self.buf.push(ch); self.pos += 1; }
                    else { self.emit(); self.state = 0; }
                }
                4 => { // InBlockComment
                    if ch == '*' && self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '/' {
                        self.state = 0;
                        self.pos += 2; // skip */
                    } else {
                        self.pos += 1; // skip comment character
                    }
                }
                _ => unreachable!(),
            }
        }
        if !self.buf.is_empty() { self.emit(); }
        self.tokens
    }
}

fn main() {
    let tests = vec![
        "SELECT /* skip this */ name FROM t1",
        "SELECT /* multi\nline\ncomment */ * FROM t1",
        "SELECT name /* , age */ FROM users",
    ];

    for sql in tests {
        println!("SQL: {}", sql);
        let tokens = Lexer::new(sql).tokenize();
        for tok in &tokens {
            println!("  {:?}", tok);
        }
        println!();
    }
}
```

</details>

### Exercise 3: Error Recovery

Instead of panicking on unexpected characters, modify the lexer to collect errors and continue tokenizing. Return `Result<Vec<Token>, Vec<LexError>>` where `LexError` contains the position and character. If there are errors, return them all at once so the user can fix multiple issues in one pass.

<details>
<summary>Solution</summary>

```rust
#[derive(Debug, Clone)]
enum Token {
    Select, From, Where,
    Identifier(String), Number(String),
    Star, Comma, GreaterThan, Semicolon,
}

#[derive(Debug)]
struct LexError {
    pos: usize,
    ch: char,
    message: String,
}

fn classify(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        "SELECT" => Token::Select,
        "FROM" => Token::From,
        "WHERE" => Token::Where,
        _ => Token::Identifier(word.to_string()),
    }
}

fn tokenize(input: &str) -> (Vec<Token>, Vec<LexError>) {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut buf = String::new();
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut state: u8 = 0;

    while pos < chars.len() {
        let ch = chars[pos];
        match state {
            0 => {
                if ch.is_ascii_alphabetic() || ch == '_' {
                    state = 1; buf.push(ch); pos += 1;
                } else if ch.is_ascii_digit() {
                    state = 2; buf.push(ch); pos += 1;
                } else if ch.is_ascii_whitespace() {
                    pos += 1;
                } else {
                    pos += 1;
                    match ch {
                        '*' => tokens.push(Token::Star),
                        ',' => tokens.push(Token::Comma),
                        '>' => tokens.push(Token::GreaterThan),
                        ';' => tokens.push(Token::Semicolon),
                        _ => {
                            errors.push(LexError {
                                pos: pos - 1,
                                ch,
                                message: format!("unexpected character '{}'", ch),
                            });
                            // Continue tokenizing -- don't stop!
                        }
                    }
                }
            }
            1 => {
                if ch.is_ascii_alphanumeric() || ch == '_' { buf.push(ch); pos += 1; }
                else {
                    tokens.push(classify(&buf)); buf.clear(); state = 0;
                }
            }
            2 => {
                if ch.is_ascii_digit() || ch == '.' { buf.push(ch); pos += 1; }
                else {
                    tokens.push(Token::Number(buf.clone())); buf.clear(); state = 0;
                }
            }
            _ => unreachable!(),
        }
    }
    if !buf.is_empty() {
        match state {
            1 => tokens.push(classify(&buf)),
            2 => tokens.push(Token::Number(buf)),
            _ => {}
        }
    }

    (tokens, errors)
}

fn main() {
    let sql = "SELECT name, @age FROM users WHERE # > 30;";
    println!("SQL: {}\n", sql);

    let (tokens, errors) = tokenize(sql);

    println!("Tokens:");
    for tok in &tokens {
        println!("  {:?}", tok);
    }

    if !errors.is_empty() {
        println!("\nErrors:");
        for err in &errors {
            println!("  Position {}: {}", err.pos, err.message);
        }
    }
    // The lexer recovers from @ and # and still produces
    // all the valid tokens around them.
}
```

</details>

---

## Recap

A state machine lexer is a disciplined way to turn a stream of characters into structured tokens. Each state knows what characters belong to it and what to do when it encounters a character that does not. The transitions are explicit, the logic per state is small, and the whole thing runs in a single linear pass. For a database that needs to parse thousands of SQL queries per second, this simplicity is a feature, not a limitation.
