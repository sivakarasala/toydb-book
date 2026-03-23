/// Query Executor (Ch10-11)
///
/// Executes plans against the storage engine.
/// Handles CREATE TABLE, INSERT, SELECT (with WHERE, ORDER BY, LIMIT), DELETE.
/// Table metadata and rows are stored in the storage engine as serialized bytes.

use crate::error::{Error, Result};
use crate::sql::parser::SelectColumn;
use crate::sql::planner::*;
use crate::sql::types::*;
use crate::storage::Storage;

/// The result of executing a SQL statement.
#[derive(Debug)]
pub enum ExecResult {
    Created(String),
    Dropped(String),
    Inserted(usize),
    Deleted(usize),
    Rows {
        columns: Vec<String>,
        rows: Vec<Row>,
    },
    Count(usize),
}

impl std::fmt::Display for ExecResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecResult::Created(name) => write!(f, "Table '{}' created.", name),
            ExecResult::Dropped(name) => write!(f, "Table '{}' dropped.", name),
            ExecResult::Inserted(n) => write!(f, "{} row(s) inserted.", n),
            ExecResult::Deleted(n) => write!(f, "{} row(s) deleted.", n),
            ExecResult::Count(n) => write!(f, "{}", n),
            ExecResult::Rows { columns, rows } => {
                // Print header
                writeln!(f, "{}", columns.join(" | "))?;
                writeln!(f, "{}", columns.iter().map(|c| "-".repeat(c.len().max(4))).collect::<Vec<_>>().join("-+-"))?;
                for row in rows {
                    let vals: Vec<String> = row.iter().map(|v| v.to_string()).collect();
                    writeln!(f, "{}", vals.join(" | "))?;
                }
                write!(f, "({} row(s))", rows.len())
            }
        }
    }
}

/// Execute a plan against a storage engine.
pub fn execute(plan: Plan, storage: &mut dyn Storage) -> Result<ExecResult> {
    match plan {
        Plan::CreateTable { name, columns } => exec_create_table(&name, &columns, storage),
        Plan::DropTable { name } => exec_drop_table(&name, storage),
        Plan::Insert { table, rows } => exec_insert(&table, rows, storage),
        Plan::Select { plan } => exec_select(plan, storage),
        Plan::Delete { table, filter } => exec_delete(&table, filter.as_ref(), storage),
    }
}

// ── Schema storage ─────────────────────────────────────────────────

const SCHEMA_PREFIX: &str = "schema:";
const ROW_PREFIX: &str = "row:";

fn schema_key(table: &str) -> String {
    format!("{}{}", SCHEMA_PREFIX, table)
}

fn row_key(table: &str, row_id: u64) -> String {
    format!("{}{}:{:020}", ROW_PREFIX, table, row_id)
}

fn row_prefix(table: &str) -> String {
    format!("{}{}:", ROW_PREFIX, table)
}

fn next_id_key(table: &str) -> String {
    format!("next_id:{}", table)
}

fn serialize_schema(schema: &TableSchema) -> Vec<u8> {
    let mut parts = vec![schema.name.clone()];
    for col in &schema.columns {
        parts.push(format!("{}:{}", col.name, col.data_type));
    }
    parts.join("\n").into_bytes()
}

fn deserialize_schema(data: &[u8]) -> Result<TableSchema> {
    let s = String::from_utf8(data.to_vec())
        .map_err(|e| Error::Storage(format!("Invalid schema data: {}", e)))?;
    let mut lines = s.lines();
    let name = lines.next().ok_or_else(|| Error::Storage("Empty schema".into()))?.to_string();
    let mut columns = Vec::new();
    for line in lines {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 { continue; }
        let data_type = match parts[1] {
            "INT" => DataType::Int,
            "TEXT" => DataType::Text,
            "BOOL" => DataType::Bool,
            other => return Err(Error::Storage(format!("Unknown type: {}", other))),
        };
        columns.push(Column { name: parts[0].to_string(), data_type });
    }
    Ok(TableSchema { name, columns })
}

