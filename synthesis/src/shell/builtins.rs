//! Built-in shell commands
//! POSIX-compliant built-in commands like cd, echo, pwd, etc.

use heapless::String;
use super::{Shell, ExecResult, Command};

/// Built-in command handler function type
pub type BuiltinHandler = fn(&mut Shell, &Command) -> Result<ExecResult, &'static str>;

/// Built-in command registration
pub struct Builtin {
    pub name: &'static str,
    pub handler: BuiltinHandler,
    pub description: &'static str,
}

/// List of all built-in commands
pub const BUILTINS: &[Builtin] = &[
    Builtin { name: "cd", handler: builtin_cd, description: "Change directory" },
    Builtin { name: "pwd", handler: builtin_pwd, description: "Print working directory" },
    Builtin { name: "echo", handler: builtin_echo, description: "Print arguments" },
    Builtin { name: "exit", handler: builtin_exit, description: "Exit shell" },
    Builtin { name: "export", handler: builtin_export, description: "Set environment variable" },
    Builtin { name: "unset", handler: builtin_unset, description: "Unset environment variable" },
    Builtin { name: "env", handler: builtin_env, description: "Print environment" },
    Builtin { name: "history", handler: builtin_history, description: "Show command history" },
    Builtin { name: "help", handler: builtin_help, description: "Show help" },
    Builtin { name: "true", handler: builtin_true, description: "Return success" },
    Builtin { name: "false", handler: builtin_false, description: "Return failure" },
];

/// Check if a command is a built-in
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|builtin| builtin.name == name)
}

/// Execute a built-in command
pub fn execute_builtin(shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    if let Some(builtin) = BUILTINS.iter().find(|b| b.name == command.program.as_str()) {
        (builtin.handler)(shell, command)
    } else {
        Err("Not a built-in command")
    }
}

/// Built-in: cd - Change directory
fn builtin_cd(shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    let target_dir = if command.args.is_empty() {
        // No argument - go to home directory
        "/root" // Default to /root instead of looking up HOME to avoid borrowing issue
    } else {
        command.args[0].as_str()
    };
    
    match shell.change_directory(target_dir) {
        Ok(()) => Ok(ExecResult { exit_code: 0, command_found: true }),
        Err(_) => Ok(ExecResult { exit_code: 1, command_found: true }),
    }
}

/// Built-in: pwd - Print working directory
fn builtin_pwd(_shell: &mut Shell, __command: &Command) -> Result<ExecResult, &'static str> {
    // TODO: Print to stdout
    // For now, just return success
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: echo - Print arguments
fn builtin_echo(_shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    // TODO: Print arguments to stdout with proper formatting
    // Handle -n flag (no newline), -e flag (escape sequences)

    let mut output = String::<1024>::new();
    let mut no_newline = false;
    let mut process_escapes = false;
    let mut first_arg = true;

    // Process flags and arguments
    for arg in &command.args {
        if first_arg && arg.starts_with('-') {
            if arg == "-n" {
                no_newline = true;
            } else if arg == "-e" {
                process_escapes = true;
            } else if arg == "-en" || arg == "-ne" {
                no_newline = true;
                process_escapes = true;
            }
        } else {
            if !first_arg {
                output.push(' ').map_err(|_| "Output too long")?;
            }
            
            if process_escapes {
                // TODO: Process escape sequences like \n, \t, etc.
                output.push_str(arg).map_err(|_| "Output too long")?;
            } else {
                output.push_str(arg).map_err(|_| "Output too long")?;
            }
            first_arg = false;
        }
    }
    
    if !no_newline {
        output.push('\n').map_err(|_| "Output too long")?;
    }
    
    // TODO: Actually output to stdout
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: exit - Exit shell
fn builtin_exit(_shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    let exit_code = if command.args.is_empty() {
        0
    } else {
        // Try to parse exit code from first argument
        // For now, just use 0 or 1
        if command.args[0] == "0" {
            0
        } else {
            1
        }
    };
    
    // TODO: Actually exit the shell
    Ok(ExecResult { exit_code, command_found: true })
}

/// Built-in: export - Set environment variable
fn builtin_export(shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    if command.args.is_empty() {
        // No arguments - print all environment variables
        return Ok(ExecResult { exit_code: 0, command_found: true });
    }
    
    for arg in &command.args {
        if let Some(eq_pos) = arg.find('=') {
            let (name, value) = arg.split_at(eq_pos);
            let value = &value[1..]; // Skip the '=' character
            
            shell.set_env(name, value)
                .map_err(|_| "Failed to set environment variable")?;
        } else {
            // Just export existing variable (make it available to child processes)
            // For now, this is a no-op since we don't have child processes yet
        }
    }
    
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: unset - Unset environment variable
fn builtin_unset(shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    for arg in &command.args {
        // Remove from environment
        if let Some(pos) = shell.env_vars.iter().position(|var| var.name == arg.as_str()) {
            shell.env_vars.swap_remove(pos);
        }
    }
    
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: env - Print environment
fn builtin_env(_shell: &mut Shell, __command: &Command) -> Result<ExecResult, &'static str> {
    // TODO: Print all environment variables
    // Format: NAME=value
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: history - Show command history
fn builtin_history(_shell: &mut Shell, __command: &Command) -> Result<ExecResult, &'static str> {
    // TODO: Print command history with line numbers
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: help - Show help
fn builtin_help(_shell: &mut Shell, command: &Command) -> Result<ExecResult, &'static str> {
    if command.args.is_empty() {
        // Show general help - list all built-ins
        // TODO: Print help text
    } else {
        // Show help for specific command
        let cmd_name = &command.args[0];
        if let Some(_builtin) = BUILTINS.iter().find(|b| b.name == cmd_name.as_str()) {
            // TODO: Print detailed help for this built-in
        } else {
            // TODO: Print "command not found" or general help
        }
    }
    
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: true - Always return success
fn builtin_true(_shell: &mut Shell, __command: &Command) -> Result<ExecResult, &'static str> {
    Ok(ExecResult { exit_code: 0, command_found: true })
}

/// Built-in: false - Always return failure
fn builtin_false(_shell: &mut Shell, __command: &Command) -> Result<ExecResult, &'static str> {
    Ok(ExecResult { exit_code: 1, command_found: true })
}

/// Process escape sequences in a string (for echo -e)
#[allow(dead_code)]
fn process_escape_sequences(input: &str) -> Result<String<1024>, &'static str> {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next_ch) = chars.next() {
                match next_ch {
                    'n' => result.push('\n').map_err(|_| "Output too long")?,
                    't' => result.push('\t').map_err(|_| "Output too long")?,
                    'r' => result.push('\r').map_err(|_| "Output too long")?,
                    '\\' => result.push('\\').map_err(|_| "Output too long")?,
                    '"' => result.push('"').map_err(|_| "Output too long")?,
                    '\'' => result.push('\'').map_err(|_| "Output too long")?,
                    '0' => result.push('\0').map_err(|_| "Output too long")?,
                    _ => {
                        // Unknown escape - just include literally
                        result.push('\\').map_err(|_| "Output too long")?;
                        result.push(next_ch).map_err(|_| "Output too long")?;
                    }
                }
            } else {
                result.push('\\').map_err(|_| "Output too long")?;
            }
        } else {
            result.push(ch).map_err(|_| "Output too long")?;
        }
    }
    
    Ok(result)
}