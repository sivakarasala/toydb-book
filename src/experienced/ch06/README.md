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
