# Chapter 8: Query Planner

In the last two chapters, we built a lexer and a parser. Given the SQL string `SELECT name FROM users WHERE age > 18`, we can now produce a beautiful tree -- an Abstract Syntax Tree -- that describes the structure of the query. But the AST describes WHAT the user asked for. It does not describe HOW to get it.

Think of the difference between a recipe and the act of cooking. The recipe says "make a cake" -- it lists ingredients and steps. But you still need to actually open the fridge, measure the flour, preheat the oven, and mix the batter. The recipe is the AST. The cooking instructions -- in the right order, with the right tools -- that is the **query plan**.

This chapter builds a query planner that transforms the AST into a plan -- a tree of operations that an executor can process step by step. The planner also validates your SQL: it checks that tables exist, columns are real, and types make sense. If you write `SELECT name FROM unicorns`, the planner will tell you there is no table called `unicorns`.

By the end of this chapter, you will have:

- A `Plan` enum representing execution plan nodes (Scan, Filter, Project, Insert, CreateTable)
- A `Schema` catalog that knows which tables and columns exist
- A `Planner` that transforms a parsed `Statement` into a validated `Plan`
- A `Display` implementation that prints plans as indented trees (like SQL's EXPLAIN)
- A deep understanding of Rust iterators and closures

---