fn serialize_row(row: &[Value]) -> Vec<u8> {
    let parts: Vec<String> = row.iter().map(|v| match v {
        Value::Int(n) => format!("I:{}", n),
        Value::Text(s) => format!("T:{}", s),
        Value::Bool(b) => format!("B:{}", b),
        Value::Null => "N:".to_string(),
    }).collect();
    parts.join("\t").into_bytes()
}

fn deserialize_row(data: &[u8]) -> Result<Row> {
    let s = String::from_utf8(data.to_vec())
        .map_err(|e| Error::Storage(format!("Invalid row data: {}", e)))?;
    s.split('\t').map(|part| {
        if part.starts_with("I:") { Ok(Value::Int(part[2..].parse().map_err(|e| Error::Storage(format!("{}", e)))?)) }
        else if part.starts_with("T:") { Ok(Value::Text(part[2..].to_string())) }
        else if part.starts_with("B:") { Ok(Value::Bool(part[2..] == *"true")) }
        else if part.starts_with("N:") { Ok(Value::Null) }
        else { Err(Error::Storage(format!("Unknown value: {}", part))) }
    }).collect()
}

fn load_schema(table: &str, storage: &dyn Storage) -> Result<TableSchema> {
    let data = storage.get(&schema_key(table))?
        .ok_or_else(|| Error::TableNotFound(table.to_string()))?;
    deserialize_schema(&data)
}

fn get_next_id(table: &str, storage: &mut dyn Storage) -> Result<u64> {
    let key = next_id_key(table);
    let id = match storage.get(&key)? {
        Some(data) => {
            let s = String::from_utf8(data).unwrap_or_default();
            s.parse::<u64>().unwrap_or(1)
        }
        None => 1,
    };
    storage.set(&key, (id + 1).to_string().into_bytes())?;
    Ok(id)
}

// ── Execution ──────────────────────────────────────────────────────

fn exec_create_table(name: &str, columns: &[crate::sql::parser::ColumnDef], storage: &mut dyn Storage) -> Result<ExecResult> {
    let key = schema_key(name);
    if storage.get(&key)?.is_some() {
        return Err(Error::TableExists(name.to_string()));
    }
    let schema = TableSchema {
        name: name.to_string(),
        columns: columns.iter().map(|c| Column {
            name: c.name.clone(),
            data_type: c.data_type.clone(),
        }).collect(),
    };
    storage.set(&key, serialize_schema(&schema))?;
    Ok(ExecResult::Created(name.to_string()))
}

fn exec_drop_table(name: &str, storage: &mut dyn Storage) -> Result<ExecResult> {
    let key = schema_key(name);
    if storage.get(&key)?.is_none() {
        return Err(Error::TableNotFound(name.to_string()));
    }
    storage.delete(&key)?;
    // Delete all rows
    let prefix = row_prefix(name);
    let rows = storage.scan_prefix(&prefix)?;
    for (k, _) in rows {
        storage.delete(&k)?;
    }
    storage.delete(&next_id_key(name))?;
    Ok(ExecResult::Dropped(name.to_string()))
}

fn exec_insert(table: &str, rows: Vec<Row>, storage: &mut dyn Storage) -> Result<ExecResult> {
    let schema = load_schema(table, storage)?;
    let mut count = 0;
    for row in &rows {
        if row.len() != schema.columns.len() {
            return Err(Error::Execution(format!(
                "Expected {} values, got {}", schema.columns.len(), row.len()
            )));
        }
        // Type check
        for (val, col) in row.iter().zip(&schema.columns) {
            if !val.matches_type(&col.data_type) {
                return Err(Error::TypeError(format!(
                    "Column '{}' expects {}, got {:?}", col.name, col.data_type, val
                )));
            }
        }
        let id = get_next_id(table, storage)?;
        storage.set(&row_key(table, id), serialize_row(row))?;
        count += 1;
    }
    Ok(ExecResult::Inserted(count))
}

