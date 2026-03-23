/// SQL Parser (Ch7-8)
///
/// Converts tokens into an AST using recursive descent parsing.
/// Supports CREATE TABLE, INSERT, SELECT (with WHERE, ORDER BY, LIMIT), DROP TABLE.

use crate::error::{Error, Result};
use crate::sql::lexer::{Keyword, Token};
use crate::sql::types::DataType;

// ── AST Nodes ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Statement {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    DropTable {
        name: String,
    },
    Insert {
        table: String,
        values: Vec<Vec<Expr>>,
    },
    Select {
        columns: Vec<SelectColumn>,
        from: String,
        where_clause: Option<Expr>,
        order_by: Option<(String, bool)>, // (column, ascending)
        limit: Option<usize>,
    },
    Delete {
        table: String,
        where_clause: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub primary_key: bool,
}

#[derive(Debug, Clone)]
pub enum SelectColumn {
    Star,
    Named(String),
    Count,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(LiteralValue),
    Column(String),
    BinaryOp { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    Not(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum LiteralValue {
    Int(i64),
    Text(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Eq, NotEq, Lt, Gt, LtEq, GtEq, And, Or, Plus, Minus,
}

// ── Parser ─────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Statement> {
        let stmt = match self.peek() {
            Token::Keyword(Keyword::Create) => self.parse_create_table()?,
            Token::Keyword(Keyword::Drop) => self.parse_drop_table()?,
            Token::Keyword(Keyword::Insert) => self.parse_insert()?,
            Token::Keyword(Keyword::Select) => self.parse_select()?,
            Token::Keyword(Keyword::Delete) => self.parse_delete()?,
            other => return Err(Error::Parse(format!("Unexpected token: {:?}", other))),
        };
        // Consume optional semicolon
        if self.peek() == &Token::Semicolon { self.advance(); }
        Ok(stmt)
    }

    // ── CREATE TABLE ───────────────────────────────────────────

    fn parse_create_table(&mut self) -> Result<Statement> {
        self.expect_keyword(Keyword::Create)?;
        self.expect_keyword(Keyword::Table)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LeftParen)?;

        let mut columns = Vec::new();
        loop {
            let col_name = self.expect_ident()?;
            let data_type = self.parse_data_type()?;
            let primary_key = if self.peek() == &Token::Keyword(Keyword::Primary) {
                self.advance();
                self.expect_keyword(Keyword::Key)?;
                true
            } else {
                false
            };
            columns.push(ColumnDef { name: col_name, data_type, primary_key });
            if self.peek() != &Token::Comma { break; }
            self.advance(); // consume comma
        }
        self.expect_token(&Token::RightParen)?;
        Ok(Statement::CreateTable { name, columns })
    }

    fn parse_data_type(&mut self) -> Result<DataType> {
        match self.advance() {
            Token::Keyword(Keyword::Int) => Ok(DataType::Int),
            Token::Keyword(Keyword::Text) => Ok(DataType::Text),
            Token::Keyword(Keyword::Bool) => Ok(DataType::Bool),
            other => Err(Error::Parse(format!("Expected data type, got {:?}", other))),
        }
    }

    // ── DROP TABLE ─────────────────────────────────────────────

    fn parse_drop_table(&mut self) -> Result<Statement> {
        self.expect_keyword(Keyword::Drop)?;
        self.expect_keyword(Keyword::Table)?;
        let name = self.expect_ident()?;
        Ok(Statement::DropTable { name })
    }

    // ── INSERT ─────────────────────────────────────────────────

    fn parse_insert(&mut self) -> Result<Statement> {
        self.expect_keyword(Keyword::Insert)?;
        self.expect_keyword(Keyword::Into)?;
        let table = self.expect_ident()?;
        self.expect_keyword(Keyword::Values)?;

        let mut rows = Vec::new();
        loop {
            self.expect_token(&Token::LeftParen)?;
            let mut values = Vec::new();
            loop {
                values.push(self.parse_expr()?);
                if self.peek() != &Token::Comma { break; }
                self.advance();
            }
            self.expect_token(&Token::RightParen)?;
            rows.push(values);
            if self.peek() != &Token::Comma { break; }
            self.advance();
        }
        Ok(Statement::Insert { table, values: rows })
    }

    // ── SELECT ─────────────────────────────────────────────────

    fn parse_select(&mut self) -> Result<Statement> {
        self.expect_keyword(Keyword::Select)?;
        let columns = self.parse_select_columns()?;
        self.expect_keyword(Keyword::From)?;
        let from = self.expect_ident()?;

        let where_clause = if self.peek() == &Token::Keyword(Keyword::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let order_by = if self.peek() == &Token::Keyword(Keyword::Order) {
            self.advance();
            self.expect_keyword(Keyword::By)?;
            let col = self.expect_ident()?;
            let asc = if self.peek() == &Token::Keyword(Keyword::Desc) {
                self.advance();
                false
            } else {
                if self.peek() == &Token::Keyword(Keyword::Asc) { self.advance(); }
                true
            };
            Some((col, asc))
        } else {
            None
        };

        let limit = if self.peek() == &Token::Keyword(Keyword::Limit) {
            self.advance();
            match self.advance() {
                Token::Number(n) => Some(n as usize),
                other => return Err(Error::Parse(format!("Expected number after LIMIT, got {:?}", other))),
            }
        } else {
            None
        };

        Ok(Statement::Select { columns, from, where_clause, order_by, limit })
    }

    fn parse_select_columns(&mut self) -> Result<Vec<SelectColumn>> {
        let mut cols = Vec::new();
        loop {
            match self.peek().clone() {
                Token::Star => { self.advance(); cols.push(SelectColumn::Star); }
                Token::Keyword(Keyword::Count) => {
                    self.advance();
                    self.expect_token(&Token::LeftParen)?;
                    self.expect_token(&Token::Star)?;
                    self.expect_token(&Token::RightParen)?;
                    cols.push(SelectColumn::Count);
                }
                _ => {
                    let name = self.expect_ident()?;
                    cols.push(SelectColumn::Named(name));
                }
            }
            if self.peek() != &Token::Comma { break; }
            self.advance();
        }
        Ok(cols)
    }

    // ── DELETE ──────────────────────────────────────────────────

    fn parse_delete(&mut self) -> Result<Statement> {
        self.expect_keyword(Keyword::Delete)?;
        self.expect_keyword(Keyword::From)?;
        let table = self.expect_ident()?;
        let where_clause = if self.peek() == &Token::Keyword(Keyword::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Statement::Delete { table, where_clause })
    }

    // ── Expressions ────────────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        while self.peek() == &Token::Keyword(Keyword::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinaryOp { left: Box::new(left), op: BinOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        while self.peek() == &Token::Keyword(Keyword::And) {
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::BinaryOp { left: Box::new(left), op: BinOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let left = self.parse_primary()?;
        let op = match self.peek() {
            Token::Equals => BinOp::Eq,
            Token::NotEquals => BinOp::NotEq,
            Token::LessThan => BinOp::Lt,
            Token::GreaterThan => BinOp::Gt,
            Token::LessEq => BinOp::LtEq,
            Token::GreaterEq => BinOp::GtEq,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_primary()?;
        Ok(Expr::BinaryOp { left: Box::new(left), op, right: Box::new(right) })
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.peek().clone() {
            Token::Number(n) => { self.advance(); Ok(Expr::Literal(LiteralValue::Int(n))) }
            Token::String(s) => { self.advance(); Ok(Expr::Literal(LiteralValue::Text(s))) }
            Token::Keyword(Keyword::True) => { self.advance(); Ok(Expr::Literal(LiteralValue::Bool(true))) }
            Token::Keyword(Keyword::False) => { self.advance(); Ok(Expr::Literal(LiteralValue::Bool(false))) }
            Token::Keyword(Keyword::Null) => { self.advance(); Ok(Expr::Literal(LiteralValue::Null)) }
            Token::Keyword(Keyword::Not) => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Not(Box::new(expr)))
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_token(&Token::RightParen)?;
                Ok(expr)
            }
            Token::Ident(_) => {
                let name = self.expect_ident()?;
                Ok(Expr::Column(name))
            }
            other => Err(Error::Parse(format!("Unexpected token in expression: {:?}", other))),
        }
    }

    // ── Helpers ────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect_keyword(&mut self, kw: Keyword) -> Result<()> {
        match self.advance() {
            Token::Keyword(k) if k == kw => Ok(()),
            other => Err(Error::Parse(format!("Expected {:?}, got {:?}", kw, other))),
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.advance() {
            Token::Ident(s) => Ok(s),
            other => Err(Error::Parse(format!("Expected identifier, got {:?}", other))),
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<()> {
        let tok = self.advance();
        if &tok == expected { Ok(()) }
        else { Err(Error::Parse(format!("Expected {:?}, got {:?}", expected, tok))) }
    }
}

/// Convenience: parse a SQL string into a Statement.
pub fn parse(sql: &str) -> Result<Statement> {
    let tokens = crate::sql::lexer::Lexer::new(sql).tokenize();
    Parser::new(tokens).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table() {
        let stmt = parse("CREATE TABLE users (id INT, name TEXT)").unwrap();
        match stmt {
            Statement::CreateTable { name, columns } => {
                assert_eq!(name, "users");
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].name, "id");
            }
            _ => panic!("Expected CreateTable"),
        }
    }

    #[test]
    fn test_insert() {
        let stmt = parse("INSERT INTO users VALUES (1, 'Alice')").unwrap();
        match stmt {
            Statement::Insert { table, values } => {
                assert_eq!(table, "users");
                assert_eq!(values.len(), 1);
                assert_eq!(values[0].len(), 2);
            }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_select_where() {
        let stmt = parse("SELECT name, age FROM users WHERE age > 21").unwrap();
        match stmt {
            Statement::Select { columns, from, where_clause, .. } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(from, "users");
                assert!(where_clause.is_some());
            }
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_select_order_limit() {
        let stmt = parse("SELECT * FROM users ORDER BY age DESC LIMIT 10").unwrap();
        match stmt {
            Statement::Select { order_by, limit, .. } => {
                assert_eq!(order_by, Some(("age".into(), false)));
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Expected Select"),
        }
    }
}
