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

**-> [Parsing Theory & Grammars -- "From Tokens to Trees"](../../ds-narratives/ch07-ast-as-tree.md)**

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
