/// Chapter 6: SQL Lexer — SOLUTION

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Select, From, Where, Insert, Into, Values, Create, Table,
    And, Or, Not, Int, Text, Bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    String(String),
    Ident(String),
    Keyword(Keyword),
    Star, Comma, LeftParen, RightParen, Semicolon,
    Equals, NotEquals, LessThan, GreaterThan,
    Plus, Minus,
    Eof,
}

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer { input: input.chars().peekable() }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            let is_eof = tok == Token::Eof;
            tokens.push(tok);
            if is_eof { break; }
        }
        tokens
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.input.peek() {
            None => Token::Eof,
            Some(&ch) => match ch {
                '*' => { self.input.next(); Token::Star }
                '(' => { self.input.next(); Token::LeftParen }
                ')' => { self.input.next(); Token::RightParen }
                ',' => { self.input.next(); Token::Comma }
                ';' => { self.input.next(); Token::Semicolon }
                '+' => { self.input.next(); Token::Plus }
                '-' => { self.input.next(); Token::Minus }
                '<' => { self.input.next(); Token::LessThan }
                '>' => { self.input.next(); Token::GreaterThan }
                '=' => { self.input.next(); Token::Equals }
                '!' => {
                    self.input.next();
                    if self.input.peek() == Some(&'=') {
                        self.input.next();
                        Token::NotEquals
                    } else {
                        // Treat lone '!' as unknown, skip
                        self.next_token()
                    }
                }
                '\'' => self.scan_string(),
                c if c.is_ascii_digit() => self.scan_number(),
                c if c.is_alphabetic() || c == '_' => self.scan_identifier(),
                _ => { self.input.next(); self.next_token() }
            },
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if ch.is_whitespace() { self.input.next(); } else { break; }
        }
    }

    fn scan_number(&mut self) -> Token {
        let mut s = String::new();
        let mut has_dot = false;
        while let Some(&ch) = self.input.peek() {
            if ch.is_ascii_digit() {
                s.push(ch);
                self.input.next();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                s.push(ch);
                self.input.next();
            } else {
                break;
            }
        }
        Token::Number(s.parse().unwrap())
    }

    fn scan_string(&mut self) -> Token {
        self.input.next(); // consume opening '
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '\'' {
                self.input.next();
                break;
            }
            s.push(ch);
            self.input.next();
        }
        Token::String(s)
    }

    fn scan_identifier(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                s.push(ch);
                self.input.next();
            } else {
                break;
            }
        }
        match Self::lookup_keyword(&s) {
            Some(kw) => Token::Keyword(kw),
            None => Token::Ident(s),
        }
    }

    fn lookup_keyword(word: &str) -> Option<Keyword> {
        match word.to_uppercase().as_str() {
            "SELECT" => Some(Keyword::Select),
            "FROM" => Some(Keyword::From),
            "WHERE" => Some(Keyword::Where),
            "INSERT" => Some(Keyword::Insert),
            "INTO" => Some(Keyword::Into),
            "VALUES" => Some(Keyword::Values),
            "CREATE" => Some(Keyword::Create),
            "TABLE" => Some(Keyword::Table),
            "AND" => Some(Keyword::And),
            "OR" => Some(Keyword::Or),
            "NOT" => Some(Keyword::Not),
            "INT" => Some(Keyword::Int),
            "TEXT" => Some(Keyword::Text),
            "BOOL" => Some(Keyword::Bool),
            _ => None,
        }
    }
}

fn main() {
    println!("=== Chapter 6: SQL Lexer — Solution ===");
    let sql = "SELECT name, age FROM users WHERE age > 21";
    let tokens = Lexer::new(sql).tokenize();
    println!("SQL: {sql}");
    println!("Tokens:");
    for tok in &tokens {
        println!("  {:?}", tok);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_star() {
        let tokens = Lexer::new("SELECT * FROM users").tokenize();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Select), Token::Star,
            Token::Keyword(Keyword::From), Token::Ident("users".into()),
            Token::Eof,
        ]);
    }

    #[test]
    fn test_insert() {
        let tokens = Lexer::new("INSERT INTO t VALUES (1, 'hello')").tokenize();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Insert), Token::Keyword(Keyword::Into),
            Token::Ident("t".into()), Token::Keyword(Keyword::Values),
            Token::LeftParen, Token::Number(1.0), Token::Comma,
            Token::String("hello".into()), Token::RightParen, Token::Eof,
        ]);
    }

    #[test]
    fn test_where_clause() {
        let tokens = Lexer::new("SELECT * FROM t WHERE x = 42").tokenize();
        assert!(tokens.contains(&Token::Keyword(Keyword::Where)));
        assert!(tokens.contains(&Token::Equals));
        assert!(tokens.contains(&Token::Number(42.0)));
    }

    #[test]
    fn test_operators() {
        let tokens = Lexer::new("a + b - c").tokenize();
        assert!(tokens.contains(&Token::Plus));
        assert!(tokens.contains(&Token::Minus));
    }
}
