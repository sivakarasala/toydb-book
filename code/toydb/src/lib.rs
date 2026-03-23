/// toydb — A toy distributed SQL database
///
/// Built chapter by chapter in "Learn Rust by Building a Database".
///
/// Layers:
///   Storage  (Ch1-3)  → trait Storage + MemoryStorage
///   SQL      (Ch6-11) → Lexer → Parser → Planner → Executor
///   Raft     (Ch14-16) → Log replication + WAL persistence
///   Server   (Ch12-13, Ch17) → ties it all together

pub mod error;
pub mod storage;
pub mod sql;
pub mod raft;

use error::Result;
use sql::executor::ExecResult;
use storage::Storage;

/// The toydb database engine — wires all layers together (Ch17).
pub struct Database {
    storage: Box<dyn Storage>,
    raft: raft::RaftLog,
}

impl Database {
    /// Create an in-memory database (no persistence).
    pub fn new() -> Self {
        Database {
            storage: Box::new(storage::MemoryStorage::new()),
            raft: raft::RaftLog::new(),
        }
    }

    /// Create a database with WAL persistence.
    pub fn with_wal(wal_path: &std::path::Path) -> Result<Self> {
        let raft = raft::RaftLog::with_wal(wal_path)?;

        // Replay committed commands
        let mut storage = storage::MemoryStorage::new();
        for cmd in raft.committed_commands() {
            // Silently replay — errors during replay are expected for DDL
            let _ = execute_sql(&cmd, &mut storage);
        }

        Ok(Database {
            storage: Box::new(storage),
            raft,
        })
    }

    /// Execute a SQL statement. Returns a displayable result.
    pub fn execute(&mut self, sql: &str) -> Result<ExecResult> {
        // Log to Raft first (for durability)
        self.raft.propose(sql.to_string())?;
        // Then execute against storage
        execute_sql(sql, self.storage.as_mut())
    }

    pub fn raft_status(&self) -> (u64, u64) {
        (self.raft.term(), self.raft.commit_index())
    }
}

fn execute_sql(sql: &str, storage: &mut dyn Storage) -> Result<ExecResult> {
    let stmt = sql::parser::parse(sql)?;
    let plan = sql::planner::plan(stmt)?;
    sql::executor::execute(plan, storage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end() {
        let mut db = Database::new();
        db.execute("CREATE TABLE users (id INT, name TEXT, age INT)").unwrap();
        db.execute("INSERT INTO users VALUES (1, 'Alice', 30)").unwrap();
        db.execute("INSERT INTO users VALUES (2, 'Bob', 25)").unwrap();
        db.execute("INSERT INTO users VALUES (3, 'Charlie', 35)").unwrap();

        let result = db.execute("SELECT name, age FROM users WHERE age > 28 ORDER BY age DESC").unwrap();
        match result {
            ExecResult::Rows { columns, rows } => {
                assert_eq!(columns, vec!["name", "age"]);
                assert_eq!(rows.len(), 2); // Alice (30) and Charlie (35)
            }
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_wal_recovery() {
        let path = std::env::temp_dir().join("toydb_e2e_wal_test");
        let _ = std::fs::remove_file(&path);

        {
            let mut db = Database::with_wal(&path).unwrap();
            db.execute("CREATE TABLE t (x INT)").unwrap();
            db.execute("INSERT INTO t VALUES (42)").unwrap();
        }

        // Recover from WAL
        let mut db = Database::with_wal(&path).unwrap();
        let result = db.execute("SELECT * FROM t").unwrap();
        match result {
            ExecResult::Rows { rows, .. } => {
                assert_eq!(rows.len(), 1);
            }
            _ => panic!("Expected Rows"),
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_multiple_tables() {
        let mut db = Database::new();
        db.execute("CREATE TABLE users (id INT, name TEXT)").unwrap();
        db.execute("CREATE TABLE items (id INT, label TEXT)").unwrap();
        db.execute("INSERT INTO users VALUES (1, 'Alice')").unwrap();
        db.execute("INSERT INTO items VALUES (1, 'Widget')").unwrap();

        let r1 = db.execute("SELECT COUNT(*) FROM users").unwrap();
        let r2 = db.execute("SELECT COUNT(*) FROM items").unwrap();
        assert!(matches!(r1, ExecResult::Count(1)));
        assert!(matches!(r2, ExecResult::Count(1)));
    }
}
