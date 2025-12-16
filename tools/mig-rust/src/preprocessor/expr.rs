//! Preprocessor expression parsing and evaluation
//!
//! Handles expressions like:
//! - KERNEL_USER
//! - !defined(SEQNOS)
//! - !defined(MACH_IPC_DEBUG) || MACH_IPC_DEBUG

use super::symbols::SymbolTable;
#[cfg(test)]
use super::symbols::SymbolValue;

/// Preprocessor expression
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessorExpr {
    /// Symbol reference (e.g., KERNEL_USER)
    Symbol(String),
    /// Constant (0 or 1)
    Constant(bool),
    /// Logical NOT
    Not(Box<PreprocessorExpr>),
    /// Logical OR
    Or(Box<PreprocessorExpr>, Box<PreprocessorExpr>),
    /// Logical AND
    And(Box<PreprocessorExpr>, Box<PreprocessorExpr>),
    /// Defined check
    Defined(String),
}

impl PreprocessorExpr {
    /// Evaluate the expression with a symbol table
    pub fn eval(&self, symbols: &SymbolTable) -> bool {
        match self {
            PreprocessorExpr::Symbol(name) => symbols.get(name).as_bool(),
            PreprocessorExpr::Constant(val) => *val,
            PreprocessorExpr::Not(expr) => !expr.eval(symbols),
            PreprocessorExpr::Or(left, right) => left.eval(symbols) || right.eval(symbols),
            PreprocessorExpr::And(left, right) => left.eval(symbols) && right.eval(symbols),
            PreprocessorExpr::Defined(name) => symbols.is_defined(name),
        }
    }
}

/// Parser for preprocessor expressions
pub struct ExprParser {
    tokens: Vec<String>,
    position: usize,
}

impl ExprParser {
    /// Create a new expression parser from a preprocessor directive line
    pub fn new(line: &str) -> Self {
        // Tokenize the expression
        // Remove leading #if/#ifdef/#ifndef
        let expr_str = line
            .trim()
            .trim_start_matches("#if")
            .trim_start_matches("#ifdef")
            .trim_start_matches("#ifndef")
            .trim();

        let tokens = Self::tokenize(expr_str);

        Self {
            tokens,
            position: 0,
        }
    }

    /// Tokenize an expression string
    fn tokenize(s: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                ' ' | '\t' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                }
                '(' | ')' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push(ch.to_string());
                }
                '|' if chars.peek() == Some(&'|') => {
                    chars.next(); // consume second |
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push("||".to_string());
                }
                '&' if chars.peek() == Some(&'&') => {
                    chars.next(); // consume second &
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push("&&".to_string());
                }
                '!' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push("!".to_string());
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    /// Parse the expression
    pub fn parse(&mut self) -> Result<PreprocessorExpr, String> {
        self.parse_or()
    }

    /// Parse OR expression
    fn parse_or(&mut self) -> Result<PreprocessorExpr, String> {
        let mut left = self.parse_and()?;

        while self.peek() == Some("||") {
            self.advance();
            let right = self.parse_and()?;
            left = PreprocessorExpr::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// Parse AND expression
    fn parse_and(&mut self) -> Result<PreprocessorExpr, String> {
        let mut left = self.parse_unary()?;

        while self.peek() == Some("&&") {
            self.advance();
            let right = self.parse_unary()?;
            left = PreprocessorExpr::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// Parse unary expression (!, defined())
    fn parse_unary(&mut self) -> Result<PreprocessorExpr, String> {
        if self.peek() == Some("!") {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(PreprocessorExpr::Not(Box::new(expr)));
        }

        self.parse_primary()
    }

    /// Parse primary expression (symbol, constant, defined(), parenthesized)
    fn parse_primary(&mut self) -> Result<PreprocessorExpr, String> {
        match self.peek() {
            Some("(") => {
                self.advance();
                let expr = self.parse_or()?;
                if self.peek() != Some(")") {
                    return Err("Expected ')'".to_string());
                }
                self.advance();
                Ok(expr)
            }
            Some("defined") => {
                self.advance();
                if self.peek() == Some("(") {
                    self.advance();
                    let name = self.advance().ok_or("Expected symbol name")?;
                    if self.peek() != Some(")") {
                        return Err("Expected ')' after defined()".to_string());
                    }
                    self.advance();
                    Ok(PreprocessorExpr::Defined(name))
                } else {
                    let name = self.advance().ok_or("Expected symbol name after defined")?;
                    Ok(PreprocessorExpr::Defined(name))
                }
            }
            Some("0") => {
                self.advance();
                Ok(PreprocessorExpr::Constant(false))
            }
            Some("1") => {
                self.advance();
                Ok(PreprocessorExpr::Constant(true))
            }
            Some(token) => {
                let name = token.to_string();
                self.advance();
                Ok(PreprocessorExpr::Symbol(name))
            }
            None => Err("Unexpected end of expression".to_string()),
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(|s| s.as_str())
    }

    fn advance(&mut self) -> Option<String> {
        if self.position < self.tokens.len() {
            let token = self.tokens[self.position].clone();
            self.position += 1;
            Some(token)
        } else {
            None
        }
    }
}

/// Parse a preprocessor directive into an expression
pub fn parse_directive(line: &str) -> Result<PreprocessorExpr, String> {
    let mut parser = ExprParser::new(line);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_symbol() {
        let expr = parse_directive("#if KERNEL_USER").unwrap();
        assert_eq!(expr, PreprocessorExpr::Symbol("KERNEL_USER".to_string()));

        let mut symbols = SymbolTable::new();
        assert!(!expr.eval(&symbols));

        symbols.define("KERNEL_USER", SymbolValue::True);
        assert!(expr.eval(&symbols));
    }

    #[test]
    fn test_parse_defined() {
        let expr = parse_directive("#if defined(SEQNOS)").unwrap();
        assert_eq!(expr, PreprocessorExpr::Defined("SEQNOS".to_string()));

        let mut symbols = SymbolTable::new();
        assert!(!expr.eval(&symbols));

        symbols.define("SEQNOS", SymbolValue::False);
        assert!(expr.eval(&symbols)); // defined, even if false
    }

    #[test]
    fn test_parse_not_defined() {
        let expr = parse_directive("#if !defined(MACH_IPC_DEBUG)").unwrap();
        assert_eq!(
            expr,
            PreprocessorExpr::Not(Box::new(PreprocessorExpr::Defined(
                "MACH_IPC_DEBUG".to_string()
            )))
        );

        let symbols = SymbolTable::new();
        assert!(expr.eval(&symbols)); // not defined = true
    }

    #[test]
    fn test_parse_or_expression() {
        let expr = parse_directive("#if !defined(ADVISORY_PAGEOUT) || ADVISORY_PAGEOUT").unwrap();

        let mut symbols = SymbolTable::new();
        // Not defined -> true || false = true
        assert!(expr.eval(&symbols));

        // Defined as false -> false || false = false
        symbols.define("ADVISORY_PAGEOUT", SymbolValue::False);
        assert!(!expr.eval(&symbols));

        // Defined as true -> false || true = true
        symbols.define("ADVISORY_PAGEOUT", SymbolValue::True);
        assert!(expr.eval(&symbols));
    }

    #[test]
    fn test_tokenize() {
        let tokens = ExprParser::tokenize("!defined(FOO) || BAR");
        assert_eq!(tokens, vec!["!", "defined", "(", "FOO", ")", "||", "BAR"]);

        let tokens = ExprParser::tokenize("A && B");
        assert_eq!(tokens, vec!["A", "&&", "B"]);
    }
}
