/// Unified error type for toydb (Ch3, Ch11)

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Plan error: {0}")]
    Plan(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Table '{0}' not found")]
    TableNotFound(String),

    #[error("Table '{0}' already exists")]
    TableExists(String),

    #[error("Column '{0}' not found")]
    ColumnNotFound(String),

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
