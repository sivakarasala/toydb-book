/// Chapter 7: SQL Parser — SOLUTION

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Number(f64),
    String(String),
    Column(String),
    BinaryOp { left: Box<Expression>, op: Op, right: Box<Expression> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op { Add, Sub, Mul, Div, Eq, NotEq, Lt, Gt }

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select { columns: Vec<Expression>, from: String, where_clause: Option<Expression> },
    Insert { table: String, values: Vec<Expression> },
    CreateTable { name: String, columns: Vec<(String, String)> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64), String(String), Ident(String), Keyword(String),
    Star, Comma, LeftParen, RightParen, Semicolon,
    Equals, NotEquals, LessThan, GreaterThan,
    Plus, Minus, Slash, Eof,
}

pub struct Parser { tokens: Vec<Token>, pos: usize }

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self { Parser { tokens, pos: 0 } }

    fn peek(&self) -> &Token { self.tokens.get(self.pos).unwrap_or(&Token::Eof) }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect_keyword(&mut self, kw: &str) {
        match self.advance() {
            Token::Keyword(k) if k.to_uppercase() == kw => {}
            other => panic!("Expected {kw}, got {other:?}"),
        }
    }

    fn expect_ident(&mut self) -> String {
        match self.advance() { Token::Ident(s) => s, other => panic!("Expected ident, got {other:?}") }
    }

    pub fn parse(&mut self) -> Statement {
        match self.peek() {
            Token::Keyword(k) if k.to_uppercase() == "SELECT" => self.parse_select(),
            Token::Keyword(k) if k.to_uppercase() == "INSERT" => self.parse_insert(),
            Token::Keyword(k) if k.to_uppercase() == "CREATE" => self.parse_create_table(),
            other => panic!("Unexpected token: {other:?}"),
        }
    }

    fn parse_select(&mut self) -> Statement {
        self.expect_keyword("SELECT");
        let mut columns = Vec::new();
        if matches!(self.peek(), Token::Star) {
            self.advance();
            columns.push(Expression::Column("*".into()));
        } else {
            columns.push(self.parse_expression());
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                columns.push(self.parse_expression());
            }
        }
        self.expect_keyword("FROM");
        let from = self.expect_ident();
        let where_clause = if matches!(self.peek(), Token::Keyword(k) if k.to_uppercase() == "WHERE") {
            self.advance();
            Some(self.parse_expression())
        } else { None };
        Statement::Select { columns, from, where_clause }
    }

    fn parse_insert(&mut self) -> Statement {
        self.expect_keyword("INSERT");
        self.expect_keyword("INTO");
        let table = self.expect_ident();
        self.expect_keyword("VALUES");
        assert!(matches!(self.advance(), Token::LeftParen));
        let mut values = vec![self.parse_expression()];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            values.push(self.parse_expression());
        }
        assert!(matches!(self.advance(), Token::RightParen));
        Statement::Insert { table, values }
    }

    fn parse_create_table(&mut self) -> Statement {
        self.expect_keyword("CREATE");
        self.expect_keyword("TABLE");
        let name = self.expect_ident();
        assert!(matches!(self.advance(), Token::LeftParen));
        let mut columns = Vec::new();
        let col_name = self.expect_ident();
        let col_type = self.expect_ident();
        columns.push((col_name, col_type));
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            let cn = self.expect_ident();
            let ct = self.expect_ident();
            columns.push((cn, ct));
        }
        assert!(matches!(self.advance(), Token::RightParen));
        Statement::CreateTable { name, columns }
    }

    pub fn parse_expression(&mut self) -> Expression {
        let left = self.parse_additive();
        match self.peek() {
            Token::Equals => { self.advance(); let r = self.parse_additive(); Expression::BinaryOp { left: Box::new(left), op: Op::Eq, right: Box::new(r) } }
            Token::NotEquals => { self.advance(); let r = self.parse_additive(); Expression::BinaryOp { left: Box::new(left), op: Op::NotEq, right: Box::new(r) } }
            Token::LessThan => { self.advance(); let r = self.parse_additive(); Expression::BinaryOp { left: Box::new(left), op: Op::Lt, right: Box::new(r) } }
            Token::GreaterThan => { self.advance(); let r = self.parse_additive(); Expression::BinaryOp { left: Box::new(left), op: Op::Gt, right: Box::new(r) } }
            _ => left,
        }
    }

    fn parse_additive(&mut self) -> Expression {
        let mut left = self.parse_primary();
        loop {
            match self.peek() {
                Token::Plus => { self.advance(); let r = self.parse_primary(); left = Expression::BinaryOp { left: Box::new(left), op: Op::Add, right: Box::new(r) }; }
                Token::Minus => { self.advance(); let r = self.parse_primary(); left = Expression::BinaryOp { left: Box::new(left), op: Op::Sub, right: Box::new(r) }; }
                _ => break,
            }
        }
        left
    }

    fn parse_primary(&mut self) -> Expression {
        match self.advance() {
            Token::Number(n) => Expression::Number(n),
            Token::String(s) => Expression::String(s),
            Token::Ident(s) => Expression::Column(s),
            Token::LeftParen => { let e = self.parse_expression(); assert!(matches!(self.advance(), Token::RightParen)); e }
            other => panic!("Unexpected in expression: {other:?}"),
        }
    }
}

fn main() {
    println!("=== Chapter 7: SQL Parser — Solution ===");
    let tokens = vec![
        Token::Keyword("SELECT".into()), Token::Ident("name".into()),
        Token::Keyword("FROM".into()), Token::Ident("users".into()),
        Token::Keyword("WHERE".into()), Token::Ident("age".into()),
        Token::GreaterThan, Token::Number(21.0), Token::Eof,
    ];
    let stmt = Parser::new(tokens).parse();
    println!("{stmt:#?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tokens(input: &[Token]) -> Vec<Token> { let mut v = input.to_vec(); v.push(Token::Eof); v }

    #[test]
    fn test_parse_select() {
        let toks = tokens(&[
            Token::Keyword("SELECT".into()), Token::Ident("a".into()),
            Token::Comma, Token::Ident("b".into()),
            Token::Keyword("FROM".into()), Token::Ident("t".into()),
        ]);
        match Parser::new(toks).parse() {
            Statement::Select { columns, from, .. } => { assert_eq!(columns.len(), 2); assert_eq!(from, "t"); }
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
        match Parser::new(toks).parse() {
            Statement::Insert { table, values } => { assert_eq!(table, "t"); assert_eq!(values.len(), 2); }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_expression_precedence() {
        let toks = tokens(&[Token::Number(1.0), Token::Plus, Token::Number(2.0), Token::Plus, Token::Number(3.0)]);
        match Parser::new(toks).parse_expression() {
            Expression::BinaryOp { op: Op::Add, .. } => {}
            _ => panic!("Expected addition"),
        }
    }
}
