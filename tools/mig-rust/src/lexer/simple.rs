/// Simplified lexer implementation to get started

use super::tokens::{Keyword, Symbol, Token};

pub struct SimpleLexer {
    input: String,
    position: usize,
    line: usize,
}

impl SimpleLexer {
    pub fn new(input: String) -> Self {
        Self {
            input,
            position: 0,
            line: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while self.position < self.input.len() {
            self.skip_whitespace();
            if self.position >= self.input.len() {
                break;
            }

            // Skip comments
            if self.peek_str("//") {
                self.skip_line_comment();
                continue;
            }
            if self.peek_str("/*") {
                self.skip_block_comment()?;
                continue;
            }

            // Preprocessor directive
            if self.current_char() == '#' {
                tokens.push(Token::Preprocessor(self.read_preprocessor()));
                continue;
            }

            // String literals
            if self.current_char() == '"' {
                tokens.push(Token::String(self.read_string()?));
                continue;
            }

            // Numbers
            if self.current_char().is_ascii_digit() {
                tokens.push(Token::Number(self.read_number()));
                continue;
            }

            // Symbols
            if let Some(symbol) = self.try_read_symbol() {
                tokens.push(Token::Symbol(symbol));
                continue;
            }

            // Keywords or identifiers
            if self.current_char().is_ascii_alphabetic() || self.current_char() == '_' {
                let word = self.read_word();
                if let Some(keyword) = self.try_keyword(&word) {
                    tokens.push(Token::Keyword(keyword));
                } else {
                    tokens.push(Token::Identifier(word));
                }
                continue;
            }

            return Err(format!("Unexpected character '{}' at line {}", self.current_char(), self.line));
        }

        Ok(tokens)
    }

    fn current_char(&self) -> char {
        self.input[self.position..].chars().next().unwrap_or('\0')
    }

    fn peek_str(&self, s: &str) -> bool {
        self.input[self.position..].starts_with(s)
    }

    fn advance(&mut self) -> char {
        let c = self.current_char();
        if c == '\n' {
            self.line += 1;
        }
        self.position += c.len_utf8();
        c
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.current_char().is_whitespace() {
            self.advance();
        }
    }

    fn skip_line_comment(&mut self) {
        while self.position < self.input.len() && self.current_char() != '\n' {
            self.advance();
        }
        if self.current_char() == '\n' {
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), String> {
        self.advance(); // /
        self.advance(); // *

        while self.position < self.input.len() {
            if self.peek_str("*/") {
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }

        Err("Unterminated block comment".to_string())
    }

    fn read_preprocessor(&mut self) -> String {
        let mut result = String::new();
        while self.position < self.input.len() && self.current_char() != '\n' {
            result.push(self.advance());
        }
        result
    }

    fn read_string(&mut self) -> Result<String, String> {
        let mut result = String::new();
        self.advance(); // opening "

        while self.position < self.input.len() {
            let c = self.current_char();

            if c == '"' {
                self.advance();
                return Ok(result);
            }

            if c == '\\' {
                self.advance();
                let escaped = self.advance();
                result.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    _ => escaped,
                });
            } else {
                result.push(self.advance());
            }
        }

        Err("Unterminated string".to_string())
    }

    fn read_number(&mut self) -> u32 {
        let mut result = String::new();
        while self.position < self.input.len() && self.current_char().is_ascii_digit() {
            result.push(self.advance());
        }
        result.parse().unwrap()
    }

    fn read_word(&mut self) -> String {
        let mut result = String::new();
        while self.position < self.input.len() {
            let c = self.current_char();
            if c.is_ascii_alphanumeric() || c == '_' {
                result.push(self.advance());
            } else {
                break;
            }
        }
        result
    }

    fn try_read_symbol(&mut self) -> Option<Symbol> {
        let symbol = match self.current_char() {
            ':' => Symbol::Colon,
            ';' => Symbol::Semicolon,
            ',' => Symbol::Comma,
            '(' => Symbol::LeftParen,
            ')' => Symbol::RightParen,
            '[' => Symbol::LeftBracket,
            ']' => Symbol::RightBracket,
            '{' => Symbol::LeftBrace,
            '}' => Symbol::RightBrace,
            '=' => Symbol::Equals,
            '*' => Symbol::Star,
            '^' => Symbol::Caret,
            '~' => Symbol::Tilde,
            '+' => Symbol::Plus,
            '-' => Symbol::Minus,
            '/' => Symbol::Slash,
            '|' => Symbol::Pipe,
            '&' => Symbol::Ampersand,
            '<' => Symbol::LessThan,
            '>' => Symbol::GreaterThan,
            '.' => Symbol::Dot,
            _ => return None,
        };

        self.advance();
        Some(symbol)
    }

    fn try_keyword(&self, word: &str) -> Option<Keyword> {
        match word.to_lowercase().as_str() {
            "subsystem" => Some(Keyword::Subsystem),
            "kerneluser" => Some(Keyword::KernelUser),
            "kernelserver" => Some(Keyword::KernelServer),
            "routine" => Some(Keyword::Routine),
            "simpleroutine" => Some(Keyword::SimpleRoutine),
            "procedure" => Some(Keyword::Procedure),
            "simpleprocedure" => Some(Keyword::SimpleProcedure),
            "type" => Some(Keyword::Type),
            "array" => Some(Keyword::Array),
            "of" => Some(Keyword::Of),
            "struct" => Some(Keyword::Struct),
            "c_string" => Some(Keyword::CString),
            "ctype" => Some(Keyword::CType),
            "cusertype" => Some(Keyword::CUserType),
            "cservertype" => Some(Keyword::CServerType),
            "intran" => Some(Keyword::InTran),
            "intranpayload" => Some(Keyword::InTranPayload),
            "outtran" => Some(Keyword::OutTran),
            "destructor" => Some(Keyword::Destructor),
            "import" => Some(Keyword::Import),
            "uimport" => Some(Keyword::UImport),
            "simport" => Some(Keyword::SImport),
            "rcsid" => Some(Keyword::RCSId),
            "skip" => Some(Keyword::Skip),
            "serverprefix" => Some(Keyword::ServerPrefix),
            "userprefix" => Some(Keyword::UserPrefix),
            "in" => Some(Keyword::In),
            "out" => Some(Keyword::Out),
            "inout" => Some(Keyword::InOut),
            "requestport" => Some(Keyword::RequestPort),
            "replyport" => Some(Keyword::ReplyPort),
            "sreplyport" => Some(Keyword::SReplyPort),
            "ureplyport" => Some(Keyword::UReplyPort),
            "waittime" => Some(Keyword::WaitTime),
            "msgoption" => Some(Keyword::MsgOption),
            "msgseqno" => Some(Keyword::MsgSeqno),
            "islong" => Some(Keyword::IsLong),
            "isnotlong" => Some(Keyword::IsNotLong),
            "dealloc" => Some(Keyword::Dealloc),
            "notdealloc" => Some(Keyword::NotDealloc),
            "servercopy" => Some(Keyword::ServerCopy),
            "countinout" => Some(Keyword::CountInOut),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokenize() {
        let mut lexer = SimpleLexer::new("subsystem test 2000;".to_string());
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0], Token::Keyword(Keyword::Subsystem)));
        assert!(matches!(tokens[1], Token::Identifier(_)));
        assert!(matches!(tokens[2], Token::Number(2000)));
        assert!(matches!(tokens[3], Token::Symbol(Symbol::Semicolon)));
    }

    #[test]
    fn test_comments() {
        let mut lexer = SimpleLexer::new("// comment\nsubsystem /* block */ test;".to_string());
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 3);
    }
}
