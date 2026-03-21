/// Chapter 7: SQL Parser — Building the AST
/// Exercise: Build a recursive descent parser that produces an AST.
///
/// Run tests: cargo test --bin exercise

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Number(f64),
    String(String),
    Column(String),
    BinaryOp {
        left: Box<Expression>,
        op: Op,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op { Add, Sub, Mul, Div, Eq, NotEq, Lt, Gt }

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select {
        columns: Vec<Expression>,
        from: String,
        where_clause: Option<Expression>,
    },
    Insert {
        table: String,
        values: Vec<Expression>,
    },
    CreateTable {
        name: String,
        columns: Vec<(String, String)>, // (name, type)
    },
}

// Reuse the lexer from ch06
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64), String(String), Ident(String), Keyword(String),
    Star, Comma, LeftParen, RightParen, Semicolon,
    Equals, NotEquals, LessThan, GreaterThan,
    Plus, Minus, Slash, Eof,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect_keyword(&mut self, kw: &str) {
        match self.advance() {
            Token::Keyword(k) if k.to_uppercase() == kw => {}
            other => panic!("Expected keyword {kw}, got {other:?}"),
        }
    }

    fn expect_ident(&mut self) -> String {
        match self.advance() {
            Token::Ident(s) => s,
            other => panic!("Expected identifier, got {other:?}"),
        }
    }

    /// Parse a statement.
    pub fn parse(&mut self) -> Statement {
        // TODO: Match on peek() to determine statement type:
        //   Keyword("SELECT") → parse_select()
        //   Keyword("INSERT") → parse_insert()
        //   Keyword("CREATE") → parse_create_table()
        todo!("Implement parse")
    }

    fn parse_select(&mut self) -> Statement {
        // TODO: SELECT columns FROM table [WHERE expr]
        // 1. Consume SELECT
        // 2. Parse column list (comma-separated expressions, or *)
        // 3. Consume FROM, parse table name
        // 4. Optionally parse WHERE clause
        todo!("Implement parse_select")
    }

    fn parse_insert(&mut self) -> Statement {
        // TODO: INSERT INTO table VALUES (expr, expr, ...)
        todo!("Implement parse_insert")
    }

    fn parse_create_table(&mut self) -> Statement {
        // TODO: CREATE TABLE name (col1 type1, col2 type2, ...)
        todo!("Implement parse_create_table")
    }

    /// Parse an expression with operator precedence.
    fn parse_expression(&mut self) -> Expression {
        // TODO: Parse comparison (=, <, >) which calls parse_additive
        todo!("Implement parse_expression")
    }

    fn parse_additive(&mut self) -> Expression {
        // TODO: Parse + and - which calls parse_primary
        todo!("Implement parse_additive")
    }

    fn parse_primary(&mut self) -> Expression {
        // TODO: Number, String, Ident (column), or (expr)
        todo!("Implement parse_primary")
    }
}

fn main() {
    println!("=== Chapter 7: SQL Parser ===");
    println!("Exercise: Build a recursive descent parser.");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(input: &[Token]) -> Vec<Token> {
        let mut v = input.to_vec();
        v.push(Token::Eof);
        v
    }

    #[test]
    fn test_parse_select() {
        let toks = tokens(&[
            Token::Keyword("SELECT".into()), Token::Ident("a".into()),
            Token::Comma, Token::Ident("b".into()),
            Token::Keyword("FROM".into()), Token::Ident("t".into()),
        ]);
        let stmt = Parser::new(toks).parse();
        match stmt {
            Statement::Select { columns, from, .. } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(from, "t");
            }
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_parse_insert() {
        let toks = tokens(&[
            Token::Keyword("INSERT".into()), Token::Keyword("INTO".into()),
            Token::Ident("t".into()), Token::Keyword("VALUES".into()),
            Token::LeftParen, Token::Number(1.0), Token::Comma,
            Token::String("hi".into()), Token::RightParen,
        ]);
        let stmt = Parser::new(toks).parse();
        match stmt {
            Statement::Insert { table, values } => {
                assert_eq!(table, "t");
                assert_eq!(values.len(), 2);
            }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_expression_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3) — but we only have +/- level
        // so 1 + 2 + 3 is left-associative
        let toks = tokens(&[
            Token::Number(1.0), Token::Plus, Token::Number(2.0),
            Token::Plus, Token::Number(3.0),
        ]);
        let expr = Parser::new(toks).parse_expression();
        // Should be BinaryOp(BinaryOp(1, +, 2), +, 3)
        match expr {
            Expression::BinaryOp { op: Op::Add, .. } => {}
            _ => panic!("Expected addition"),
        }
    }
}
