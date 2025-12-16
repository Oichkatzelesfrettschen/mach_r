pub mod ast;

use crate::lexer::tokens::{Keyword, Symbol, Token};
use ast::*;

/// Parser for MIG .defs files
pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        // Filter out comments and preprocessor directives
        let tokens: Vec<Token> = tokens
            .into_iter()
            .filter(|t| !matches!(t, Token::Comment | Token::Preprocessor(_)))
            .collect();

        Self {
            tokens,
            position: 0,
        }
    }

    /// Parse a complete .defs file
    pub fn parse(&mut self) -> Result<Subsystem, ParseError> {
        // First statement must be subsystem declaration
        let subsystem_decl = self.parse_subsystem()?;

        let mut statements = Vec::new();

        while !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        Ok(Subsystem {
            name: subsystem_decl.name,
            base: subsystem_decl.base,
            modifiers: subsystem_decl.modifiers,
            statements,
        })
    }

    /// Parse subsystem declaration: subsystem [mods] name base;
    fn parse_subsystem(&mut self) -> Result<SubsystemDecl, ParseError> {
        self.expect_keyword(Keyword::Subsystem)?;

        let mut modifiers = Vec::new();

        // Skip preprocessor directives and check for optional modifiers
        loop {
            // Skip any preprocessor directives
            while matches!(self.peek(), Some(Token::Preprocessor(_))) {
                self.advance();
            }

            // Check for modifiers
            if let Some(Token::Keyword(kw)) = self.peek() {
                match kw {
                    Keyword::KernelUser => {
                        modifiers.push(SubsystemMod::KernelUser);
                        self.advance();
                    }
                    Keyword::KernelServer => {
                        modifiers.push(SubsystemMod::KernelServer);
                        self.advance();
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        // Skip preprocessor directives before name
        while matches!(self.peek(), Some(Token::Preprocessor(_))) {
            self.advance();
        }

        let name = self.expect_identifier()?;
        let base = self.expect_number()?;
        self.expect_symbol(Symbol::Semicolon)?;

        Ok(SubsystemDecl {
            name,
            base,
            modifiers,
        })
    }

    /// Parse a statement
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(Token::Keyword(Keyword::Type)) => Ok(Statement::TypeDecl(self.parse_type_decl()?)),
            Some(Token::Keyword(Keyword::Routine)) => Ok(Statement::Routine(self.parse_routine()?)),
            Some(Token::Keyword(Keyword::SimpleRoutine)) => {
                Ok(Statement::SimpleRoutine(self.parse_simple_routine()?))
            }
            Some(Token::Keyword(Keyword::Import)) => {
                Ok(Statement::Import(self.parse_import(ImportKind::Normal)?))
            }
            Some(Token::Keyword(Keyword::UImport)) => {
                Ok(Statement::Import(self.parse_import(ImportKind::User)?))
            }
            Some(Token::Keyword(Keyword::SImport)) => {
                Ok(Statement::Import(self.parse_import(ImportKind::Server)?))
            }
            Some(Token::Keyword(Keyword::Skip)) => {
                self.advance();
                self.expect_symbol(Symbol::Semicolon)?;
                Ok(Statement::Skip)
            }
            Some(Token::Keyword(Keyword::ServerPrefix)) => {
                self.advance();
                let prefix = self.expect_identifier()?;
                self.expect_symbol(Symbol::Semicolon)?;
                Ok(Statement::ServerPrefix(prefix))
            }
            Some(Token::Keyword(Keyword::UserPrefix)) => {
                self.advance();
                let prefix = self.expect_identifier()?;
                self.expect_symbol(Symbol::Semicolon)?;
                Ok(Statement::UserPrefix(prefix))
            }
            // Skip preprocessor directives (e.g., #include)
            Some(Token::Preprocessor(_)) => {
                self.advance();
                self.parse_statement() // Continue to next statement
            }
            // Skip comments
            Some(Token::Comment) => {
                self.advance();
                self.parse_statement() // Continue to next statement
            }
            _ => Err(self.error("Expected statement (type, routine, import, skip, prefix)")),
        }
    }

    /// Parse type declaration: type name = typespec [annotations];
    fn parse_type_decl(&mut self) -> Result<TypeDecl, ParseError> {
        self.expect_keyword(Keyword::Type)?;
        let name = self.expect_identifier()?;
        self.expect_symbol(Symbol::Equals)?;
        let spec = self.parse_type_spec()?;

        // TODO: Parse annotations (ctype, intran, etc.)
        let annotations = TypeAnnotations::default();

        self.expect_symbol(Symbol::Semicolon)?;

        Ok(TypeDecl {
            name,
            spec,
            annotations,
        })
    }

    /// Parse type specification
    fn parse_type_spec(&mut self) -> Result<TypeSpec, ParseError> {
        // Check for array
        if matches!(self.peek(), Some(Token::Keyword(Keyword::Array))) {
            return self.parse_array_type();
        }

        // Check for struct
        if matches!(self.peek(), Some(Token::Keyword(Keyword::Struct))) {
            return self.parse_struct_type();
        }

        // Check for pointer (^)
        if matches!(self.peek(), Some(Token::Symbol(Symbol::Caret))) {
            self.advance();
            let inner = Box::new(self.parse_type_spec()?);
            return Ok(TypeSpec::Pointer(inner));
        }

        // Check for c_string
        if matches!(self.peek(), Some(Token::Keyword(Keyword::CString))) {
            return self.parse_cstring_type();
        }

        // Otherwise, it's a basic type (identifier)
        let name = self.expect_identifier()?;
        Ok(TypeSpec::Basic(name))
    }

    /// Parse array type: array[size] of element
    fn parse_array_type(&mut self) -> Result<TypeSpec, ParseError> {
        self.expect_keyword(Keyword::Array)?;
        self.expect_symbol(Symbol::LeftBracket)?;

        let size = if matches!(self.peek(), Some(Token::Symbol(Symbol::Star))) {
            self.advance();
            // Check for max: [*:max]
            if matches!(self.peek(), Some(Token::Symbol(Symbol::Colon))) {
                self.advance();
                let max = self.expect_number()?;
                ArraySize::VariableWithMax(max)
            } else {
                ArraySize::Variable
            }
        } else if matches!(self.peek(), Some(Token::Symbol(Symbol::RightBracket))) {
            ArraySize::Variable
        } else {
            let n = self.expect_number()?;
            ArraySize::Fixed(n)
        };

        self.expect_symbol(Symbol::RightBracket)?;
        self.expect_keyword(Keyword::Of)?;

        let element = Box::new(self.parse_type_spec()?);

        Ok(TypeSpec::Array { size, element })
    }

    /// Parse struct type
    fn parse_struct_type(&mut self) -> Result<TypeSpec, ParseError> {
        self.expect_keyword(Keyword::Struct)?;

        // struct[count] of type  OR  struct { fields }
        if matches!(self.peek(), Some(Token::Symbol(Symbol::LeftBracket))) {
            self.advance();
            let count = self.expect_number()?;
            self.expect_symbol(Symbol::RightBracket)?;
            self.expect_keyword(Keyword::Of)?;
            let element = Box::new(self.parse_type_spec()?);
            return Ok(TypeSpec::StructArray { count, element });
        }

        // struct { ... }
        self.expect_symbol(Symbol::LeftBrace)?;
        let mut fields = Vec::new();

        while !matches!(self.peek(), Some(Token::Symbol(Symbol::RightBrace))) {
            let field_type = self.parse_type_spec()?;
            let field_name = self.expect_identifier()?;
            self.expect_symbol(Symbol::Semicolon)?;
            fields.push(StructField {
                name: field_name,
                field_type,
            });
        }

        self.expect_symbol(Symbol::RightBrace)?;
        Ok(TypeSpec::Struct(fields))
    }

    /// Parse c_string type
    fn parse_cstring_type(&mut self) -> Result<TypeSpec, ParseError> {
        self.expect_keyword(Keyword::CString)?;
        self.expect_symbol(Symbol::LeftBracket)?;

        let max_size = if matches!(self.peek(), Some(Token::Symbol(Symbol::Star))) {
            self.advance();
            if matches!(self.peek(), Some(Token::Symbol(Symbol::Colon))) {
                self.advance();
                Some(self.expect_number()?)
            } else {
                None
            }
        } else {
            Some(self.expect_number()?)
        };

        self.expect_symbol(Symbol::RightBracket)?;

        Ok(TypeSpec::CString {
            max_size,
            varying: max_size.is_none(),
        })
    }

    /// Parse routine: routine name(args);
    fn parse_routine(&mut self) -> Result<Routine, ParseError> {
        self.expect_keyword(Keyword::Routine)?;
        let name = self.expect_identifier()?;
        let args = self.parse_arguments()?;
        self.expect_symbol(Symbol::Semicolon)?;

        Ok(Routine {
            name,
            kind: RoutineKind::Routine,
            args,
        })
    }

    /// Parse simple routine
    fn parse_simple_routine(&mut self) -> Result<Routine, ParseError> {
        self.expect_keyword(Keyword::SimpleRoutine)?;
        let name = self.expect_identifier()?;
        let args = self.parse_arguments()?;
        self.expect_symbol(Symbol::Semicolon)?;

        Ok(Routine {
            name,
            kind: RoutineKind::SimpleRoutine,
            args,
        })
    }

    /// Parse arguments: (arg1; arg2; ...)
    fn parse_arguments(&mut self) -> Result<Vec<Argument>, ParseError> {
        self.expect_symbol(Symbol::LeftParen)?;

        let mut args = Vec::new();

        while !matches!(self.peek(), Some(Token::Symbol(Symbol::RightParen))) {
            args.push(self.parse_argument()?);

            // Semicolon separates arguments
            if matches!(self.peek(), Some(Token::Symbol(Symbol::Semicolon))) {
                self.advance();
            }
        }

        self.expect_symbol(Symbol::RightParen)?;

        Ok(args)
    }

    /// Parse single argument: [direction] name : type [flags]
    fn parse_argument(&mut self) -> Result<Argument, ParseError> {
        // Parse direction
        let direction = self.parse_direction()?;

        let name = self.expect_identifier()?;
        self.expect_symbol(Symbol::Colon)?;
        let arg_type = self.parse_type_spec()?;

        // Parse optional type qualifiers (e.g., "const", "dealloc")
        let mut flags = IpcFlags::default();
        while matches!(self.peek(), Some(Token::Symbol(Symbol::Comma))) {
            self.advance(); // consume comma

            // Parse qualifier keyword
            match self.peek() {
                Some(Token::Keyword(Keyword::Const)) => {
                    self.advance();
                    // Mark as const (we'll need to add this to IpcFlags)
                }
                Some(Token::Keyword(Keyword::Dealloc)) => {
                    self.advance();
                    flags.dealloc = Some(DeallocMode::Dealloc);
                }
                Some(Token::Keyword(Keyword::NotDealloc)) => {
                    self.advance();
                    flags.dealloc = Some(DeallocMode::NotDealloc);
                }
                Some(Token::Keyword(Keyword::ServerCopy)) => {
                    self.advance();
                    flags.server_copy = true;
                }
                Some(Token::Keyword(Keyword::CountInOut)) => {
                    self.advance();
                    flags.count_in_out = true;
                }
                _ => break, // Unknown qualifier, stop parsing qualifiers
            }
        }

        Ok(Argument {
            name,
            direction,
            arg_type,
            flags,
        })
    }

    /// Parse argument direction
    fn parse_direction(&mut self) -> Result<Direction, ParseError> {
        match self.peek() {
            Some(Token::Keyword(Keyword::In)) => {
                self.advance();
                Ok(Direction::In)
            }
            Some(Token::Keyword(Keyword::Out)) => {
                self.advance();
                Ok(Direction::Out)
            }
            Some(Token::Keyword(Keyword::InOut)) => {
                self.advance();
                Ok(Direction::InOut)
            }
            Some(Token::Keyword(Keyword::RequestPort)) => {
                self.advance();
                Ok(Direction::RequestPort)
            }
            Some(Token::Keyword(Keyword::ReplyPort)) => {
                self.advance();
                Ok(Direction::ReplyPort)
            }
            Some(Token::Keyword(Keyword::SReplyPort)) => {
                self.advance();
                Ok(Direction::SReplyPort)
            }
            Some(Token::Keyword(Keyword::UReplyPort)) => {
                self.advance();
                Ok(Direction::UReplyPort)
            }
            Some(Token::Keyword(Keyword::WaitTime)) => {
                self.advance();
                Ok(Direction::WaitTime)
            }
            Some(Token::Keyword(Keyword::MsgOption)) => {
                self.advance();
                Ok(Direction::MsgOption)
            }
            Some(Token::Keyword(Keyword::MsgSeqno)) => {
                self.advance();
                Ok(Direction::MsgSeqno)
            }
            // Default to In if no direction specified
            _ => Ok(Direction::In),
        }
    }

    /// Parse import: import "file"; or import <file>;
    fn parse_import(&mut self, kind: ImportKind) -> Result<Import, ParseError> {
        self.advance(); // Skip import keyword

        // Check if it's angle bracket import <file> or quoted "file"
        let file = if matches!(self.peek(), Some(Token::Symbol(Symbol::LessThan))) {
            self.advance(); // Skip <

            // Build path from tokens until we hit >
            let mut path = String::new();
            loop {
                match self.peek() {
                    Some(Token::Symbol(Symbol::GreaterThan)) => break,
                    Some(Token::Identifier(name)) => {
                        path.push_str(name);
                        self.advance();
                    }
                    Some(Token::Symbol(Symbol::Slash)) => {
                        path.push('/');
                        self.advance();
                    }
                    Some(Token::Symbol(Symbol::Dot)) => {
                        path.push('.');
                        self.advance();
                    }
                    Some(Token::Number(n)) => {
                        path.push_str(&n.to_string());
                        self.advance();
                    }
                    _ => return Err(self.error("Unexpected token in import path")),
                }
            }

            self.expect_symbol(Symbol::GreaterThan)?;
            path
        } else {
            self.expect_string()?
        };

        self.expect_symbol(Symbol::Semicolon)?;

        Ok(Import { kind, file })
    }

    // Helper methods

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        self.position += 1;
        token
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    fn expect_keyword(&mut self, expected: Keyword) -> Result<(), ParseError> {
        match self.advance() {
            Some(Token::Keyword(kw)) if *kw == expected => Ok(()),
            _ => Err(self.error(&format!("Expected keyword {}", expected))),
        }
    }

    fn expect_symbol(&mut self, expected: Symbol) -> Result<(), ParseError> {
        match self.advance() {
            Some(Token::Symbol(sym)) if *sym == expected => Ok(()),
            _ => Err(self.error(&format!("Expected symbol {}", expected))),
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let token = self.advance().cloned();
        match token {
            Some(Token::Identifier(s)) => Ok(s),
            other => Err(self.error(&format!("Expected identifier, found {:?}", other))),
        }
    }

    fn expect_number(&mut self) -> Result<u32, ParseError> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(*n),
            _ => Err(self.error("Expected number")),
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        match self.advance() {
            Some(Token::String(s)) => Ok(s.clone()),
            _ => Err(self.error("Expected string")),
        }
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            position: self.position,
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub position: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

struct SubsystemDecl {
    name: String,
    base: u32,
    modifiers: Vec<SubsystemMod>,
}