fn exec_select(plan: SelectPlan, storage: &mut dyn Storage) -> Result<ExecResult> {
    let schema = load_schema(&plan.table, storage)?;

    // Check for COUNT(*)
    let is_count = plan.columns.iter().any(|c| matches!(c, SelectColumn::Count));

    // Scan all rows
    let prefix = row_prefix(&plan.table);
    let raw_rows = storage.scan_prefix(&prefix)?;
    let mut rows: Vec<Row> = Vec::new();

    for (_, data) in &raw_rows {
        let row = deserialize_row(data)?;
        if let Some(ref filter) = plan.filter {
            if !evaluate_filter(filter, &row, &schema)? { continue; }
        }
        rows.push(row);
    }

    if is_count {
        return Ok(ExecResult::Count(rows.len()));
    }

    // Order by
    if let Some((ref col, ascending)) = plan.order_by {
        let idx = schema.column_index(col)
            .ok_or_else(|| Error::ColumnNotFound(col.clone()))?;
        rows.sort_by(|a, b| {
            let cmp = compare_values(&a[idx], &b[idx]);
            if ascending { cmp } else { cmp.reverse() }
        });
    }

    // Limit
    if let Some(limit) = plan.limit {
        rows.truncate(limit);
    }

    // Project columns
    let (col_names, projected) = project_rows(&plan.columns, &rows, &schema)?;

    Ok(ExecResult::Rows { columns: col_names, rows: projected })
}

fn exec_delete(table: &str, filter: Option<&FilterExpr>, storage: &mut dyn Storage) -> Result<ExecResult> {
    let schema = load_schema(table, storage)?;
    let prefix = row_prefix(table);
    let raw_rows = storage.scan_prefix(&prefix)?;
    let mut count = 0;

    for (key, data) in &raw_rows {
        let row = deserialize_row(data)?;
        let should_delete = match filter {
            Some(f) => evaluate_filter(f, &row, &schema)?,
            None => true,
        };
        if should_delete {
            storage.delete(key)?;
            count += 1;
        }
    }

    Ok(ExecResult::Deleted(count))
}

// ── Filter evaluation ──────────────────────────────────────────────

fn evaluate_filter(filter: &FilterExpr, row: &[Value], schema: &TableSchema) -> Result<bool> {
    match filter {
        FilterExpr::Compare { column, op, value } => {
            let idx = schema.column_index(column)
                .ok_or_else(|| Error::ColumnNotFound(column.clone()))?;
            let row_val = &row[idx];
            Ok(match op {
                CompareOp::Eq => compare_values(row_val, value) == std::cmp::Ordering::Equal,
                CompareOp::NotEq => compare_values(row_val, value) != std::cmp::Ordering::Equal,
                CompareOp::Lt => compare_values(row_val, value) == std::cmp::Ordering::Less,
                CompareOp::Gt => compare_values(row_val, value) == std::cmp::Ordering::Greater,
                CompareOp::LtEq => compare_values(row_val, value) != std::cmp::Ordering::Greater,
                CompareOp::GtEq => compare_values(row_val, value) != std::cmp::Ordering::Less,
            })
        }
        FilterExpr::And(a, b) => {
            Ok(evaluate_filter(a, row, schema)? && evaluate_filter(b, row, schema)?)
        }
        FilterExpr::Or(a, b) => {
            Ok(evaluate_filter(a, row, schema)? || evaluate_filter(b, row, schema)?)
        }
        FilterExpr::Not(inner) => {
            Ok(!evaluate_filter(inner, row, schema)?)
        }
    }
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        // Cross-type: compare as strings
        _ => a.to_string().cmp(&b.to_string()),
    }
}

