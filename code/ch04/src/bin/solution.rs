/// Chapter 4: Serialization — SOLUTION

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub columns: Vec<String>,
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(columns: Vec<String>, values: Vec<Value>) -> Self {
        Row { columns, values }
    }

    pub fn get(&self, column: &str) -> Option<&Value> {
        self.columns
            .iter()
            .position(|c| c == column)
            .map(|i| &self.values[i])
    }
}

pub fn encode_value(value: &Value) -> Vec<u8> {
    bincode::serialize(value).expect("serialize failed")
}

pub fn decode_value(bytes: &[u8]) -> Value {
    bincode::deserialize(bytes).expect("deserialize failed")
}

pub fn encode_row(row: &Row) -> Vec<u8> {
    bincode::serialize(row).expect("serialize failed")
}

pub fn decode_row(bytes: &[u8]) -> Row {
    bincode::deserialize(bytes).expect("deserialize failed")
}

fn main() {
    println!("=== Chapter 4: Serialization — Solution ===");
    let row = Row::new(
        vec!["id".into(), "name".into(), "active".into()],
        vec![
            Value::Int(1),
            Value::String("ToyDB".into()),
            Value::Bool(true),
        ],
    );

    let bytes = encode_row(&row);
    println!("Row: {:?}", row);
    println!("Encoded: {} bytes", bytes.len());
    let decoded = decode_row(&bytes);
    println!("Decoded: {:?}", decoded);
    assert_eq!(row, decoded);
    println!("Round-trip: OK!");
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
            vec!["id".into(), "name".into(), "active".into()],
            vec![
                Value::Int(1),
                Value::String("Alice".into()),
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
            vec!["x".into(), "y".into()],
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
        assert!(bytes.len() < 20);
    }
}
