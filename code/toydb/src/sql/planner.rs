/// Query Planner (Ch8-9)
///
/// Converts parsed AST into an execution plan.
/// The planner is deliberately simple — no cost-based optimization,
/// just a direct translation with basic filter pushdown.

use crate::error::{Error, Result};
use crate::sql::parser::*;
use crate::sql::types::Value;

/// An execution plan node.
#[derive(Debug)]
pub enum Plan {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    DropTable {
        name: String,
    },
    Insert {
        table: String,
        rows: Vec<Vec<Value>>,
    },
    Select {
        plan: SelectPlan,
    },
    Delete {
        table: String,
        filter: Option<FilterExpr>,
    },
}

#[derive(Debug)]
pub struct SelectPlan {
    pub table: String,
    pub columns: Vec<SelectColumn>,
    pub filter: Option<FilterExpr>,
    pub order_by: Option<(String, bool)>,
    pub limit: Option<usize>,
}

/// A simplified filter expression for execution.
#[derive(Debug, Clone)]
pub enum FilterExpr {
    Compare { column: String, op: CompareOp, value: Value },
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
}

#[derive(Debug, Clone)]
pub enum CompareOp {
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
}

/// Convert an AST Statement into an execution Plan.
pub fn plan(stmt: Statement) -> Result<Plan> {
    match stmt {
        Statement::CreateTable { name, columns } => {
            Ok(Plan::CreateTable { name, columns })
        }
        Statement::DropTable { name } => {
            Ok(Plan::DropTable { name })
        }
        Statement::Insert { table, values } => {
            let rows = values.into_iter()
                .map(|row| row.into_iter().map(expr_to_value).collect::<Result<Vec<_>>>())
                .collect::<Result<Vec<_>>>()?;
            Ok(Plan::Insert { table, rows })
        }
        Statement::Select { columns, from, where_clause, order_by, limit } => {
            let filter = where_clause.map(expr_to_filter).transpose()?;
            Ok(Plan::Select {
                plan: SelectPlan { table: from, columns, filter, order_by, limit },
            })
        }
        Statement::Delete { table, where_clause } => {
            let filter = where_clause.map(expr_to_filter).transpose()?;
            Ok(Plan::Delete { table, filter })
        }
    }
}

fn expr_to_value(expr: Expr) -> Result<Value> {
    match expr {
        Expr::Literal(LiteralValue::Int(n)) => Ok(Value::Int(n)),
        Expr::Literal(LiteralValue::Text(s)) => Ok(Value::Text(s)),
        Expr::Literal(LiteralValue::Bool(b)) => Ok(Value::Bool(b)),
        Expr::Literal(LiteralValue::Null) => Ok(Value::Null),
        other => Err(Error::Plan(format!("Expected literal value, got {:?}", other))),
    }
}

fn expr_to_filter(expr: Expr) -> Result<FilterExpr> {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            match op {
                BinOp::And => Ok(FilterExpr::And(
                    Box::new(expr_to_filter(*left)?),
                    Box::new(expr_to_filter(*right)?),
                )),
                BinOp::Or => Ok(FilterExpr::Or(
                    Box::new(expr_to_filter(*left)?),
                    Box::new(expr_to_filter(*right)?),
                )),
                _ => {
                    let column = match *left {
                        Expr::Column(name) => name,
                        other => return Err(Error::Plan(format!("Expected column, got {:?}", other))),
                    };
                    let value = expr_to_value(*right)?;
                    let compare_op = match op {
                        BinOp::Eq => CompareOp::Eq,
                        BinOp::NotEq => CompareOp::NotEq,
                        BinOp::Lt => CompareOp::Lt,
                        BinOp::Gt => CompareOp::Gt,
                        BinOp::LtEq => CompareOp::LtEq,
                        BinOp::GtEq => CompareOp::GtEq,
                        _ => return Err(Error::Plan(format!("Unsupported operator in filter"))),
                    };
                    Ok(FilterExpr::Compare { column, op: compare_op, value })
                }
            }
        }
        Expr::Not(inner) => Ok(FilterExpr::Not(Box::new(expr_to_filter(*inner)?))),
        other => Err(Error::Plan(format!("Unsupported filter expression: {:?}", other))),
    }
}
