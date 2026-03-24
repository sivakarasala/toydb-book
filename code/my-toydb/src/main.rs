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

    db.set("name".to_string(), "toydb".to_string());
    db.set("version".to_string(), "0.1.0".to_string());
    db.set("language".to_string(), "Rust".to_string());

    println!("GET name = {:?}", db.get("name"));
    println!("GET missing = {:?}", db.get("missing"));

    println!("\nAll keys:");
    for (key, value) in db.list() {
        println!(" {} = {}", key, value);
    }

    let deleted = db.delete("version");
    println!("\nDELETE version: {}", deleted);
    println!("GET version = {:?}", db.get("version"));

}
