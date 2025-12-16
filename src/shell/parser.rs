//! Command line parser for POSIX shell
//! Handles tokenization, quoting, and basic command parsing

use super::Command;
use heapless::{String, Vec};

/// Token types for shell parsing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenType {
    Word,
    Pipe,
    Redirect,
    Background,
    Semicolon,
}

/// Shell token
#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String<256>,
}

impl Token {
    /// Create a new token
    pub fn new(token_type: TokenType, value: &str) -> Result<Self, &'static str> {
        let mut token_value = String::new();
        token_value
            .push_str(value)
            .map_err(|_| "Token value too long")?;

        Ok(Self {
            token_type,
            value: token_value,
        })
    }
}

/// Parse command line into tokens
pub fn tokenize(line: &str) -> Result<Vec<Token, 128>, &'static str> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current_word = String::<256>::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';

    while let Some(ch) = chars.next() {
        match ch {
            // Handle quotes
            '"' | '\'' => {
                if in_quotes && ch == quote_char {
                    in_quotes = false;
                    quote_char = '\0';
                } else if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                }
            }

            // Handle whitespace
            ' ' | '\t' | '\n' => {
                if in_quotes {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                } else if !current_word.is_empty() {
                    tokens
                        .push(Token::new(TokenType::Word, &current_word)?)
                        .map_err(|_| "Too many tokens")?;
                    current_word.clear();
                }
            }

            // Handle special characters
            '|' => {
                if in_quotes {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                } else {
                    if !current_word.is_empty() {
                        tokens
                            .push(Token::new(TokenType::Word, &current_word)?)
                            .map_err(|_| "Too many tokens")?;
                        current_word.clear();
                    }
                    tokens
                        .push(Token::new(TokenType::Pipe, "|")?)
                        .map_err(|_| "Too many tokens")?;
                }
            }

            '&' => {
                if in_quotes {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                } else {
                    if !current_word.is_empty() {
                        tokens
                            .push(Token::new(TokenType::Word, &current_word)?)
                            .map_err(|_| "Too many tokens")?;
                        current_word.clear();
                    }
                    tokens
                        .push(Token::new(TokenType::Background, "&")?)
                        .map_err(|_| "Too many tokens")?;
                }
            }

            ';' => {
                if in_quotes {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                } else {
                    if !current_word.is_empty() {
                        tokens
                            .push(Token::new(TokenType::Word, &current_word)?)
                            .map_err(|_| "Too many tokens")?;
                        current_word.clear();
                    }
                    tokens
                        .push(Token::new(TokenType::Semicolon, ";")?)
                        .map_err(|_| "Too many tokens")?;
                }
            }

            '<' | '>' => {
                if in_quotes {
                    current_word.push(ch).map_err(|_| "Token too long")?;
                } else {
                    if !current_word.is_empty() {
                        tokens
                            .push(Token::new(TokenType::Word, &current_word)?)
                            .map_err(|_| "Too many tokens")?;
                        current_word.clear();
                    }

                    // Handle >> redirection
                    if ch == '>' && chars.peek() == Some(&'>') {
                        chars.next(); // consume second '>'
                        tokens
                            .push(Token::new(TokenType::Redirect, ">>")?)
                            .map_err(|_| "Too many tokens")?;
                    } else {
                        let redirect = if ch == '<' { "<" } else { ">" };
                        tokens
                            .push(Token::new(TokenType::Redirect, redirect)?)
                            .map_err(|_| "Too many tokens")?;
                    }
                }
            }

            // Handle backslash escaping
            '\\' => {
                if let Some(next_ch) = chars.next() {
                    current_word.push(next_ch).map_err(|_| "Token too long")?;
                }
            }

            // Regular characters
            _ => {
                current_word.push(ch).map_err(|_| "Token too long")?;
            }
        }
    }

    // Add final word if present
    if !current_word.is_empty() {
        tokens
            .push(Token::new(TokenType::Word, &current_word)?)
            .map_err(|_| "Too many tokens")?;
    }

    // Check for unclosed quotes
    if in_quotes {
        return Err("Unclosed quote");
    }

    Ok(tokens)
}

