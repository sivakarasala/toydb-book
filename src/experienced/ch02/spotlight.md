## Spotlight: Traits & Generics

Every chapter has one spotlight concept. This chapter's spotlight is **traits and generics** — the way Rust defines shared behavior and writes code that works across multiple types.

### What is a trait?

A trait is a contract. It says: "any type that implements me must provide these methods." Think of it as a promise a type makes to the rest of your codebase.

```rust
trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
}
```

This trait declares two methods but provides no implementations. Any type that wants to call itself a `Storage` must define both. The compiler enforces this at compile time — not at runtime, not in tests, not "hopefully in code review." If you forget a method, the code does not build.

### Implementing a trait

To fulfill the contract, you write an `impl TraitName for YourType` block:

```rust
struct MemoryStorage {
    data: BTreeMap<String, Vec<u8>>,
}

impl Storage for MemoryStorage {
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), Error> {
        self.data.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.data.get(key).cloned())
    }
}
```

The struct has its own data (`BTreeMap`), and the trait implementation defines how that data is accessed through the `Storage` interface. You can have multiple types implementing the same trait — a `DiskStorage`, a `NetworkStorage`, a `MockStorage` for tests — and they all fulfill the same contract.

### Trait bounds on generics

Here is where traits become powerful. You can write a function (or a struct) that works with *any* type implementing a trait:

```rust
fn count_keys<S: Storage>(store: &S) -> usize {
    // This function works with MemoryStorage, DiskStorage, anything
    // that implements Storage
    todo!()
}
```

The `<S: Storage>` syntax says: "S can be any type, as long as it implements Storage." This is a **trait bound** — it constrains the generic type parameter. The compiler generates specialized code for each concrete type you use, so there is no runtime overhead. You get the flexibility of polymorphism with the performance of monomorphism.

### Why this matters for databases

The `Storage` trait is the seam in your architecture. Everything above it (the SQL engine, the query planner, the client protocol) calls `set()`, `get()`, `delete()`, and `scan()` without knowing whether the data lives in a `BTreeMap`, an on-disk B-tree, or a distributed consensus log. When you build persistent storage in Chapter 3, you will implement the same trait for a different backend. The database will not change — only the engine underneath.

> **Coming from JS/Python/Go?**
>
> **JavaScript:** There is no direct equivalent. JavaScript uses duck typing — if an object has a `.get()` method, you can call it, and if it does not, you get a runtime error. Rust traits are like TypeScript interfaces, but enforced at compile time with no escape hatch. There is no `as any`.
>
> ```typescript
> // TypeScript: interface is optional, duck typing still works
> interface Storage {
>   set(key: string, value: Uint8Array): void;
>   get(key: string): Uint8Array | null;
> }
> ```
>
> **Python:** Traits are closest to `Protocol` (PEP 544) or abstract base classes (`ABC`). The difference: Python protocols are checked by mypy (optional), while Rust traits are checked by the compiler (mandatory). You cannot skip trait checking in Rust.
>
> ```python
> # Python: ABC, but enforcement is optional
> from abc import ABC, abstractmethod
>
> class Storage(ABC):
>     @abstractmethod
>     def set(self, key: str, value: bytes) -> None: ...
>
>     @abstractmethod
>     def get(self, key: str) -> bytes | None: ...
> ```
>
> **Go:** Go interfaces are the closest analog. Both are satisfied implicitly (Go) or explicitly (Rust). The key difference: Rust traits support generics and static dispatch, while Go interfaces always use dynamic dispatch (interface values carry a vtable pointer). Rust gives you the choice.
>
> ```go
> // Go: implicit interface satisfaction
> type Storage interface {
>     Set(key string, value []byte) error
>     Get(key string) ([]byte, error)
> }
> ```
>
> In all three languages, you can define a "shape" that types must match. Rust's version catches violations earlier (compile time), runs faster (static dispatch by default), and cannot be bypassed.

---
