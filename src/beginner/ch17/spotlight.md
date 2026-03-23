## Spotlight: Module System & Workspace

Every chapter has one **spotlight concept**. This chapter's spotlight is **module system and workspace** -- how Rust organizes code into pieces and controls what each piece can see.

### The room analogy

Think of your project as a house. Each **module** is a room:

```
Your Database House:
+------------------+------------------+
|     Kitchen      |   Living Room    |
|   (storage)      |    (server)      |
|   - stove (kv)   |   - couch (tcp)  |
|   - fridge(mvcc) |   - tv (handler) |
+------------------+------------------+
|     Bedroom      |    Study         |
|     (sql)        |    (raft)        |
|   - desk(parser) |   - books(wal)   |
|   - lamp(lexer)  |   - chair(node)  |
+------------------+------------------+
```

Each room has its own furniture (functions and types). Doors between rooms (`pub`) control what you can access from where. The kitchen's stove is visible from the living room (because we made it `pub`), but the bedroom's private diary is hidden.

### Creating modules with `mod`

In Rust, you declare modules in your `lib.rs` or `main.rs`:

```rust,ignore
// src/lib.rs

mod storage;   // tells Rust: "load src/storage.rs"
mod sql;       // tells Rust: "load src/sql.rs"
mod raft;      // tells Rust: "load src/raft.rs"
mod server;    // tells Rust: "load src/server.rs"
```

Each `mod` declaration does two things:
1. Creates a namespace (like `storage::MvccStorage`)
2. Tells the compiler to include that file

Modules can be nested. If `sql` is a directory:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;      // loads src/sql/lexer.rs
pub mod parser;     // loads src/sql/parser.rs
pub mod planner;    // loads src/sql/planner.rs
pub mod executor;   // loads src/sql/executor.rs
```

> **Programming Concept: File = Module**
>
> In Rust, the file system structure mirrors the module hierarchy. This is not a convention -- it is a rule. If you write `mod sql;` in `lib.rs`, the compiler looks for `src/sql.rs` or `src/sql/mod.rs`. If neither exists, you get a compile error. This makes it easy to find code -- the module path tells you exactly which file to open.

### Visibility: `pub`, `pub(crate)`, and private

By default, everything in Rust is **private**. You cannot access a function, struct, or field from outside its module unless you explicitly make it visible:

```rust,ignore
// src/sql/parser.rs

// Anyone can see and use this struct
pub struct Parser {
    tokens: Vec<Token>,   // PRIVATE -- only Parser's own code can touch this
    position: usize,      // PRIVATE
}

// Only code in this crate can call this function
pub(crate) fn validate_ast(ast: &Statement) -> Result<(), String> {
    // ...
}

// Only code in this module can call this function
fn consume_token(tokens: &[Token], pos: &mut usize) -> Option<&Token> {
    // ...
}
```

Three levels of visibility:

| Keyword | Who can see it? | Analogy |
|---------|----------------|---------|
| (nothing) | Same module only | Your bedroom -- only you go in |
| `pub(crate)` | Anywhere in this crate | The kitchen -- family only |
| `pub` | Anyone, including external code | The front porch -- the whole neighborhood |

This maps directly to our database layers. The parser's internal state (`tokens`, `position`) is private -- no one outside the parser needs to know about token positions. The `Parser` struct itself is `pub` -- it is the public API that the server uses.

### The `use` keyword: importing names

Without `use`, you would write full paths everywhere:

```rust,ignore
// Without use -- verbose
fn execute(
    plan: crate::sql::planner::Plan,
    storage: &mut crate::storage::mvcc::MvccStorage,
) -> crate::sql::executor::ResultSet {
    // ...
}
```

With `use`, you import names into the current scope:

```rust,ignore
use crate::sql::planner::Plan;
use crate::sql::executor::ResultSet;
use crate::storage::mvcc::MvccStorage;

// Much cleaner
fn execute(plan: Plan, storage: &mut MvccStorage) -> ResultSet {
    // ...
}
```

The convention: import types (structs, enums) directly. Import functions through their parent module to avoid confusion about where they come from:

```rust,ignore
use std::io;             // then call: io::Read, io::Write
use std::io::BufReader;  // type imported directly
use std::collections::HashMap;  // type imported directly
```

> **What Just Happened?**
>
> We covered Rust's module system:
> - **`mod`** declares a module and tells the compiler which file to load
> - **`pub`** makes things visible outside their module
> - **`use`** imports names so you do not write full paths
> - The file system structure mirrors the module hierarchy
>
> This is how we organize thousands of lines of database code into manageable pieces, with clear boundaries between layers.

### Re-exports: simplifying the public API

Sometimes your module structure has deep paths. Re-exports flatten them:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;
pub mod parser;
pub mod planner;
pub mod executor;

// Re-export the most commonly used types at the module root
pub use parser::Parser;
pub use planner::{Plan, Planner};
pub use executor::{Executor, ResultSet};
```

Now other code can write `use crate::sql::Parser` instead of `use crate::sql::parser::Parser`. The internal module structure is hidden behind convenient re-exports.

---
