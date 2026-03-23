/// SQL Lexer (Ch6)
///
/// Converts a SQL string into a stream of tokens.
/// Handles keywords, identifiers, numbers, strings, and operators.

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Select, From, Where, Insert, Into, Values, Create, Table, Drop,
    And, Or, Not, Int, Text, Bool, Order, By, Asc, Desc, Limit,
    Delete, Update, Set, Null, True, False, Count, Primary, Key,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(i64),
    Float(f64),
    String(String),
    Ident(String),
    Keyword(Keyword),
    Star,
    Comma,
    LeftParen,
    RightParen,
    Semicolon,
    Equals,
    NotEquals,
    LessThan,
    GreaterThan,
    LessEq,
    GreaterEq,
    Plus,
    Minus,
    Dot,
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
                '.' => { self.input.next(); Token::Dot }
                '<' => {
                    self.input.next();
                    if self.input.peek() == Some(&'=') { self.input.next(); Token::LessEq }
                    else { Token::LessThan }
                }
                '>' => {
                    self.input.next();
                    if self.input.peek() == Some(&'=') { self.input.next(); Token::GreaterEq }
                    else { Token::GreaterThan }
                }
                '=' => { self.input.next(); Token::Equals }
                '!' => {
                    self.input.next();
                    if self.input.peek() == Some(&'=') { self.input.next(); Token::NotEquals }
                    else { self.next_token() }
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
        while let Some(&ch) = self.input.peek() {
            if ch.is_ascii_digit() { s.push(ch); self.input.next(); }
            else { break; }
        }
        if self.input.peek() == Some(&'.') {
            s.push('.');
            self.input.next();
            while let Some(&ch) = self.input.peek() {
                if ch.is_ascii_digit() { s.push(ch); self.input.next(); }
                else { break; }
            }
            Token::Float(s.parse().unwrap())
        } else {
            Token::Number(s.parse().unwrap())
        }
    }

    fn scan_string(&mut self) -> Token {
        self.input.next(); // opening quote
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch == '\'' {
                self.input.next();
                // Handle escaped quotes ''
                if self.input.peek() == Some(&'\'') {
                    s.push('\'');
                    self.input.next();
                } else {
                    break;
                }
            } else {
                s.push(ch);
                self.input.next();
            }
        }
        Token::String(s)
    }

    fn scan_identifier(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&ch) = self.input.peek() {
            if ch.is_alphanumeric() || ch == '_' { s.push(ch); self.input.next(); }
            else { break; }
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
            "DROP" => Some(Keyword::Drop),
            "AND" => Some(Keyword::And),
            "OR" => Some(Keyword::Or),
            "NOT" => Some(Keyword::Not),
            "INT" => Some(Keyword::Int),
            "TEXT" => Some(Keyword::Text),
            "BOOL" => Some(Keyword::Bool),
            "ORDER" => Some(Keyword::Order),
            "BY" => Some(Keyword::By),
            "ASC" => Some(Keyword::Asc),
            "DESC" => Some(Keyword::Desc),
            "LIMIT" => Some(Keyword::Limit),
            "DELETE" => Some(Keyword::Delete),
            "UPDATE" => Some(Keyword::Update),
            "SET" => Some(Keyword::Set),
            "NULL" => Some(Keyword::Null),
            "TRUE" => Some(Keyword::True),
            "FALSE" => Some(Keyword::False),
            "COUNT" => Some(Keyword::Count),
            "PRIMARY" => Some(Keyword::Primary),
            "KEY" => Some(Keyword::Key),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select() {
        let tokens = Lexer::new("SELECT * FROM users WHERE age > 21").tokenize();
        assert_eq!(tokens[0], Token::Keyword(Keyword::Select));
        assert_eq!(tokens[1], Token::Star);
        assert_eq!(tokens[2], Token::Keyword(Keyword::From));
        assert!(tokens.contains(&Token::GreaterThan));
        assert!(tokens.contains(&Token::Number(21)));
    }

    #[test]
    fn test_insert() {
        let tokens = Lexer::new("INSERT INTO users VALUES (1, 'Alice')").tokenize();
        assert_eq!(tokens[0], Token::Keyword(Keyword::Insert));
        assert!(tokens.contains(&Token::String("Alice".into())));
    }

    #[test]
    fn test_create_table() {
        let tokens = Lexer::new("CREATE TABLE users (id INT, name TEXT)").tokenize();
        assert_eq!(tokens[0], Token::Keyword(Keyword::Create));
        assert_eq!(tokens[1], Token::Keyword(Keyword::Table));
    }
}
