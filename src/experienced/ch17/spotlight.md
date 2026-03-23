## Spotlight: Module System & Workspace

Every chapter has one spotlight concept. This chapter's spotlight is **module system and workspace** — how Rust organizes code at every scale, from a single function's visibility to a multi-crate project.

### Modules: organizing code within a crate

A Rust crate is a compilation unit — the smallest thing the compiler processes as a whole. Within a crate, **modules** organize code into namespaces:

```rust,ignore
// src/lib.rs

mod storage;     // loads from src/storage.rs or src/storage/mod.rs
mod sql;         // loads from src/sql.rs or src/sql/mod.rs
mod raft;        // loads from src/raft.rs or src/raft/mod.rs
mod server;      // loads from src/server.rs or src/server/mod.rs
```

Each `mod` declaration creates a namespace and tells the compiler to include that file. Modules can be nested:

```rust,ignore
// src/sql/mod.rs

pub mod lexer;      // src/sql/lexer.rs
pub mod parser;     // src/sql/parser.rs
pub mod planner;    // src/sql/planner.rs
pub mod optimizer;  // src/sql/optimizer.rs
pub mod executor;   // src/sql/executor.rs
```

The file system structure mirrors the module hierarchy. This is not a convention — it is a rule. The compiler looks for `src/sql/lexer.rs` because the module path is `sql::lexer`.

### Visibility: pub, pub(crate), and private

Rust defaults to private. Everything is hidden unless you explicitly expose it:

```rust,ignore
// src/sql/parser.rs

pub struct Parser {           // visible to anyone who can see the `sql::parser` module
    tokens: Vec<Token>,       // PRIVATE — only Parser's own methods can access this
    position: usize,          // PRIVATE
}

pub(crate) fn validate_ast(  // visible within this crate, but not to external crates
    ast: &Statement,
) -> Result<(), String> {
    // ...
}

fn consume_token(            // PRIVATE — only functions in this module can call this
    tokens: &[Token],
    pos: &mut usize,
) -> Option<&Token> {
    // ...
}
```

Three levels:
- **Private** (no keyword): visible only within the same module and its children
- **`pub(crate)`**: visible anywhere in the same crate, but not exported to dependents
- **`pub`**: visible to anyone, including external crates

This maps directly to our database layers. The parser's internal state (`tokens`, `position`) is private — no one outside the parser needs to know about token positions. The `validate_ast` function is `pub(crate)` — the server uses it, but external users of our library should not. The `Parser` struct and its `parse()` method are `pub` — they are the public API.

### The `use` keyword: bringing names into scope

Without `use`, you write full paths everywhere:

```rust,ignore
// Verbose — every type fully qualified
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

fn execute(
    plan: Plan,
    storage: &mut MvccStorage,
) -> ResultSet {
    // ...
}
```

The convention: import types (structs, enums, traits) directly. Import functions through their parent module to avoid ambiguity:

```rust,ignore
use std::io;            // then use: io::Read, io::Write
use std::io::BufReader; // type imported directly
use std::collections::HashMap; // type imported directly
```

### Workspaces: multiple crates in one repository

As a project grows, a single crate becomes unwieldy. Compile times increase because any change recompiles everything. Testing is slower. Dependencies are shared when they should not be.

A **workspace** splits the project into multiple crates that live in the same repository:

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "toydb-storage",
    "toydb-sql",
    "toydb-raft",
    "toydb-server",
]
```

Each member is an independent crate with its own `Cargo.toml`, `src/`, and tests:

```
toydb/
├── Cargo.toml          (workspace root)
├── toydb-storage/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── kv.rs
│       ├── bitcask.rs
│       └── mvcc.rs
├── toydb-sql/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── lexer.rs
│       ├── parser.rs
│       ├── planner.rs
│       ├── optimizer.rs
│       └── executor.rs
├── toydb-raft/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── node.rs
│       ├── wal.rs
│       ├── state.rs
│       └── snapshot.rs
└── toydb-server/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── server.rs
        └── client.rs
```

Crates in a workspace can depend on each other:

```toml
# toydb-server/Cargo.toml
[dependencies]
toydb-storage = { path = "../toydb-storage" }
toydb-sql = { path = "../toydb-sql" }
toydb-raft = { path = "../toydb-raft" }
```

The dependency graph enforces layer boundaries: `toydb-sql` does not depend on `toydb-raft`, so SQL code cannot accidentally call Raft functions. This is information hiding enforced by the build system.

### Re-exports: simplifying the public API

A workspace crate might have deep module paths. Re-exports flatten them for consumers:

```rust,ignore
// toydb-sql/src/lib.rs

pub mod lexer;
pub mod parser;
pub mod planner;
pub mod optimizer;
pub mod executor;

// Re-export the most commonly used types at the crate root
pub use parser::Parser;
pub use planner::{Plan, Planner};
pub use executor::{Executor, ResultSet};
pub use lexer::Lexer;
```

Now `toydb-server` can write `use toydb_sql::Parser` instead of `use toydb_sql::parser::Parser`. The internal module structure is an implementation detail hidden behind convenient re-exports.

> **Coming from JS/Python/Go?**
>
> | Concept | JavaScript | Python | Go | Rust |
> |---------|-----------|--------|-----|------|
> | Module | ES module (file) | Module (file) | Package (directory) | Module (file or directory) |
> | Import | `import { X } from './x'` | `from x import X` | `import "pkg"` | `use crate::x::X` |
> | Private | No enforcement (convention: `_`) | Convention: `_prefix` | Lowercase first letter | Default (no keyword) |
> | Public | `export` keyword | No enforcement | Uppercase first letter | `pub` keyword |
> | Workspace | npm workspaces / yarn | Not built-in (setuptools) | Go modules | Cargo workspace |
> | Re-export | `export { X } from './x'` | `from x import X` in `__init__.py` | Not needed (package = directory) | `pub use x::X` |
>
> Go's approach is closest to Rust's: packages are directories, visibility is controlled by case. But Go has only two levels (exported/unexported), while Rust has three (pub/pub(crate)/private). And Go's package system is based on directories, while Rust's module system can nest arbitrarily within a single file.
>
> The biggest difference from JavaScript and Python: Rust's visibility rules are enforced by the compiler. In JS, "private" is a naming convention (or the relatively new `#private` syntax). In Python, `_private` is a suggestion that tools and people routinely ignore. In Rust, if a field is not `pub`, external code literally cannot access it — the program will not compile.

---
