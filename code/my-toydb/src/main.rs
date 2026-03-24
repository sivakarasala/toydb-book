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
use std::io::{self, Write};

struct Database {
    data: HashMap<String, String>,
}

impl Database {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn list(&self) -> Vec<(&String, &String)> {
        self.data.iter().collect()
    }
}

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
                let value = parts[2].to_string();
                db.set(key, value);
                println!("OK");
            }
            "GET" => {
                if parts.len() < 2 {
                    println!("Usage: GET <key>");
                    continue;
                }
                match db.get(parts[1]) {
                    Some(value) => println!("{}", value),
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
                        println!("{} = {}", key, value);
                    }
                }
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
                println!("Unknown command: '{}'. Type HELP for available commands.", parts[0])
            }
        }
    }
}
