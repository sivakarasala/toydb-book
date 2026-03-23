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
