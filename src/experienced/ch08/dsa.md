## DSA in Context: Tree-to-Tree Transformation

The query planner performs a **tree-to-tree transformation**: it takes an AST (one tree structure) and produces a plan (a different tree structure). This is a fundamental operation in computer science — compilers, transpilers, and query engines all do it.

### The transformation pattern

The AST for `SELECT name FROM users WHERE age > 18`:

```
Statement::Select
├── columns: [Named("name")]
├── from: "users"
└── where_clause:
    BinaryOp (>)
    ├── Column("age")
    └── Literal(18)
```

The plan tree for the same query:

```
Plan::Project [name]
└── Plan::Filter (age > 18)
    └── Plan::Scan users [id, name, age, email]
```

These are different tree shapes. The AST mirrors the SQL syntax (SELECT comes first, FROM comes second). The plan mirrors the execution order (Scan happens first, Project happens last). The planner maps one structure to the other.

### DFS for validation

When the planner validates an expression like `age > 18 AND name = 'Alice'`, it performs a DFS traversal of the expression tree:

```
        AND
       /   \
      >      =
     / \    / \
   age  18 name 'Alice'
```

The DFS visits nodes in this order: `AND` -> `>` -> `age` -> `18` -> `=` -> `name` -> `'Alice'`. At each leaf, it checks whether column references are valid. At each inner node, it checks type compatibility. The `?` operator provides early termination: if `age` does not exist, the traversal stops immediately.

This DFS validation is O(n) where n is the number of nodes in the expression tree. Each node is visited exactly once. There is no need for BFS here because validation does not depend on level ordering — we just need to visit every node.

### Recursive descent for transformation

The planner uses **recursive descent** to build the plan. For a SELECT statement:

1. Look up the table name in the schema (resolves the leaf).
2. Build a Scan node (the deepest node in the plan tree).
3. If there is a WHERE clause, wrap the Scan in a Filter.
4. If there are specific columns, wrap in a Project.

Each step takes the current subtree and wraps it in a new node. This is the same pattern as building a linked list by prepending: each new node becomes the root, and the old tree becomes a child.

---