/// Parse tokens into a command structure
pub fn parse_command(tokens: &[Token]) -> Result<Command, &'static str> {
    let mut command = Command::new();
    let mut token_iter = tokens.iter().peekable();
    let mut expect_filename = None; // For redirection

    for token in token_iter.by_ref() {
        match token.token_type {
            TokenType::Word => {
                if let Some(redirect_type) = expect_filename {
                    // This word is a filename for redirection
                    let mut filename = String::new();
                    filename
                        .push_str(&token.value)
                        .map_err(|_| "Filename too long")?;

                    match redirect_type {
                        "<" => command.stdin_file = Some(filename),
                        ">" | ">>" => command.stdout_file = Some(filename),
                        "2>" => command.stderr_file = Some(filename),
                        _ => return Err("Unknown redirection type"),
                    }
                    expect_filename = None;
                } else if command.program.is_empty() {
                    // First word is the program
                    command
                        .program
                        .push_str(&token.value)
                        .map_err(|_| "Program name too long")?;
                } else {
                    // Subsequent words are arguments
                    let mut arg = String::new();
                    arg.push_str(&token.value)
                        .map_err(|_| "Argument too long")?;
                    command.args.push(arg).map_err(|_| "Too many arguments")?;
                }
            }

            TokenType::Redirect => {
                expect_filename = Some(token.value.as_str());
            }

            TokenType::Background => {
                command.background = true;
            }

            TokenType::Pipe => {
                // TODO: Handle pipes (would need to return multiple commands)
                return Err("Pipes not yet implemented");
            }

            TokenType::Semicolon => {
                // TODO: Handle multiple commands
                return Err("Command sequences not yet implemented");
            }
        }
    }

    if expect_filename.is_some() {
        return Err("Expected filename after redirection");
    }

    if command.program.is_empty() {
        return Err("Empty command");
    }

    Ok(command)
}

/// Parse a complete command line
pub fn parse_command_line(line: &str) -> Result<Command, &'static str> {
    let tokens = tokenize(line)?;
    parse_command(&tokens)
}

/// Expand environment variables in a string
pub fn expand_variables(
    input: &str,
    get_env: impl Fn(&str) -> Option<&str>,
) -> Result<String<512>, &'static str> {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek().is_some() {
            let mut var_name = String::<64>::new();

            // Handle ${VAR} syntax
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                for ch in chars.by_ref() {
                    if ch == '}' {
                        break;
                    }
                    var_name.push(ch).map_err(|_| "Variable name too long")?;
                }
            } else {
                // Handle $VAR syntax
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var_name.push(ch).map_err(|_| "Variable name too long")?;
                        chars.next();
                    } else {
                        break;
                    }
                }
            }

            // Look up variable
            if let Some(value) = get_env(&var_name) {
                result
                    .push_str(value)
                    .map_err(|_| "Expanded string too long")?;
            }
            // If variable doesn't exist, just ignore (POSIX behavior)
        } else {
            result.push(ch).map_err(|_| "Expanded string too long")?;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokenize() {
        let tokens = tokenize("ls -la /home").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, TokenType::Word);
        assert_eq!(tokens[0].value, "ls");
        assert_eq!(tokens[1].value, "-la");
        assert_eq!(tokens[2].value, "/home");
    }

    #[test]
    fn test_quoted_tokenize() {
        let tokens = tokenize(r#"echo "hello world" 'single quotes'"#).unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1].value, "hello world");
        assert_eq!(tokens[2].value, "single quotes");
    }

    #[test]
    fn test_redirection_tokenize() {
        let tokens = tokenize("cat < input.txt > output.txt").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[1].token_type, TokenType::Redirect);
        assert_eq!(tokens[3].token_type, TokenType::Redirect);
    }
}
