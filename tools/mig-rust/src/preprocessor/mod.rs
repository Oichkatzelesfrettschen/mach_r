//! Preprocessor for MIG .defs files
//!
//! Handles conditional compilation directives like #if, #ifdef, #ifndef, #else, #endif
//! This is a simplified preprocessor focused on the patterns used in Mach .defs files.

pub mod expr;
pub mod symbols;
pub mod filter;

pub use expr::{PreprocessorExpr, ExprParser};
pub use symbols::{SymbolTable, SymbolValue};
pub use filter::{PreprocessorFilter, PreprocessorError};

/// Configuration for preprocessor evaluation
#[derive(Debug, Clone)]
pub struct PreprocessorConfig {
    /// Predefined symbols and their values
    pub symbols: SymbolTable,
    /// Whether to keep preprocessor directives in output (for debugging)
    pub keep_directives: bool,
}

impl PreprocessorConfig {
    /// Create a new configuration with default symbols
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            keep_directives: false,
        }
    }

    /// Create configuration for user-side code generation
    pub fn for_user() -> Self {
        let mut config = Self::new();
        config.symbols.define("KERNEL_USER", SymbolValue::True);
        config.symbols.define("KERNEL_SERVER", SymbolValue::False);
        config
    }

    /// Create configuration for server-side code generation
    pub fn for_server() -> Self {
        let mut config = Self::new();
        config.symbols.define("KERNEL_USER", SymbolValue::False);
        config.symbols.define("KERNEL_SERVER", SymbolValue::True);
        config
    }

    /// Define a symbol with a boolean value
    pub fn define(&mut self, name: &str, value: bool) -> &mut Self {
        self.symbols.define(name, if value { SymbolValue::True } else { SymbolValue::False });
        self
    }

    /// Undefine a symbol
    pub fn undefine(&mut self, name: &str) -> &mut Self {
        self.symbols.undefine(name);
        self
    }
}

impl Default for PreprocessorConfig {
    fn default() -> Self {
        Self::new()
    }
}
