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
