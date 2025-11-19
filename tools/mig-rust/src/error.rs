//! Unified error types for mig-rust using thiserror

use thiserror::Error;

/// Top-level error type for MIG operations
#[derive(Error, Debug)]
pub enum MigError {
    #[error("lexer error: {0}")]
    Lexer(String),

    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("preprocessor error: {0}")]
    Preprocessor(#[from] PreprocessorError),

    #[error("semantic error: {0}")]
    Semantic(#[from] SemanticError),

    #[error("code generation error: {0}")]
    Codegen(#[from] CodegenError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Parse errors
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("expected {expected}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
    },

    #[error("undefined type: {0}")]
    UndefinedType(String),

    #[error("invalid subsystem declaration")]
    InvalidSubsystem,

    #[error("invalid routine declaration")]
    InvalidRoutine,

    #[error("invalid type specification: {0}")]
    InvalidTypeSpec(String),

    #[error("duplicate definition: {0}")]
    DuplicateDefinition(String),
}

/// Preprocessor errors
#[derive(Error, Debug)]
pub enum PreprocessorError {
    #[error("unbalanced conditional: #endif without #if")]
    UnbalancedEndif,

    #[error("unbalanced conditional: #else without #if")]
    UnbalancedElse,

    #[error("unclosed conditional block at end of file")]
    UnclosedBlock,

    #[error("invalid expression: {0}")]
    InvalidExpression(String),

    #[error("undefined symbol: {0}")]
    UndefinedSymbol(String),
}

/// Semantic analysis errors
#[derive(Error, Debug)]
pub enum SemanticError {
    #[error("undefined type: {0}")]
    UndefinedType(String),

    #[error("type mismatch: expected {expected}, found {actual}")]
    TypeMismatch {
        expected: String,
        actual: String,
    },

    #[error("array size must be specified for type {0}")]
    MissingArraySize(String),

    #[error("array size too large: {size} > {max}")]
    ArrayTooLarge {
        size: u32,
        max: u32,
    },

    #[error("invalid port disposition: {0}")]
    InvalidPortDisposition(String),

    #[error("message layout exceeds maximum size: {size} > {max}")]
    MessageTooLarge {
        size: usize,
        max: usize,
    },

    #[error("invalid direction for argument {name}: {direction}")]
    InvalidDirection {
        name: String,
        direction: String,
    },
}

/// Code generation errors
#[derive(Error, Debug)]
pub enum CodegenError {
    #[error("unresolved type: {0}")]
    UnresolvedType(String),

    #[error("unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("invalid template: {0}")]
    InvalidTemplate(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("formatting error: {0}")]
    Fmt(#[from] std::fmt::Error),
}

/// Backwards compatibility wrapper
#[derive(Debug)]
pub struct LegacyError {
    pub message: String,
}

impl std::fmt::Display for LegacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LegacyError {}

impl From<String> for CodegenError {
    fn from(message: String) -> Self {
        CodegenError::UnsupportedFeature(message)
    }
}

impl From<&str> for CodegenError {
    fn from(message: &str) -> Self {
        CodegenError::UnsupportedFeature(message.to_string())
    }
}
