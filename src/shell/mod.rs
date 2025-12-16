//! POSIX-compliant shell implementation for Mach_R
//! Pure Rust, no_std compatible shell with essential POSIX features

use heapless::{String, Vec};

pub mod builtins;
pub mod executor;
pub mod parser;

/// Maximum command line length
const MAX_COMMAND_LENGTH: usize = 1024;
/// Maximum number of arguments
const MAX_ARGS: usize = 64;
/// Maximum number of environment variables
const MAX_ENV_VARS: usize = 128;

/// Shell command structure
#[derive(Debug, Clone)]
pub struct Command {
    /// Command name/program
    pub program: String<256>,
    /// Command arguments
    pub args: Vec<String<256>, MAX_ARGS>,
    /// Input redirection
    pub stdin_file: Option<String<256>>,
    /// Output redirection
    pub stdout_file: Option<String<256>>,
    /// Error redirection
    pub stderr_file: Option<String<256>>,
    /// Background execution flag
    pub background: bool,
}

impl Command {
    /// Create a new empty command
    pub fn new() -> Self {
        Self {
            program: String::new(),
            args: Vec::new(),
            stdin_file: None,
            stdout_file: None,
            stderr_file: None,
            background: false,
        }
    }
}

/// Shell environment variable
#[derive(Debug, Clone)]
pub struct EnvVar {
    pub name: String<64>,
    pub value: String<256>,
}

/// Shell execution result
#[derive(Debug, Clone, Copy)]
pub struct ExecResult {
    /// Exit code
    pub exit_code: i32,
    /// Whether command was found
    pub command_found: bool,
}

/// Shell state and configuration
pub struct Shell {
    /// Current working directory
    pub cwd: String<512>,
    /// Environment variables
    pub env_vars: Vec<EnvVar, MAX_ENV_VARS>,
    /// Command history
    pub history: Vec<String<MAX_COMMAND_LENGTH>, 100>,
    /// Exit status of last command
    pub last_exit_code: i32,
    /// Interactive mode flag
    pub interactive: bool,
    /// Shell prompt
    pub prompt: String<64>,
}

impl Shell {
    /// Create a new shell instance
    pub fn new() -> Self {
        let mut shell = Self {
            cwd: String::new(),
            env_vars: Vec::new(),
            history: Vec::new(),
            last_exit_code: 0,
            interactive: true,
            prompt: String::new(),
        };

        // Set default working directory
        shell.cwd.push_str("/").ok();

        // Set default prompt
        shell.prompt.push_str("mach_r$ ").ok();

        // Initialize default environment variables
        shell.set_env("PWD", "/").ok();
        shell.set_env("HOME", "/root").ok();
        shell.set_env("PATH", "/bin:/usr/bin").ok();
        shell.set_env("SHELL", "/bin/mach_r_shell").ok();

        shell
    }

    /// Set environment variable
    pub fn set_env(&mut self, name: &str, value: &str) -> Result<(), &'static str> {
        // Remove existing variable if present
        if let Some(pos) = self.env_vars.iter().position(|var| var.name == name) {
            self.env_vars.swap_remove(pos);
        }

        let mut env_name = String::new();
        env_name
            .push_str(name)
            .map_err(|_| "Environment variable name too long")?;

        let mut env_value = String::new();
        env_value
            .push_str(value)
            .map_err(|_| "Environment variable value too long")?;

        let env_var = EnvVar {
            name: env_name,
            value: env_value,
        };

        self.env_vars
            .push(env_var)
            .map_err(|_| "Too many environment variables")?;
        Ok(())
    }

    /// Get environment variable
    pub fn get_env(&self, name: &str) -> Option<&str> {
        self.env_vars
            .iter()
            .find(|var| var.name == name)
            .map(|var| var.value.as_str())
    }

    /// Change directory
    pub fn change_directory(&mut self, path: &str) -> Result<(), &'static str> {
        // TODO: Implement actual directory change via filesystem
        self.cwd.clear();
        self.cwd.push_str(path).map_err(|_| "Path too long")?;
        self.set_env("PWD", path)?;
        Ok(())
    }

    /// Add command to history
    pub fn add_to_history(&mut self, command: &str) -> Result<(), &'static str> {
        let mut cmd = String::new();
        cmd.push_str(command)
            .map_err(|_| "Command too long for history")?;

        if self.history.is_full() {
            self.history.swap_remove(0); // Remove oldest entry
        }

        self.history.push(cmd).map_err(|_| "History full")?;
        Ok(())
    }

    /// Execute a command line
    pub fn execute_line(&mut self, line: &str) -> Result<ExecResult, &'static str> {
        if line.trim().is_empty() {
            return Ok(ExecResult {
                exit_code: 0,
                command_found: true,
            });
        }

        // Add to history
        self.add_to_history(line)?;

        // Parse the command
        let command = parser::parse_command_line(line)?;

        // Execute the command
        let result = executor::execute_command(self, &command)?;

        self.last_exit_code = result.exit_code;
        Ok(result)
    }

    /// Get the shell prompt
    pub fn get_prompt(&self) -> &str {
        self.prompt.as_str()
    }

    /// Run interactive shell loop
    pub fn run_interactive(&mut self) -> Result<(), &'static str> {
        // TODO: Implement actual interactive loop with input handling
        // This would involve:
        // 1. Display prompt
        // 2. Read input line
        // 3. Parse and execute command
        // 4. Handle special keys (tab completion, history, etc.)
        // 5. Repeat until exit

        Ok(())
    }

    /// Run shell script from string
    pub fn run_script(&mut self, script: &str) -> Result<ExecResult, &'static str> {
        let mut last_result = ExecResult {
            exit_code: 0,
            command_found: true,
        };

        // Split script into lines and execute each
        for line in script.split('\n') {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                last_result = self.execute_line(trimmed)?;

                // Exit on error if not interactive
                if !self.interactive && last_result.exit_code != 0 {
                    break;
                }
            }
        }

        Ok(last_result)
    }
}

static mut SHELL: Option<Shell> = None;

/// Initialize the shell subsystem
pub fn init() -> Result<(), &'static str> {
    let shell = Shell::new();

    unsafe {
        SHELL = Some(shell);
    }

    Ok(())
}

/// Get the global shell instance
pub fn get_shell() -> Option<&'static mut Shell> {
    unsafe { (*core::ptr::addr_of_mut!(SHELL)).as_mut() }
}
