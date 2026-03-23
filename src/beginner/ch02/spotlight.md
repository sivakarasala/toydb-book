## Spotlight: Traits & Generics

Every chapter has one spotlight concept. This chapter's spotlight is **traits and generics** — the way Rust defines shared behavior and writes code that works across multiple types.

### What is a trait?

Imagine you are designing a USB port. You do not care what device gets plugged in — a keyboard, a mouse, a phone charger, a thumb drive. All you care about is that the device fits the port and follows the USB protocol. The port defines a set of rules (how to send data, how to receive power), and any device that follows those rules can be plugged in.

A Rust **trait** is like a USB port specification. It defines a set of methods that a type must provide. Any type that provides those methods "implements" the trait and can be used wherever the trait is expected.

```rust
trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>);
    fn get(&self, key: &str) -> Option<Vec<u8>>;
}
```

This trait says: "Any type that calls itself a `Storage` must have a `set` method and a `get` method with exactly these signatures."

> **Analogy: Trait = Contract**
>
> A trait is a contract. It is a promise that a type makes: "I guarantee I can do these things." The trait does not say *how* the work gets done — it just says *what* must be available. A `MemoryStorage` might keep data in a `BTreeMap`. A `DiskStorage` might write to a file. Both fulfill the same contract.

### Defining a trait

Let's look at the syntax more carefully:

```rust
trait Greet {
    fn hello(&self) -> String;
}
```

- `trait Greet` — declares a new trait named `Greet`.
- `fn hello(&self) -> String;` — declares a method signature. Note the semicolon at the end instead of a body `{ ... }`. This means "the trait declares this method but does not provide a default implementation." Each type must write its own.

### Implementing a trait

To make a type fulfill the trait's contract, you write an `impl TraitName for TypeName` block:

```rust
struct Person {
    name: String,
}

impl Greet for Person {
    fn hello(&self) -> String {
        format!("Hello, my name is {}!", self.name)
    }
}

struct Robot {
    id: u32,
}

impl Greet for Robot {
    fn hello(&self) -> String {
        format!("BEEP BOOP. I AM UNIT {}.", self.id)
    }
}
```

Now both `Person` and `Robot` implement the `Greet` trait. They both have a `hello` method, but each one does something different.

> **What just happened?**
>
> We defined one trait (`Greet`) and two types (`Person`, `Robot`). Each type implements the trait differently. The trait is the contract; the `impl` blocks are how each type fulfills that contract.
>
> The `format!` macro works like `println!` but instead of printing to the screen, it returns a `String`. We use it here because `hello` needs to return a `String`, not print one.

### Why traits matter

Without traits, you would write separate functions for each storage type:

```rust
fn save_to_memory(store: &mut MemoryStorage, key: &str, value: &[u8]) { ... }
fn save_to_disk(store: &mut DiskStorage, key: &str, value: &[u8]) { ... }
```

And every part of your code that uses storage would need to know which kind it is working with. If you add a third storage type, you rewrite everything.

With traits, you write your code once against the trait:

```rust
fn save<S: Storage>(store: &mut S, key: &str, value: &[u8]) {
    store.set(key.to_string(), value.to_vec());
}
```

This function works with `MemoryStorage`, `DiskStorage`, `NetworkStorage`, or any future type that implements `Storage`. The `<S: Storage>` part is what makes this possible, and it is called a **generic with a trait bound**. Let's understand that next.

### What are generics?

Generics let you write code that works with multiple types. You have already seen one: `HashMap<String, Value>`. The `<String, Value>` part says "this HashMap uses `String` for keys and `Value` for values." If you change it to `HashMap<String, i64>`, the same HashMap code works with different types.

You can write your own generic functions and structs:

```rust
fn first_element<T>(list: &[T]) -> &T {
    &list[0]
}
```

The `<T>` is a **type parameter**. It is a placeholder that says "this function works with any type." When you call `first_element(&[1, 2, 3])`, `T` becomes `i32`. When you call `first_element(&["a", "b"])`, `T` becomes `&str`.

> **Analogy: Generics = Fill in the blank**
>
> Think of `<T>` like a blank on a form. The form says "Name of pet: ___". You can write anything in the blank — "Rufus", "Whiskers", "Bubbles" — and the form still makes sense. `T` is the blank. The actual type fills it in when the code is used.

### Trait bounds: generics with requirements

Plain generics accept *any* type. But sometimes you need the type to have certain abilities. That is where trait bounds come in:

```rust
fn print_greeting<G: Greet>(greeter: &G) {
    println!("{}", greeter.hello());
}
```

The `<G: Greet>` syntax says: "G can be any type, **as long as it implements Greet.**" If you try to pass a type that does not implement `Greet`, the compiler will refuse with a clear error message.

This is how we will build our generic database: `Database<S: Storage>` — a database that works with any storage engine, as long as that engine implements the `Storage` trait.

### Common mistakes with traits

**Mistake: Forgetting to implement all required methods**

```rust
trait Storage {
    fn set(&mut self, key: String, value: Vec<u8>);
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn delete(&mut self, key: &str);
}

struct MyStore;

impl Storage for MyStore {
    fn set(&mut self, key: String, value: Vec<u8>) { }
    fn get(&self, key: &str) -> Option<Vec<u8>> { None }
    // Forgot delete!
}
// ERROR: not all trait items implemented, missing: `delete`
```

The compiler catches this. You cannot partially implement a trait.

**Mistake: Wrong method signature**

```rust
impl Storage for MyStore {
    fn set(&mut self, key: &str, value: Vec<u8>) { }  // Wrong! key should be String
}
// ERROR: method `set` has an incompatible type for trait
```

The signatures must match exactly. The compiler will tell you what is wrong.

---
