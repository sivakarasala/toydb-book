/// toydb — Interactive SQL REPL
///
/// This is the culmination of all 18 chapters.
/// Run: cargo run
/// Or with persistence: cargo run -- --wal /tmp/toydb.wal

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut db = if args.len() >= 3 && args[1] == "--wal" {
        let path = std::path::Path::new(&args[2]);
        match toydb::Database::with_wal(path) {
            Ok(db) => {
                let (term, commit) = db.raft_status();
                if commit > 0 {
                    println!("Recovered {} entries from WAL (term {})", commit, term);
                }
                db
            }
            Err(e) => {
                eprintln!("Failed to open WAL: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        toydb::Database::new()
    };

    println!("toydb — A toy SQL database built in Rust");
    println!("Type SQL statements, or .help for commands.\n");

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print!("toydb> ");
        io::stdout().flush().unwrap();
        input.clear();

        if stdin.read_line(&mut input).unwrap() == 0 {
            println!();
            break; // EOF
        }

        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }

        // Meta commands
        match trimmed {
            ".quit" | ".exit" => break,
            ".help" => {
                println!("Commands:");
                println!("  .help          Show this help");
                println!("  .quit          Exit the REPL");
                println!("  .status        Show Raft status");
                println!();
                println!("SQL:");
                println!("  CREATE TABLE name (col TYPE, ...)");
                println!("  INSERT INTO name VALUES (val, ...)");
                println!("  SELECT cols FROM name [WHERE ...] [ORDER BY col] [LIMIT n]");
                println!("  DELETE FROM name [WHERE ...]");
                println!("  DROP TABLE name");
                println!();
                println!("Types: INT, TEXT, BOOL");
                continue;
            }
            ".status" => {
                let (term, commit) = db.raft_status();
                println!("Raft term: {}, committed: {}", term, commit);
                continue;
            }
            _ => {}
        }

        match db.execute(trimmed) {
            Ok(result) => println!("{}", result),
            Err(e) => println!("Error: {}", e),
        }
    }
}