fn project_rows(
    columns: &[SelectColumn],
    rows: &[Row],
    schema: &TableSchema,
) -> Result<(Vec<String>, Vec<Row>)> {
    // Determine output column names and indices
    let mut col_names = Vec::new();
    let mut indices = Vec::new();

    for col in columns {
        match col {
            SelectColumn::Star => {
                for (i, c) in schema.columns.iter().enumerate() {
                    col_names.push(c.name.clone());
                    indices.push(i);
                }
            }
            SelectColumn::Named(name) => {
                let idx = schema.column_index(name)
                    .ok_or_else(|| Error::ColumnNotFound(name.clone()))?;
                col_names.push(name.clone());
                indices.push(idx);
            }
            SelectColumn::Count => {
                // handled earlier
            }
        }
    }

    let projected = rows.iter().map(|row| {
        indices.iter().map(|&i| row[i].clone()).collect()
    }).collect();

    Ok((col_names, projected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;

    fn run_sql(storage: &mut MemoryStorage, sql: &str) -> Result<ExecResult> {
        let stmt = crate::sql::parser::parse(sql)?;
        let plan = crate::sql::planner::plan(stmt)?;
        execute(plan, storage)
    }

    #[test]
    fn test_create_insert_select() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE users (id INT, name TEXT, age INT)").unwrap();
        run_sql(&mut s, "INSERT INTO users VALUES (1, 'Alice', 30)").unwrap();
        run_sql(&mut s, "INSERT INTO users VALUES (2, 'Bob', 25)").unwrap();

        let result = run_sql(&mut s, "SELECT * FROM users").unwrap();
        match result {
            ExecResult::Rows { rows, columns } => {
                assert_eq!(columns, vec!["id", "name", "age"]);
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_where_clause() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (x INT, y TEXT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (1, 'a')").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (2, 'b')").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (3, 'c')").unwrap();

        let result = run_sql(&mut s, "SELECT * FROM t WHERE x > 1").unwrap();
        match result {
            ExecResult::Rows { rows, .. } => assert_eq!(rows.len(), 2),
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_order_by_limit() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (name TEXT, score INT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES ('A', 10)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES ('B', 30)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES ('C', 20)").unwrap();

        let result = run_sql(&mut s, "SELECT * FROM t ORDER BY score DESC LIMIT 2").unwrap();
        match result {
            ExecResult::Rows { rows, .. } => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][1], Value::Int(30)); // B first
            }
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_delete() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (id INT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (1)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (2)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (3)").unwrap();

        let result = run_sql(&mut s, "DELETE FROM t WHERE id = 2").unwrap();
        match result {
            ExecResult::Deleted(n) => assert_eq!(n, 1),
            _ => panic!("Expected Deleted"),
        }

        let result = run_sql(&mut s, "SELECT * FROM t").unwrap();
        match result {
            ExecResult::Rows { rows, .. } => assert_eq!(rows.len(), 2),
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_count() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (x INT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (1)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (2)").unwrap();

        let result = run_sql(&mut s, "SELECT COUNT(*) FROM t").unwrap();
        match result {
            ExecResult::Count(n) => assert_eq!(n, 2),
            _ => panic!("Expected Count"),
        }
    }

    #[test]
    fn test_column_projection() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (a INT, b TEXT, c INT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (1, 'x', 10)").unwrap();

        let result = run_sql(&mut s, "SELECT b, c FROM t").unwrap();
        match result {
            ExecResult::Rows { columns, rows } => {
                assert_eq!(columns, vec!["b", "c"]);
                assert_eq!(rows[0], vec![Value::Text("x".into()), Value::Int(10)]);
            }
            _ => panic!("Expected Rows"),
        }
    }

    #[test]
    fn test_drop_table() {
        let mut s = MemoryStorage::new();
        run_sql(&mut s, "CREATE TABLE t (x INT)").unwrap();
        run_sql(&mut s, "INSERT INTO t VALUES (1)").unwrap();
        run_sql(&mut s, "DROP TABLE t").unwrap();
        assert!(run_sql(&mut s, "SELECT * FROM t").is_err());
    }
}
