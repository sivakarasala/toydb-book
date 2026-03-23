## Spotlight: Variables, Types & HashMap

Every chapter in this book has one **spotlight concept** — the Rust idea we dig into deeply. This chapter's spotlight is **variables, types, and HashMap** — the foundation of every data structure you will build.

### What is a variable?

A variable is a name for a piece of data. Think of it like a labeled box. You put something in the box, and later you can look at what is inside by reading the label.

In Rust, you create a variable with the `let` keyword:

```rust
let name = "toydb";
```

This creates a variable called `name` and puts the text `"toydb"` inside it. From now on, whenever you write `name` in your code, Rust knows you mean `"toydb"`.

### Immutability: variables that don't change

Here is something that surprises most beginners: **in Rust, variables cannot be changed by default.** Once you put something in the box, the box is sealed.

```rust
let name = "toydb";
// name = "mydb";    // ERROR! Cannot change an immutable variable
```

If you try to change `name`, the compiler (the program that turns your code into a runnable application) will refuse to compile your code. It will show an error message explaining what went wrong.

Why would a language do this? Because bugs often come from things changing when you did not expect them to. If a variable cannot change, you can look at where it was created and know *exactly* what it contains. No surprises.

### Mutable variables: opting in to change

Sometimes you *need* a variable to change. For that, you add the keyword `mut` (short for "mutable"):

```rust
let mut count = 0;
count = count + 1;   // OK — count is mutable
count += 1;          // shorthand for the same thing
println!("count is {}", count);  // prints: count is 2
```

The `mut` keyword is your way of saying "I intend to change this later." It makes your intention clear to anyone reading the code — including your future self.

> **What just happened?**
>
> We created a variable `count` with the value `0` and marked it as `mut` (mutable). Then we changed its value twice. The `+=` operator is shorthand for "add to the current value." Finally, `println!` prints text to the terminal. The `{}` is a placeholder that gets replaced with the value of `count`.

### Types: what kind of data is in the box?

Every variable in Rust has a **type** — it describes what kind of data the variable holds. Rust has several built-in types:

```rust
let age = 30;              // i32 — a whole number (integer), 32 bits
let pi = 3.14159;          // f64 — a decimal number (floating-point), 64 bits
let name = "toydb";        // &str — a piece of text (a "string slice")
let active = true;         // bool — true or false
```

You might notice something: we did not write the types ourselves. Rust figured them out automatically. This is called **type inference** — the compiler looks at the value and determines the type.

But you *can* write the type explicitly if you want to be clear:

```rust
let age: i32 = 30;
let pi: f64 = 3.14159;
let active: bool = true;
```

Both styles are valid. As you get comfortable with Rust, you will let the compiler infer types most of the time and only write them explicitly when it helps readability.

### Integer types: choosing the right size

Rust has many integer types, and each one has a specific size:

| Type | Size | Range | When to use |
|------|------|-------|-------------|
| `i8` | 8 bits | -128 to 127 | Tiny numbers |
| `i16` | 16 bits | -32,768 to 32,767 | Small numbers |
| `i32` | 32 bits | about -2 billion to 2 billion | **Default — most numbers** |
| `i64` | 64 bits | about -9.2 quintillion to 9.2 quintillion | Big numbers (database IDs) |
| `u8` | 8 bits | 0 to 255 | Bytes, small counts |
| `u32` | 32 bits | 0 to about 4 billion | Counts that are never negative |
| `u64` | 64 bits | 0 to about 18.4 quintillion | File sizes, big counts |
| `usize` | depends on computer | 0 to a lot | Indexing, counting items in a list |

The `i` prefix means "signed" (can be negative). The `u` prefix means "unsigned" (zero or positive only). The number is how many bits of memory it uses.

For our database, we will mostly use `i64` for integer values (big enough for any reasonable number) and `f64` for decimal values.

> **Analogy: Types are like container shapes**
>
> Think of types like different shaped containers. A `bool` is a tiny box that can only hold "yes" or "no." An `i32` is a medium box that holds whole numbers. A `String` is a stretchy bag that holds text of any length. You cannot put a number into a text container or vice versa — the shapes do not fit. This protects you from mistakes like trying to add a person's name to their age.

### Two kinds of text: `String` vs `&str`

Rust has two main text types, and this confuses almost everyone at first. Let's clear it up with an analogy.

Imagine a book on a library shelf:

- **`&str`** (pronounced "string slice") is like *reading the book at the library*. You can look at the words, but you do not own the book. You cannot add pages or cross out words. When you write `"hello"` in your code, that is a `&str`.

