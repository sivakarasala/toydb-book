## Design Insight: Obvious Code

> *"The best code is code that is obvious — if someone reads it, they immediately understand what it does."*
> — John Ousterhout, *A Philosophy of Software Design*

Look at the API you built:

```rust
db.set("name".to_string(), Value::parse("Alice"));
db.get("name");
db.delete("name");
db.list();
db.stats();
```

There is no cleverness here. The struct is named `Database`. The methods are named `set`, `get`, `delete`, `list`, `stats`. A programmer who has never seen your code can read any of these lines and know exactly what it does. This is not an accident — it is a design choice.

Ousterhout calls this "obvious code." The alternative is "clever code" — code that uses non-obvious tricks, obscure patterns, or requires extensive documentation to understand. Clever code is fun to write and painful to maintain.

Throughout this book, we will prefer obvious names over short names. `OperationStats` over `OpStats`. `entry_count` over `len` (because `len` could mean key count, byte count, or bucket count). `type_name` over `kind`. Every name should answer the question "what is this?" without context.

---
