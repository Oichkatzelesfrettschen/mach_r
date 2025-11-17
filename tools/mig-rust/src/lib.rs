pub mod lexer;
pub mod parser;
pub mod types;
pub mod codegen;

// Re-export main types
pub use lexer::simple::SimpleLexer;
pub use parser::{Parser, ParseError};
pub use parser::ast::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lexing() {
        let mut lexer = SimpleLexer::new("subsystem test 2000;".to_string());
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens.len(), 4);
    }
}
