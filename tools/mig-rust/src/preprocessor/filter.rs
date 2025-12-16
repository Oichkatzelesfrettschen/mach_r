//! Token stream filtering based on preprocessor conditionals

use super::expr::{parse_directive, PreprocessorExpr};
use super::symbols::SymbolTable;
use crate::lexer::tokens::Token;

/// Preprocessor error
#[derive(Debug, Clone)]
pub struct PreprocessorError {
    pub message: String,
}

impl std::fmt::Display for PreprocessorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Preprocessor error: {}", self.message)
    }
}

impl std::error::Error for PreprocessorError {}

/// State of a conditional block
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockState {
    /// Block is active (tokens should be included)
    Active,
    /// Block is inactive (tokens should be filtered out)
    Inactive,
    /// Block was active earlier, now in else branch
    WasActive,
}

/// Conditional block on the stack
#[derive(Debug, Clone)]
struct ConditionalBlock {
    /// State of this block
    state: BlockState,
    /// Line number where block started
    _line: usize,
}

/// Filters tokens based on preprocessor directives
pub struct PreprocessorFilter {
    /// Symbol table for evaluation
    symbols: SymbolTable,
    /// Stack of conditional blocks
    block_stack: Vec<ConditionalBlock>,
    /// Current line number (for error reporting)
    line_number: usize,
}

impl PreprocessorFilter {
    /// Create a new preprocessor filter
    pub fn new(symbols: SymbolTable) -> Self {
        Self {
            symbols,
            block_stack: Vec::new(),
            line_number: 0,
        }
    }

    /// Check if we're currently in an active block
    fn is_active(&self) -> bool {
        self.block_stack
            .iter()
            .all(|block| block.state == BlockState::Active)
    }

    /// Process tokens through the preprocessor
    pub fn filter(&mut self, tokens: Vec<Token>) -> Result<Vec<Token>, PreprocessorError> {
        let mut output = Vec::new();

        for token in tokens {
            match token {
                Token::Preprocessor(ref directive) => {
                    self.process_directive(directive)?;
                    // Don't include preprocessor directives in output
                }
                _ => {
                    // Only include token if we're in an active block
                    if self.is_active() {
                        output.push(token);
                    }
                }
            }
        }

        // Check for unclosed blocks
        if !self.block_stack.is_empty() {
            return Err(PreprocessorError {
                message: format!(
                    "{} unclosed conditional block(s) at end of file",
                    self.block_stack.len()
                ),
            });
        }

        Ok(output)
    }

    /// Process a single preprocessor directive
    fn process_directive(&mut self, directive: &str) -> Result<(), PreprocessorError> {
        let trimmed = directive.trim();

        if trimmed.starts_with("#if") && !trimmed.starts_with("#endif") {
            self.process_if(directive)?;
        } else if trimmed.starts_with("#else") {
            self.process_else()?;
        } else if trimmed.starts_with("#endif") {
            self.process_endif()?;
        } else if trimmed.starts_with("#define") || trimmed.starts_with("#undef") {
            // Ignore #define and #undef for now
            // In a full preprocessor, these would modify the symbol table
        }
        // Ignore other directives like #include (already handled by parser)

        Ok(())
    }

    /// Process #if, #ifdef, #ifndef
    fn process_if(&mut self, directive: &str) -> Result<(), PreprocessorError> {
        let trimmed = directive.trim();

        // Parse the expression
        let expr = if trimmed.starts_with("#ifdef") {
            // #ifdef FOO -> defined(FOO)
            let symbol = trimmed
                .trim_start_matches("#ifdef")
                .trim()
                .split_whitespace()
                .next()
                .ok_or_else(|| PreprocessorError {
                    message: "Expected symbol after #ifdef".to_string(),
                })?;
            PreprocessorExpr::Defined(symbol.to_string())
        } else if trimmed.starts_with("#ifndef") {
            // #ifndef FOO -> !defined(FOO)
            let symbol = trimmed
                .trim_start_matches("#ifndef")
                .trim()
                .split_whitespace()
                .next()
                .ok_or_else(|| PreprocessorError {
                    message: "Expected symbol after #ifndef".to_string(),
                })?;
            PreprocessorExpr::Not(Box::new(PreprocessorExpr::Defined(symbol.to_string())))
        } else {
            // #if <expr>
            parse_directive(directive).map_err(|e| PreprocessorError {
                message: format!("Failed to parse expression: {}", e),
            })?
        };

        // Evaluate the expression
        let result = expr.eval(&self.symbols);

        // Determine the state of this block
        let state = if self.is_active() {
            // Parent is active, use evaluation result
            if result {
                BlockState::Active
            } else {
                BlockState::Inactive
            }
        } else {
            // Parent is inactive, this block is also inactive
            BlockState::Inactive
        };

        self.block_stack.push(ConditionalBlock {
            state,
            _line: self.line_number,
        });

        Ok(())
    }

