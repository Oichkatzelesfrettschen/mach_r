//! Symbol table for preprocessor defines

use std::collections::HashMap;

/// Value of a preprocessor symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolValue {
    /// Symbol is undefined
    Undefined,
    /// Symbol is defined as false (0)
    False,
    /// Symbol is defined as true (non-zero)
    True,
}

impl SymbolValue {
    /// Convert to boolean (undefined = false for evaluation)
    pub fn as_bool(self) -> bool {
        match self {
            SymbolValue::Undefined => false,
            SymbolValue::False => false,
            SymbolValue::True => true,
        }
    }

    /// Check if symbol is defined (regardless of value)
    pub fn is_defined(self) -> bool {
        !matches!(self, SymbolValue::Undefined)
    }
}

/// Symbol table mapping symbol names to values
#[derive(Debug, Clone)]
pub struct SymbolTable {
    symbols: HashMap<String, SymbolValue>,
}

impl SymbolTable {
    /// Create a new empty symbol table
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    /// Define a symbol with a value
    pub fn define(&mut self, name: &str, value: SymbolValue) {
        self.symbols.insert(name.to_string(), value);
    }

    /// Undefine a symbol
    pub fn undefine(&mut self, name: &str) {
        self.symbols.remove(name);
    }

    /// Get the value of a symbol
    pub fn get(&self, name: &str) -> SymbolValue {
        self.symbols
            .get(name)
            .copied()
            .unwrap_or(SymbolValue::Undefined)
    }

    /// Check if a symbol is defined
    pub fn is_defined(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }

    /// Get all defined symbols
    pub fn defined_symbols(&self) -> Vec<String> {
        self.symbols.keys().cloned().collect()
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table() {
        let mut table = SymbolTable::new();

        assert_eq!(table.get("FOO"), SymbolValue::Undefined);
        assert!(!table.is_defined("FOO"));

        table.define("FOO", SymbolValue::True);
        assert_eq!(table.get("FOO"), SymbolValue::True);
        assert!(table.is_defined("FOO"));

        table.define("BAR", SymbolValue::False);
        assert_eq!(table.get("BAR"), SymbolValue::False);
        assert!(table.is_defined("BAR"));

        table.undefine("FOO");
        assert_eq!(table.get("FOO"), SymbolValue::Undefined);
        assert!(!table.is_defined("FOO"));
    }

    #[test]
    fn test_symbol_value_as_bool() {
        assert!(!SymbolValue::Undefined.as_bool());
        assert!(!SymbolValue::False.as_bool());
        assert!(SymbolValue::True.as_bool());
    }

    #[test]
    fn test_symbol_value_is_defined() {
        assert!(!SymbolValue::Undefined.is_defined());
        assert!(SymbolValue::False.is_defined());
        assert!(SymbolValue::True.is_defined());
    }
}
