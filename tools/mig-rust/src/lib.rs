//! MIG (Mach Interface Generator) Rust implementation
//!
//! Code generation tool - suppress style lints
#![allow(clippy::useless_format)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::trim_split_whitespace)]

pub mod codegen;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod semantic;
pub mod types;

// Re-export main types
pub use error::*;
pub use lexer::simple::SimpleLexer;
pub use parser::ast::*;
pub use parser::Parser;
pub use preprocessor::{PreprocessorConfig, PreprocessorFilter, SymbolTable, SymbolValue};
pub use semantic::{AnalyzedRoutine, AnalyzedSubsystem, SemanticAnalyzer};

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
