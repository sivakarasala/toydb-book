# Chapter 8: Query Planner

You have a lexer that turns SQL strings into tokens and a parser that arranges those tokens into an AST. If you stopped here, you could walk the AST and immediately start reading and writing data. Many toy databases do exactly this. But every production database has a stage between parsing and execution: the query planner. The planner takes the AST — which describes WHAT the user asked for — and produces a plan — which describes HOW to get it. This separation is one of the most important architectural decisions in database design, and this chapter shows you why.

The planner resolves table names against a schema catalog, validates that columns actually exist, type-checks expressions, and assembles a tree of plan nodes. The result is a self-contained instruction set that an executor can process without ever looking at the original SQL again.

By the end of this chapter, you will have:

- A `Plan` enum representing execution plan nodes (Scan, Filter, Project, Insert, Update, Delete, CreateTable)
- A `Schema` catalog that knows which tables and columns exist
- A `Planner` that transforms a parsed `Statement` into a validated `Plan`
- Schema validation with descriptive error messages for missing tables and columns
- A `Display` implementation that prints plans as indented trees (like SQL's EXPLAIN)
- A deep understanding of Rust iterators, closures, and iterator adaptors

---
