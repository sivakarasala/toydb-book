## Rust Gym

Three short exercises to strengthen your trait object and dynamic dispatch skills. All use `std` only.

### Gym 1: Heterogeneous Animal Collection

Create a `Vec<Box<dyn Animal>>` with different animal types. Each type implements `speak()` differently.

```rust
// Goal: define a trait and multiple implementors, store them in a Vec,
// iterate and call the trait method.

trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

// Your task: define Dog, Cat, and Duck structs that implement Animal.
// Create a Vec<Box<dyn Animal>> with one of each.
// Print each animal's name and speech.

// Expected output:
// Rex says: Woof!
// Whiskers says: Meow!
// Donald says: Quack!
```

<details>
<summary>Hint</summary>

Each struct needs a `name` field (a `String`). Implement `Animal` for each one. Use `Box::new(Dog { name: "Rex".to_string() })` to create a `Box<dyn Animal>`.

</details>

<details>
<summary>Solution</summary>

```rust
trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

struct Dog { name: String }
struct Cat { name: String }
struct Duck { name: String }

impl Animal for Dog {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Woof!".to_string() }
}

impl Animal for Cat {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Meow!".to_string() }
}

impl Animal for Duck {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Quack!".to_string() }
}

fn main() {
    let animals: Vec<Box<dyn Animal>> = vec![
        Box::new(Dog { name: "Rex".to_string() }),
        Box::new(Cat { name: "Whiskers".to_string() }),
        Box::new(Duck { name: "Donald".to_string() }),
    ];

    for animal in &animals {
        println!("{} says: {}", animal.name(), animal.speak());
    }
}
```

Output:

```
Rex says: Woof!
Whiskers says: Meow!
Donald says: Quack!
```

Each element in `animals` is a different concrete type, but the `Vec` does not care. It stores `Box<dyn Animal>` — a pointer plus a vtable. When we call `animal.speak()`, Rust looks up the `speak` function pointer in the vtable and calls it. The call is resolved at runtime, not compile time.

</details>

### Gym 2: Plugin System

Build a simple plugin system where plugins register themselves and the system runs them in order.

```rust
// Goal: a PluginHost that holds Box<dyn Plugin> values
// and calls execute() on each one.

trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

// Your task:
// 1. Implement UppercasePlugin (converts input to uppercase)
// 2. Implement ReversePlugin (reverses the input string)
// 3. Implement PrefixPlugin { prefix: String } (prepends a prefix)
// 4. Build a PluginHost that stores Vec<Box<dyn Plugin>>
// 5. PluginHost::run() applies all plugins in sequence,
//    passing each plugin's output as the next plugin's input.

// Expected output for input "hello" with plugins [Uppercase, Reverse, Prefix(">> ")]:
// [UppercasePlugin] "hello" -> "HELLO"
// [ReversePlugin] "HELLO" -> "OLLEH"
// [PrefixPlugin] "OLLEH" -> ">> OLLEH"
// Final result: >> OLLEH
```

<details>
<summary>Hint</summary>

`PluginHost::run` should loop over `&self.plugins`, calling `plugin.execute(current)` and updating `current` with the result each time. The tricky part is the `PrefixPlugin` — it needs to own its prefix string, so its struct has a `prefix: String` field.

</details>

<details>
<summary>Solution</summary>

```rust
trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

struct UppercasePlugin;
impl Plugin for UppercasePlugin {
    fn name(&self) -> &str { "UppercasePlugin" }
    fn execute(&self, input: &str) -> String { input.to_uppercase() }
}

struct ReversePlugin;
impl Plugin for ReversePlugin {
    fn name(&self) -> &str { "ReversePlugin" }
    fn execute(&self, input: &str) -> String {
        input.chars().rev().collect()
    }
}

struct PrefixPlugin { prefix: String }
impl Plugin for PrefixPlugin {
    fn name(&self) -> &str { "PrefixPlugin" }
    fn execute(&self, input: &str) -> String {
        format!("{}{}", self.prefix, input)
    }
}

struct PluginHost {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginHost {
    fn new() -> Self {
        PluginHost { plugins: Vec::new() }
    }

    fn add(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    fn run(&self, input: &str) -> String {
        let mut current = input.to_string();
        for plugin in &self.plugins {
            let next = plugin.execute(&current);
            println!("[{}] {:?} -> {:?}", plugin.name(), current, next);
            current = next;
        }
        current
    }
}

fn main() {
    let mut host = PluginHost::new();
    host.add(Box::new(UppercasePlugin));
    host.add(Box::new(ReversePlugin));
    host.add(Box::new(PrefixPlugin { prefix: ">> ".to_string() }));

    let result = host.run("hello");
    println!("Final result: {}", result);
}
```

Output:

```
[UppercasePlugin] "hello" -> "HELLO"
[ReversePlugin] "HELLO" -> "OLLEH"
[PrefixPlugin] "OLLEH" -> ">> OLLEH"
Final result: >> OLLEH
```

