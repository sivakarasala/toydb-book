/// my-toydb — Your SQL database, built chapter by chapter.
///
/// This file starts empty. By Ch17, it becomes a full REPL.
///
/// Milestones:
///   After Ch 1-2:  You can store and retrieve key-value pairs
///   After Ch 6-7:  You can parse SQL into an AST
///   After Ch 10-11: You can execute SQL queries (THE BIG ONE)
///   After Ch 14-16: Your database survives crashes
///   After Ch 17:    This file becomes an interactive REPL
///
/// For now, just make sure it compiles:
///   cargo build
///
/// When you're ready for the REPL (Ch17), look at:
///   ../toydb/src/main.rs
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

// --- Value type ---

#[derive(Debug, Clone)]
enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "{}", s),
            Value::Integer(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
        }
    }
}

impl Value {
    fn type_name(&self) -> &str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
        }
    }

    fn parse(input: &str) -> Self {
        // Try boolean first
        if input.eq_ignore_ascii_case("true") {
            return Value::Boolean(true);
        }
        if input.eq_ignore_ascii_case("false") {
            return Value::Boolean(false);
        }

        // Try integer
        if let Ok(n) = input.parse::<i64>() {
            return Value::Integer(n);
        }

        // Try float
        if let Ok(n) = input.parse::<f64>() {
            return Value::Float(n);
        }

        // Default to string
        Value::String(input.to_string())
    }
}

// --- Operation statistics ---

struct OperationStats {
    gets: usize,
    sets: usize,
    deletes: usize,
}

impl OperationStats {
    fn new() -> Self {
        Self {
            gets: 0,
            sets: 0,
            deletes: 0,
        }
    }

    fn total(&self) -> usize {
        self.gets + self.sets + self.deletes
    }
}

// --- Database ---

struct Database {
    data: HashMap<String, Value>,
    stats: OperationStats,
}

impl Database {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
            stats: OperationStats::new(),
        }
    }

    fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
        self.stats.sets += 1;
    }

    fn get(&mut self, key: &str) -> Option<&Value> {
        self.stats.gets += 1;
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.stats.deletes += 1;
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &Value)> {
        self.data.iter().collect()
    }

    fn stats(&self) -> &OperationStats {
        &self.stats
    }

    fn entry_count(&self) -> usize {
        self.data.len()
    }
}

// --- REPL ---

fn main() {
    let mut db = Database::new();

    println!("toydb v0.1.0");
    println!("Type HELP for available commands.\n");

    loop {
        print!("toydb> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF (Ctrl+D)
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        let command = parts[0].to_uppercase();

        match command.as_str() {
            "SET" => {
                if parts.len() < 3 {
                    println!("Usage: SET <key> <value>");
                    continue;
                }
                let key = parts[1].to_string();
                let value = Value::parse(parts[2]);
                db.set(key, value);
                println!("OK");
            }
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                let result = db
                    .get(parts[1])
                    .map(|v| format!("({}) {}", v.type_name(), v));
                match result {
                    Some(display) => println!("{}", display),
                    None => println!("(nil)"),
                }
            }
            "DELETE" => {
                if parts.len() < 2 {
                    println!("Usage: DELETE <key>");
                    continue;
                }
                if db.delete(parts[1]) {
                    println!("OK");
                } else {
                    println!("(nil)");
                }
            }
            "LIST" => {
                let entries = db.list();
                if entries.is_empty() {
                    println!("(empty)");
                } else {
                    for (key, value) in entries {
                        println!("{} = ({}) {}", key, value.type_name(), value);
                    }
                }
            }
            "STATS" => {
                let stats = db.stats();
                println!("--- toydb statistics ---");
                println!("  Keys stored:        {}", db.entry_count());
                println!("  SET operations:     {}", stats.sets);
                println!("  GET operations:     {}", stats.gets);
                println!("  DEL operations:     {}", stats.deletes);
                println!("  Total ops:          {}", stats.total());
                println!("------------------------");
            }
            "HELP" => {
                println!("Commands:");
                println!(" SET <key> <value> - store a key-value pair");
                println!(" GET <key>         - retrieve a value by key");
                println!(" DELETE <key>      - remove a key-value pair");
                println!(" LIST              - show all key-value pairs");
                println!(" HELP              - show this message");
                println!(" EXIT              - quit");
            }
            "EXIT" | "QUIT" => {
                println!("Bye!");
                break;
            }
            _ => {
                println!(
                    "Unknown command: '{}'. Type HELP for available commands.",
                    parts[0]
                );
            }
        }
    }
}
