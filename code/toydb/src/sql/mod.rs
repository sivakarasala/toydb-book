/// SQL Layer (Ch6-11)
///
/// Ch6:  Lexer — string → tokens
/// Ch7:  Parser — tokens → AST
/// Ch8:  Planner — AST → plan
/// Ch9:  Optimizer — plan → optimized plan
/// Ch10: Executor — plan → results
/// Ch11: Joins, aggregations, sorting

pub mod lexer;
pub mod parser;
pub mod planner;
pub mod executor;
pub mod types;
