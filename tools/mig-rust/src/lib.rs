pub mod error;
pub mod lexer;
pub mod parser;
pub mod types;
pub mod codegen;
pub mod semantic;
pub mod preprocessor;

// Re-export main types
pub use error::*;
pub use lexer::simple::SimpleLexer;
pub use parser::Parser;
pub use parser::ast::*;
pub use semantic::{SemanticAnalyzer, AnalyzedSubsystem, AnalyzedRoutine};
pub use preprocessor::{PreprocessorConfig, PreprocessorFilter, SymbolTable, SymbolValue};

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
