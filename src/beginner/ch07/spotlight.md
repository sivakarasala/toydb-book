## Spotlight: Recursive Types & Box

Every chapter has one **spotlight concept** -- the Rust idea we dig into deeply. This chapter's spotlight is **recursive types and `Box<T>`**.

### The problem: a type that contains itself

Imagine you are building a family tree. A person has a name and two parents, each of whom is also a person. In Rust, you might try this:

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

To understand why this fails, you need to know something about how Rust stores data.

### Stack memory: everything has a fixed size

When Rust creates a variable, it puts it on the **stack** -- a region of memory where every item has a fixed, known size. An `i64` is always 8 bytes. A `bool` is always 1 byte. An `enum` is the size of its largest variant (plus a small tag that says which variant it is).

So what is the size of `Expression`? Let us try to calculate:

- `Literal(i64)` -- 8 bytes. Easy.
- `Add { left: Expression, right: Expression }` -- that is the size of two `Expression` values. But each `Expression` could be another `Add`, which contains two more `Expression` values, which could be `Add`...

This goes on forever. The size is infinite. The compiler cannot calculate it.

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

### What is the heap?

Besides the stack, your program has access to the **heap** -- a much larger region of memory where data can live for as long as you want, in any size you want. The trade-off is that accessing heap memory is slightly slower than stack memory, and you have to explicitly ask for it.

Think of it like this:
- **Stack** = a stack of plates at a cafeteria. Each plate is the same size, and you can only add or remove from the top. Fast, but rigid.
- **Heap** = a storage warehouse. You can request any amount of space, and you get a ticket (a pointer) that tells you where your stuff is stored. More flexible, but you need the ticket to find your things.

### Box: a pointer to heap-allocated data

`Box<T>` is Rust's way of saying "put this value on the heap and give me a pointer to it." A `Box` is always the same size -- 8 bytes on a 64-bit computer -- regardless of what it points to. It is just a pointer (a ticket to the warehouse).

```rust
// This DOES compile
enum Expression {
    Literal(i64),           // 8 bytes
    Add {
        left: Box<Expression>,   // 8 bytes (just a pointer)
        right: Box<Expression>,  // 8 bytes (just a pointer)
    },
}
```

Now Rust can calculate the size: `Add` is 16 bytes (two pointers), `Literal` is 8 bytes, so `Expression` is 16 bytes plus a small tag. The actual child `Expression` values live on the heap, and they can be nested as deeply as you want.

> **What just happened?**
>
> We solved an infinite-size problem by using indirection. Instead of storing child `Expression` values directly inside the parent (which would make the parent infinitely large), we store pointers to children on the heap. Each pointer is a fixed 8 bytes. The children themselves can be as complex as they like -- their size does not affect the parent's size.

### Creating boxed values

To put a value in a `Box`, you use `Box::new()`:

```rust
let expr = Expression::Add {
    left: Box::new(Expression::Literal(1)),
    right: Box::new(Expression::Add {
        left: Box::new(Expression::Literal(2)),
        right: Box::new(Expression::Literal(3)),
    }),
};
// This represents: 1 + (2 + 3)
```

Each `Box::new(...)` allocates space on the heap, puts the value there, and gives you back a pointer. The tree looks like this in memory:

```
Stack:                  Heap:
┌──────────────┐
│ Add          │
│  left: ──────────────> Literal(1)
│  right: ─────────────> Add
│              │           left: ──────> Literal(2)
└──────────────┘           right: ─────> Literal(3)
```

### Pattern matching on boxed values

Matching on a `Box` works just like matching on a regular value. Rust automatically "looks through" the Box for you:

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

Inside the `Add` arm, `left` and `right` are `&Box<Expression>`. But Rust has a feature called **auto-dereferencing** -- it automatically follows the pointer inside the `Box` to get to the `Expression` inside. So you can pass `left` directly to a function expecting `&Expression`. You do not need to write any extra code to "unbox" the value.

> **What just happened?**
>
> `Box<T>` implements a trait called `Deref`, which lets Rust automatically treat a `&Box<T>` as a `&T`. When you call `evaluate(left)`, Rust sees that `left` is `&Box<Expression>`, but `evaluate` expects `&Expression`. It auto-dereferences the Box for you. This is why `Box` feels transparent in pattern matching -- you rarely need to think about it.

### Why not use references instead of Box?

You might wonder: could we write `left: &Expression` instead of `Box<Expression>`? The answer is no, and the reason is **ownership**.

A reference (`&Expression`) borrows data that exists somewhere else. But when the parser builds an AST, it *creates* new nodes. There is no "somewhere else" to borrow from -- the parser is making brand new `Expression` values and assembling them into a tree. The tree needs to **own** its children.

- `Box<T>` = "I own this value. It lives on the heap, and when I am dropped, it will be freed."
- `&T` = "I am borrowing this value. Someone else owns it and is responsible for keeping it alive."

The AST owns its nodes, so it uses `Box`.

### Why trees matter for SQL

An AST is a tree. Trees are recursive by definition: a tree is either a leaf (no children) or a node with children that are themselves trees. In Rust, "a type that contains itself" requires heap indirection with `Box`. This is not a Rust limitation -- every language heap-allocates tree children. JavaScript and Python just do it automatically (every object is on the heap). Rust makes you say so explicitly with `Box::new()`.

> **Common Mistakes**
>
> 1. **Forgetting `Box::new()`**: Writing `left: Expression::Literal(1)` instead of `left: Box::new(Expression::Literal(1))`. The compiler will tell you: "expected `Box<Expression>`, found `Expression`."
>
> 2. **Trying to use `&Expression` instead of `Box<Expression>`**: References need a lifetime and an existing owner. For owned tree nodes, always use `Box`.
>
> 3. **Over-boxing**: You do not need `Box` for non-recursive fields. `Literal(i64)` stores the `i64` directly -- no Box needed because `i64` has a known, fixed size and does not recurse.

---