    /// Process #else
    fn process_else(&mut self) -> Result<(), PreprocessorError> {
        if self.block_stack.is_empty() {
            return Err(PreprocessorError {
                message: "#else without matching #if".to_string(),
            });
        }

        let block_index = self.block_stack.len() - 1;
        let current_state = self.block_stack[block_index].state;

        let new_state = match current_state {
            BlockState::Active => {
                // Was active, now inactive
                BlockState::WasActive
            }
            BlockState::Inactive => {
                // Check if parent blocks are all active
                let parent_active = self.block_stack[..block_index]
                    .iter()
                    .all(|b| b.state == BlockState::Active);

                if parent_active {
                    // Parent is active and we were inactive, now active
                    BlockState::Active
                } else {
                    // Parent inactive, stay inactive
                    BlockState::Inactive
                }
            }
            BlockState::WasActive => {
                // Already had an else, stay inactive
                // (Note: real preprocessor would error on multiple #else)
                BlockState::WasActive
            }
        };

        self.block_stack[block_index].state = new_state;
        Ok(())
    }

    /// Process #endif
    fn process_endif(&mut self) -> Result<(), PreprocessorError> {
        self.block_stack.pop().ok_or_else(|| PreprocessorError {
            message: "#endif without matching #if".to_string(),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokens::{Keyword, Symbol, Token};

    fn make_tokens() -> Vec<Token> {
        vec![
            Token::Preprocessor("#if KERNEL_USER".to_string()),
            Token::Keyword(Keyword::In),
            Token::Preprocessor("#else".to_string()),
            Token::Keyword(Keyword::Out),
            Token::Preprocessor("#endif".to_string()),
            Token::Symbol(Symbol::Semicolon),
        ]
    }

    #[test]
    fn test_filter_if_true() {
        let mut symbols = SymbolTable::new();
        symbols.define("KERNEL_USER", super::super::symbols::SymbolValue::True);

        let mut filter = PreprocessorFilter::new(symbols);
        let result = filter.filter(make_tokens()).unwrap();

        assert_eq!(result.len(), 2); // 'in' and ';'
        assert!(matches!(result[0], Token::Keyword(Keyword::In)));
    }

    #[test]
    fn test_filter_if_false() {
        let symbols = SymbolTable::new(); // KERNEL_USER undefined = false

        let mut filter = PreprocessorFilter::new(symbols);
        let result = filter.filter(make_tokens()).unwrap();

        assert_eq!(result.len(), 2); // 'out' and ';'
        assert!(matches!(result[0], Token::Keyword(Keyword::Out)));
    }

    #[test]
    fn test_nested_conditionals() {
        let tokens = vec![
            Token::Preprocessor("#if FOO".to_string()),
            Token::Keyword(Keyword::In),
            Token::Preprocessor("#if BAR".to_string()),
            Token::Keyword(Keyword::Out),
            Token::Preprocessor("#endif".to_string()),
            Token::Preprocessor("#endif".to_string()),
        ];

        let mut symbols = SymbolTable::new();
        symbols.define("FOO", super::super::symbols::SymbolValue::True);
        symbols.define("BAR", super::super::symbols::SymbolValue::True);

        let mut filter = PreprocessorFilter::new(symbols);
        let result = filter.filter(tokens).unwrap();

        assert_eq!(result.len(), 2); // Both 'in' and 'out'
    }

    #[test]
    fn test_unclosed_block() {
        let tokens = vec![
            Token::Preprocessor("#if KERNEL_USER".to_string()),
            Token::Keyword(Keyword::In),
        ];

        let symbols = SymbolTable::new();
        let mut filter = PreprocessorFilter::new(symbols);
        let result = filter.filter(tokens);

        assert!(result.is_err());
    }
}