This is exactly the pattern our optimizer uses. The `PluginHost` is the `Optimizer`, each `Plugin` is an `OptimizerRule`, and `run()` is `optimize()`. The only difference is that our optimizer transforms a `Plan` tree instead of a `String`.

</details>

### Gym 3: Static vs Dynamic Dispatch Comparison

Write the same function using both `impl Trait` and `dyn Trait`, and observe the differences.

```rust
// Goal: understand the tradeoff between static and dynamic dispatch.

trait Formatter {
    fn format(&self, value: f64) -> String;
}

struct DecimalFormatter { places: usize }
struct PercentFormatter;
struct CurrencyFormatter { symbol: char }

// Your task:
// 1. Implement Formatter for all three types.
// 2. Write format_static(formatter: &impl Formatter, value: f64) -> String
// 3. Write format_dynamic(formatter: &dyn Formatter, value: f64) -> String
// 4. Write format_all(formatters: &[Box<dyn Formatter>], value: f64)
//    that prints the formatted value for each formatter.
// 5. Try writing format_all with impl Trait — explain why it does not compile.

// Expected output for value 0.1567:
// Decimal(2): 0.16
// Percent: 15.67%
// Currency($): $0.16
```

<details>
<summary>Hint</summary>

`format_static` uses monomorphization — the compiler generates one version per concrete type. `format_dynamic` uses a vtable. `format_all` must use `dyn Trait` because the `Vec` contains different types. If you try `fn format_all(formatters: &[impl Formatter], value: f64)`, it means "a slice where all elements are the same (unknown) type," which is not what we want.

</details>

<details>
<summary>Solution</summary>

```rust
trait Formatter {
    fn format(&self, value: f64) -> String;
}

struct DecimalFormatter { places: usize }
impl Formatter for DecimalFormatter {
    fn format(&self, value: f64) -> String {
        format!("{:.prec$}", value, prec = self.places)
    }
}

struct PercentFormatter;
impl Formatter for PercentFormatter {
    fn format(&self, value: f64) -> String {
        format!("{:.2}%", value * 100.0)
    }
}

struct CurrencyFormatter { symbol: char }
impl Formatter for CurrencyFormatter {
    fn format(&self, value: f64) -> String {
        format!("{}{:.2}", self.symbol, value)
    }
}

// Static dispatch: compiler generates specialized versions
fn format_static(formatter: &impl Formatter, value: f64) -> String {
    formatter.format(value)
}

// Dynamic dispatch: runtime vtable lookup
fn format_dynamic(formatter: &dyn Formatter, value: f64) -> String {
    formatter.format(value)
}

// Must use dyn Trait — elements are different types
fn format_all(formatters: &[Box<dyn Formatter>], value: f64) {
    for formatter in formatters {
        println!("  {}", formatter.format(value));
    }
}

// This does NOT compile:
// fn format_all_static(formatters: &[impl Formatter], value: f64) {
//     // Error: `impl Trait` means "one specific type that implements Formatter"
//     // All elements must be the same type — defeats the purpose.
//     // This is a slice of T where T: Formatter, not a slice of
//     // "anything that implements Formatter."
// }

fn main() {
    let value = 0.1567;

    // Static dispatch calls — each resolves at compile time
    let dec = DecimalFormatter { places: 2 };
    let pct = PercentFormatter;
    let cur = CurrencyFormatter { symbol: '$' };

    println!("Static dispatch:");
    println!("  Decimal(2): {}", format_static(&dec, value));
    println!("  Percent: {}", format_static(&pct, value));
    println!("  Currency($): {}", format_static(&cur, value));

    // Dynamic dispatch — same results, resolved at runtime
    let formatters: Vec<Box<dyn Formatter>> = vec![
        Box::new(DecimalFormatter { places: 2 }),
        Box::new(PercentFormatter),
        Box::new(CurrencyFormatter { symbol: '$' }),
    ];

    println!("\nDynamic dispatch:");
    format_all(&formatters, value);
}
```

Output:

```
Static dispatch:
  Decimal(2): 0.16
  Percent: 15.67%
  Currency($): $0.16

Dynamic dispatch:
  0.16
  15.67%
  $0.16
```

The results are identical. The difference is in how the compiler handles each call:

- `format_static(&dec, value)` — the compiler knows `dec` is `DecimalFormatter` and generates a direct function call. It can inline `DecimalFormatter::format` at the call site.
- `format_dynamic(&dec as &dyn Formatter, value)` — the compiler generates a vtable lookup. It loads the function pointer from the vtable and calls through it. Inlining is not possible.

For our optimizer rules, the vtable overhead is negligible. Each `optimize()` call does substantial work (walking an entire plan tree). The cost of one pointer dereference per rule is invisible. Use `dyn Trait` when you need heterogeneous collections; use `impl Trait` when you need maximum performance on hot paths.

</details>

---