- **`String`** is like *buying your own copy of the book*. You own it. You can highlight, add sticky notes, tear out pages. You create one with `String::from("hello")` or `"hello".to_string()`.

```rust
let greeting: &str = "Hello";                        // borrowed — read-only
let owned_greeting: String = String::from("Hello");  // owned — you control it
```

Why does this matter? When you store data in a collection (like a `HashMap`, which we will learn about shortly), the collection needs to *own* the data. You cannot hand it a `&str` because that is just a reference to data owned by someone else. The collection needs a `String` — its own copy.

```rust
let key: &str = "name";                         // just looking at it
let owned_key: String = key.to_string();        // made my own copy
let also_owned: String = String::from("name");  // another way to make a copy
```

> **What just happened?**
>
> We saw that `&str` is a lightweight reference to text (like pointing at a word in someone else's book), while `String` is owned text (like having your own book). The `.to_string()` method and `String::from()` both create an owned `String` from a `&str`. You will use both constantly in Rust.

### Common mistakes with strings

**Mistake 1: Trying to change a `&str`**

```rust
let greeting = "hello";
// greeting.push_str(" world");  // ERROR: &str is read-only
```

Fix: use a `String` instead:

```rust
let mut greeting = String::from("hello");
greeting.push_str(" world");  // OK — String is owned and mutable
```

**Mistake 2: Confusing `&str` and `String` in function parameters**

```rust
fn greet(name: String) {
    println!("Hello, {}!", name);
}

// greet("Alice");  // ERROR: "Alice" is a &str, not a String
greet("Alice".to_string());  // OK — converted to String
```

A better practice is to accept `&str` in function parameters (more flexible):

```rust
fn greet(name: &str) {
    println!("Hello, {}!", name);
}

greet("Alice");                        // OK — &str
greet(&String::from("Alice"));         // OK — &String can be used as &str
```

Do not worry about memorizing all of this right now. The compiler will remind you when you get it wrong, and its error messages are helpful.

### HashMap: your first data structure

A **HashMap** is like a dictionary or a phone book. You look up a **key** (the person's name) and get back a **value** (their phone number). In programming, this is called a "key-value store" — and it is exactly what a simple database is.

```rust
use std::collections::HashMap;

let mut phonebook: HashMap<String, String> = HashMap::new();
```

Let's break this line down piece by piece:

- `use std::collections::HashMap;` — This tells Rust we want to use the `HashMap` type from the standard library. Think of it like importing a tool from a toolbox.
- `let mut phonebook` — Create a mutable variable called `phonebook` (mutable because we will add entries).
- `HashMap<String, String>` — The type. It maps `String` keys to `String` values. The `<String, String>` part tells Rust what types the keys and values will be.
- `HashMap::new()` — Create a new, empty HashMap.

Now let's use it:

```rust
// Add entries
phonebook.insert("Alice".to_string(), "555-0101".to_string());
phonebook.insert("Bob".to_string(), "555-0202".to_string());

// Look up an entry
if let Some(number) = phonebook.get("Alice") {
    println!("Alice's number is {}", number);
}

// Check if a key exists
println!("Has Charlie? {}", phonebook.contains_key("Charlie"));  // false

// Remove an entry
phonebook.remove("Bob");
```

> **What just happened?**
>
> - `insert` adds a key-value pair. Both the key and value must be `String` (owned), so we call `.to_string()` on the string literals.
> - `get` looks up a key and returns `Some(&value)` if found, or `None` if not. This is Rust's way of handling "the key might not exist" — instead of returning null (which causes bugs in many languages), Rust returns an `Option` that forces you to handle both cases.
> - `contains_key` returns `true` or `false`.
> - `remove` deletes a key-value pair.

### Option: Rust's way of saying "maybe"

In many languages, looking up a missing key gives you `null`, `undefined`, or throws an exception. Rust does something different: it returns an `Option`.

An `Option` is like a gift box that might be empty:

```rust
let result: Option<&String> = phonebook.get("Alice");
// result is either Some(&"555-0101") or None
```

You handle it with `match` or `if let`:

```rust
// Using match — handle both cases explicitly
match phonebook.get("Alice") {
    Some(number) => println!("Found: {}", number),
    None => println!("Not found"),
}

// Using if let — when you only care about the Some case
if let Some(number) = phonebook.get("Alice") {
    println!("Found: {}", number);
}
```

This might feel like extra work compared to just getting `null`. But it eliminates an entire category of bugs. In languages with `null`, you can accidentally use a null value and crash your program. In Rust, the compiler forces you to check for `None` before using the value. You literally cannot forget.

---
