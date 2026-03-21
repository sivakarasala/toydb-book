/// Chapter 4: Serialization
/// Exercise: Use serde + bincode for encoding/decoding database values.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use serde::{Deserialize, Serialize};

// ── Value ───────────────────────────────────────────────────────────

/// TODO: Add #[derive(Serialize, Deserialize)] alongside Debug, Clone, PartialEq
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

// ── Row ─────────────────────────────────────────────────────────────

/// A database row: an ordered list of named columns.
/// TODO: Add Serialize, Deserialize derives
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub columns: Vec<String>,
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(columns: Vec<String>, values: Vec<Value>) -> Self {
        Row { columns, values }
    }

    /// Get a value by column name.
    pub fn get(&self, column: &str) -> Option<&Value> {
        // TODO: Find the column index, return the corresponding value
        todo!("Implement get")
    }
}

// ── Encoding/Decoding ───────────────────────────────────────────────

/// Encode a Value to bytes using bincode.
pub fn encode_value(value: &Value) -> Vec<u8> {
    // TODO: Use bincode::serialize
    todo!("Implement encode_value")
}

/// Decode a Value from bytes using bincode.
pub fn decode_value(bytes: &[u8]) -> Value {
    // TODO: Use bincode::deserialize
    todo!("Implement decode_value")
}

/// Encode a Row to bytes.
pub fn encode_row(row: &Row) -> Vec<u8> {
    // TODO: Use bincode::serialize
    todo!("Implement encode_row")
}

/// Decode a Row from bytes.
pub fn decode_row(bytes: &[u8]) -> Row {
    // TODO: Use bincode::deserialize
    todo!("Implement decode_row")
}

fn main() {
    println!("=== Chapter 4: Serialization ===");
    println!("Exercise: Implement encode/decode with serde + bincode.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_round_trip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
        ];
        for v in values {
            let bytes = encode_value(&v);
            let decoded = decode_value(&bytes);
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn test_row_round_trip() {
        let row = Row::new(
            vec!["id".to_string(), "name".to_string(), "active".to_string()],
            vec![
                Value::Int(1),
                Value::String("Alice".to_string()),
                Value::Bool(true),
            ],
        );
        let bytes = encode_row(&row);
        let decoded = decode_row(&bytes);
        assert_eq!(row, decoded);
    }

    #[test]
    fn test_row_get() {
        let row = Row::new(
            vec!["x".to_string(), "y".to_string()],
            vec![Value::Int(10), Value::Int(20)],
        );
        assert_eq!(row.get("x"), Some(&Value::Int(10)));
        assert_eq!(row.get("y"), Some(&Value::Int(20)));
        assert_eq!(row.get("z"), None);
    }

    #[test]
    fn test_binary_is_compact() {
        let v = Value::Int(42);
        let bytes = encode_value(&v);
        // bincode should be much smaller than JSON
        assert!(bytes.len() < 20);
    }
}
