/// Chapter 6: SQL Lexer — Tokenization
/// Exercise: Build a SQL lexer that converts a string into tokens.
///
/// Run tests: cargo test --bin exercise
/// Run:       cargo run --bin exercise

use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

// ── Token Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Create,
    Table,
    And,
    Or,
    Not,
    Int,
    Text,
    Bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Number(f64),
    String(String),
    Ident(String),
    Keyword(Keyword),
    // Operators
    Star,       // *
    Comma,      // ,
    LeftParen,  // (
    RightParen, // )
    Semicolon,  // ;
    Equals,     // =
    NotEquals,  // !=
    LessThan,   // <
    GreaterThan,// >
    Plus,       // +
    Minus,      // -
    // End
    Eof,
}

// ── Lexer ───────────────────────────────────────────────────────────

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input: input.chars().peekable(),
        }
    }

    /// Tokenize the entire input into a Vec of tokens.
    pub fn tokenize(&mut self) -> Vec<Token> {
        // TODO: Loop calling next_token() until you get Token::Eof
        // Collect all tokens (including Eof) into a Vec
        todo!("Implement tokenize")
    }

    /// Get the next token.
    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.input.peek() {
            None => Token::Eof,
            Some(&ch) => match ch {
                // TODO: Match single-character tokens: *, (, ), ,, ;, +, -
                // TODO: Match < and > (just single char for now)
                // TODO: Match = and != (peek ahead for !=)
                // TODO: Match digits → call scan_number()
                // TODO: Match '\'' → call scan_string()
                // TODO: Match alphabetic → call scan_identifier()
                // TODO: For unknown chars, advance and try again
                _ => {
                    todo!("Implement character matching")
                }
            },
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&ch) = self.input.peek() {
            if ch.is_whitespace() {
                self.input.next();
            } else {
                break;
            }
        }
    }

    fn scan_number(&mut self) -> Token {
        // TODO: Collect digits (and one optional '.'), parse as f64
        todo!("Implement scan_number")
    }

    fn scan_string(&mut self) -> Token {
        // TODO: Consume opening quote, collect chars until closing quote
        todo!("Implement scan_string")
    }

    fn scan_identifier(&mut self) -> Token {
        // TODO: Collect alphanumeric + underscore chars
        // Check if the identifier is a keyword (case-insensitive)
        // Return Token::Keyword or Token::Ident
        todo!("Implement scan_identifier")
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
    println!("=== Chapter 6: SQL Lexer ===");
    println!("Exercise: Implement a SQL tokenizer.");
    println!("Run `cargo test --bin exercise` to check your implementation.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_star() {
        let tokens = Lexer::new("SELECT * FROM users").tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Keyword::Select),
                Token::Star,
                Token::Keyword(Keyword::From),
                Token::Ident("users".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_insert() {
        let tokens = Lexer::new("INSERT INTO t VALUES (1, 'hello')").tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Keyword::Insert),
                Token::Keyword(Keyword::Into),
                Token::Ident("t".into()),
                Token::Keyword(Keyword::Values),
                Token::LeftParen,
                Token::Number(1.0),
                Token::Comma,
                Token::String("hello".into()),
                Token::RightParen,
                Token::Eof,
            ]
        );
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
