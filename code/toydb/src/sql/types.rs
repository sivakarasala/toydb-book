/// SQL type system — shared across all SQL modules.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Int,
    Text,
    Bool,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Int => write!(f, "INT"),
            DataType::Text => write!(f, "TEXT"),
            DataType::Bool => write!(f, "BOOL"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Text(String),
    Bool(bool),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Text(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "NULL"),
        }
    }
}

impl Value {
    pub fn matches_type(&self, dt: &DataType) -> bool {
        matches!(
            (self, dt),
            (Value::Int(_), DataType::Int)
                | (Value::Text(_), DataType::Text)
                | (Value::Bool(_), DataType::Bool)
                | (Value::Null, _)
        )
    }
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<Column>,
}

impl TableSchema {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

/// A row is a vector of values.
pub type Row = Vec<Value>;
