## DSA in Context: Abstract Syntax Trees

An **Abstract Syntax Tree** (AST) is a tree data structure where:
- **Leaf nodes** are values: column references, literals, constants
- **Internal nodes** are operations: binary operators, unary operators, function calls
- **The root** is the top-level operation

The "abstract" in AST means it discards syntactic details that do not affect meaning. Parentheses, whitespace, and operator tokens are not in the tree — only the structure they imply. `(1 + 2) * 3` and `((1 + 2)) * 3` produce identical ASTs because the extra parentheses change nothing.

### Tree traversal

Processing an AST requires tree traversal — visiting every node. The two fundamental traversals are:

**Pre-order (visit parent before children):**
```
visit(node):
    process(node)
    for child in node.children:
        visit(child)
```

Used for: printing the tree, copying the tree, serializing.

**Post-order (visit children before parent):**
```
visit(node):
    for child in node.children:
        visit(child)
    process(node)
```

Used for: evaluating expressions (you need child values before you can compute the parent), freeing memory (free children before the parent).

Our `eval` function from Drill 1 is a post-order traversal — it evaluates children first (`eval(left)`, `eval(right)`), then combines the results. The `pretty_print` function is a pre-order traversal — it prints the current node's operator first, then recursively prints children.

### Precedence climbing as tree construction

The Pratt parsing algorithm from Exercise 3 builds the AST bottom-up. Higher-precedence operators become deeper nodes (closer to the leaves), and lower-precedence operators become shallower nodes (closer to the root). This is correct because tree evaluation is post-order: deeper nodes evaluate first, and deeper means "higher precedence."

For `1 + 2 * 3`:

```
    Add         <- root (evaluated last)
   /   \
  1    Mul      <- deeper (evaluated first)
      /   \
     2     3
```

The `*` is deeper than `+`, so it evaluates first: `2 * 3 = 6`, then `1 + 6 = 7`. Precedence climbing ensures this tree shape by only allowing higher-precedence operators to be consumed into the right-hand side of lower-precedence operators.

### Time and space complexity

Parsing is O(N) where N is the number of tokens. Each token is consumed exactly once — the parser never backtracks. The resulting AST has O(N) nodes (one leaf per value token, one internal node per operator token). Evaluating the AST is also O(N) — one visit per node.

The space complexity of the AST is O(N) for the nodes plus O(D) stack space for recursive traversal, where D is the depth of the tree. For typical SQL queries, D is small (under 20 levels of nesting). For pathological queries like `a AND b AND c AND ... AND z`, the left-associative tree has depth N, but this is still manageable.

---
