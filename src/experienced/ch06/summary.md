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
