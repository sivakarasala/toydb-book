## Spotlight: Recursive Types & Box

Every chapter has one spotlight concept. This chapter's spotlight is **recursive types and `Box<T>`** — the mechanism Rust uses to make tree-shaped data structures possible.

### The infinite size problem

Consider this attempt at an expression type:

```rust
// This does NOT compile
enum Expression {
    Literal(i64),
    Add {
        left: Expression,   // How big is this?
        right: Expression,  // And this?
    },
}
```

Rust stores enum values inline on the stack. To do that, it needs to know the size of every variant at compile time. The size of an enum is the size of its largest variant. `Literal(i64)` is 8 bytes. But `Add`? It contains two `Expression` values, each of which could themselves be `Add`, which contains two more `Expression` values, which could be `Add`, which... this is infinite recursion. The compiler cannot calculate a finite size.

```
error[E0072]: recursive type `Expression` has infinite size
 --> src/lib.rs:1:1
  |
1 | enum Expression {
  | ^^^^^^^^^^^^^^^
2 |     Literal(i64),
3 |     Add {
4 |         left: Expression,
  |               ---------- recursive without indirection
  |
help: insert some indirection (e.g., a `Box`, `Rc`, or `&`) to break the cycle
```

The error message even tells you the fix.

### Box: heap allocation breaks the cycle

`Box<T>` is a pointer to a heap-allocated value. It is always the same size — 8 bytes on a 64-bit system — regardless of what `T` is. By putting the recursive field behind a `Box`, you replace an inline value of unknown size with a pointer of known size:

```rust
enum Expression {
    Literal(i64),           // 8 bytes
    Add {
        left: Box<Expression>,   // 8 bytes (pointer)
        right: Box<Expression>,  // 8 bytes (pointer)
    },
}
```

Now Rust can calculate the size: `Add` is 16 bytes (two pointers), `Literal` is 8 bytes, so `Expression` is 16 bytes plus the enum discriminant. The actual `Expression` values that `left` and `right` point to live on the heap, and they can be as deeply nested as you want.

### Creating and matching boxed values

Creating a boxed value:

```rust
let expr = Expression::Add {
    left: Box::new(Expression::Literal(1)),
    right: Box::new(Expression::Add {
        left: Box::new(Expression::Literal(2)),
        right: Box::new(Expression::Literal(3)),
    }),
};
// Represents: 1 + (2 + 3)
```

Pattern matching on boxed values works the same as unboxed — you just dereference with `*`:

```rust
fn evaluate(expr: &Expression) -> i64 {
    match expr {
        Expression::Literal(n) => *n,
        Expression::Add { left, right } => {
            evaluate(left) + evaluate(right)
        }
    }
}
```

Inside the `Add` arm, `left` and `right` are `&Box<Expression>`. But `Box<T>` implements `Deref<Target = T>`, so you can pass them directly to a function expecting `&Expression`. Rust auto-dereferences the `Box` for you.

### Why not just use references?

You might wonder: why not `left: &Expression` instead of `Box<Expression>`? References borrow data that exists somewhere else. But when building an AST, the parser *creates* the nodes — there is no "somewhere else" to borrow from. `Box` is for owned, heap-allocated data. References are for temporary borrows of existing data. The AST owns its nodes, so it uses `Box`.

### Tree-shaped data is naturally recursive

An AST is a tree. Trees are recursive by definition: a tree is either a leaf or a node with children that are themselves trees. In Rust, "a type that contains itself" requires heap indirection. This is not a limitation — it is the type system making explicit what other languages hide. Every tree in JavaScript, Python, and Go also heap-allocates its children. Rust just makes you say so.

> **Coming from other languages?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|----|------|
> | Recursive type | Objects nest freely | Objects nest freely | `*Node` pointer | `Box<Node>` |
> | Heap allocation | Automatic (all objects) | Automatic (all objects) | Explicit (`new()`) | Explicit (`Box::new()`) |
> | Stack vs heap | No control | No control | Compiler decides | You decide |
> | Size of a pointer | Hidden | Hidden | 8 bytes | 8 bytes (`Box<T>`) |
> | Deref to inner type | N/A | N/A | Automatic (`*p`) | Automatic (`Deref` trait) |
>
> The key difference: in JavaScript and Python, *everything* is heap-allocated, so recursive types "just work" — but you pay the allocation cost for every value, even simple integers. In Rust, you only heap-allocate when you need to (recursive types, dynamic dispatch, large values). `Box::new()` is Rust saying "I know this needs to go on the heap."

---
