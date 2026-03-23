## DSA in Context: Module Dependency Graphs

The integration of multiple layers creates a **dependency graph** — a directed graph where nodes are modules and edges are "depends on" relationships.

### Our dependency graph

```
toydb-server
├── toydb-sql
│   └── (no external deps)
├── toydb-storage
│   └── (no external deps)
└── toydb-raft
    └── (no external deps)
```

This is a **tree** — no cycles. `toydb-sql` does not depend on `toydb-raft`. `toydb-storage` does not depend on `toydb-sql`. Only `toydb-server` depends on all three.

### Why cycles are dangerous

If `toydb-sql` depended on `toydb-storage` AND `toydb-storage` depended on `toydb-sql`, you would have a **cycle**:

```
toydb-sql ←→ toydb-storage  (CYCLE — does not compile!)
```

Rust's crate system forbids cyclic dependencies — the compiler rejects them. This is a feature, not a limitation. Cyclic dependencies mean the two crates cannot be understood independently. A change in either one might break the other. Testing requires both to be present. The cycle binds them into a single conceptual unit that should probably be a single crate.

### Topological sort and build order

Cargo builds crates in **topological order** — a crate is compiled only after all its dependencies are compiled. For our workspace:

```
Build order:
1. toydb-sql      (no deps — can build immediately)
2. toydb-storage   (no deps — can build in parallel with toydb-sql)
3. toydb-raft      (no deps — can build in parallel with both above)
4. toydb-server    (depends on all three — must wait for them)
```

Steps 1-3 can run in parallel because they have no dependencies on each other. This is why workspaces with many leaf crates build faster than monolithic crates — the compiler can parallelize.

### Dependency inversion

What if the executor needs to know about storage, but storage should not know about the executor? Use a **trait** (interface) to invert the dependency:

```rust,ignore
// In toydb-sql (no dependency on toydb-storage)
pub trait Storage {
    fn scan_table(&self, table: &str) -> Box<dyn Iterator<Item = Row>>;
    fn insert_row(&mut self, table: &str, row: Row) -> Result<(), String>;
}

// In toydb-storage (no dependency on toydb-sql)
impl Storage for MvccStorage {
    fn scan_table(&self, table: &str) -> Box<dyn Iterator<Item = Row>> {
        // ...
    }
    fn insert_row(&mut self, table: &str, row: Row) -> Result<(), String> {
        // ...
    }
}
```

Wait — this does not work as written. `toydb-storage` would need to depend on `toydb-sql` to implement the `Storage` trait. The solution is to put the trait in a separate crate (`toydb-traits`) or use the `Storage` trait in the server crate where both are available. This is the **dependency inversion principle**: high-level modules define interfaces, low-level modules implement them, and both depend on the abstraction (the trait) rather than on each other.

---
