//! Core Utilities for Mach_R Shell
//!
//! Implements essential UNIX-style utilities: ls, cat, echo, ps, etc.
//! These operate through the Mach_R message-passing system.

use crate::servers::file_server;
use crate::types::{TaskId};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Shell utilities implementation
pub struct Utilities {
    current_task: TaskId,
}

impl Utilities {
    /// Create new utilities instance
    pub fn new(task_id: TaskId) -> Self {
        Self {
            current_task: task_id,
        }
    }

    /// List directory contents (ls command)
    pub fn list_directory(&self, path: &str) -> Result<(), &'static str> {
        crate::println!("$ ls {}", path);

        let _file_server = file_server::file_server();
        
        // For now, we'll use the built-in file system entries
        // In a full implementation, this would query the file server
        match path {
            "/" | "" => {
                crate::println!("dev/");
                crate::println!("hello.txt");
                crate::println!("bin/");
                crate::println!("usr/");
            },
            "/dev" => {
                crate::println!("null");
                crate::println!("zero");
                crate::println!("console");
            },
            "/bin" => {
                crate::println!("ls");
                crate::println!("cat");
                crate::println!("echo");
                crate::println!("ps");
                crate::println!("shell");
            },
            "/usr" => {
                crate::println!("bin/");
                crate::println!("lib/");
            },
            _ => {
                crate::println!("ls: {}: No such file or directory", path);
                return Err("Directory not found");
            }
        }
        
        Ok(())
    }

    /// Display file contents (cat command)
    pub fn cat_file(&self, path: &str) -> Result<(), &'static str> {
        crate::println!("$ cat {}", path);
        
        let file_server = file_server::file_server();
        
        // Try to open and read the file
        match file_server.file_open(path.to_string(), 0, self.current_task) {
            Ok(fd) => {
                match file_server.file_read(fd, 1024, self.current_task) {
                    Ok((data, bytes_read)) => {
                        if let Ok(content) = core::str::from_utf8(&data[..bytes_read as usize]) {
                            crate::print!("{}", content);
                        } else {
                            crate::println!("cat: {}: Binary file", path);
                        }
                    },
                    Err(_) => {
                        crate::println!("cat: {}: Error reading file", path);
                        return Err("Read error");
                    }
                }
                let _ = file_server.file_close(fd, self.current_task);
            },
            Err(_) => {
                crate::println!("cat: {}: No such file or directory", path);
                return Err("File not found");
            }
        }
        
        Ok(())
    }

    /// Echo text to output (echo command)
    pub fn echo(&self, args: &[&str]) -> Result<(), &'static str> {
        if args.is_empty() {
            crate::println!();
            return Ok(());
        }

        // Join arguments with spaces
        let mut output = String::new();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                output.push(' ');
            }
            output.push_str(arg);
        }
        
        crate::println!("{}", output);
        Ok(())
    }

    /// Show running processes (ps command)
    pub fn process_status(&self) -> Result<(), &'static str> {
        crate::println!("$ ps");
        crate::println!("PID  COMMAND");
        crate::println!("  0  kernel");
        crate::println!("  1  init");
        crate::println!("  2  name_server");
        crate::println!("  3  file_server");
        crate::println!("  4  vm_server");
        crate::println!(" 10  shell");
        
        // In a full implementation, this would query the task manager
        let _task_manager = crate::task::manager();
        crate::println!("\nSystem status:");
        crate::println!("  Active tasks: Multiple");
        crate::println!("  Available memory: Page-based allocation");
        crate::println!("  IPC: Mach message passing");
        
        Ok(())
    }

    /// Create directory (mkdir command)
    pub fn make_directory(&self, path: &str) -> Result<(), &'static str> {
        crate::println!("$ mkdir {}", path);
        
        // For demonstration - in full implementation would create through file server
        crate::println!("mkdir: Directory creation not yet implemented in file server");
        crate::println!("mkdir: Would create directory '{}'", path);
        
        Ok(())
    }

    /// Remove file (rm command)
    pub fn remove_file(&self, path: &str) -> Result<(), &'static str> {
        crate::println!("$ rm {}", path);
        
        // For demonstration - in full implementation would remove through file server
        crate::println!("rm: File removal not yet implemented in file server");
        crate::println!("rm: Would remove file '{}'", path);
        
        Ok(())
    }

    /// Show current working directory (pwd command)
    pub fn print_working_directory(&self) -> Result<(), &'static str> {
        crate::println!("$ pwd");
        crate::println!("/");  // Root directory for now
        Ok(())
    }

    /// Change directory (cd command)
    pub fn change_directory(&self, path: &str) -> Result<(), &'static str> {
        crate::println!("$ cd {}", path);
        
        // For demonstration - in full implementation would track current directory
        match path {
            "/" | "/dev" | "/bin" | "/usr" => {
                crate::println!("Changed to directory: {}", path);
                Ok(())
            },
            _ => {
                crate::println!("cd: {}: No such directory", path);
                Err("Directory not found")
            }
        }
    }

    /// Execute a shell command
    pub fn execute_command(&self, command_line: &str) -> Result<(), &'static str> {
        let parts: Vec<&str> = command_line.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let command = parts[0];
        let args = &parts[1..];

        match command {
            "ls" => {
                let path = if args.is_empty() { "/" } else { args[0] };
                self.list_directory(path)
            },
            "cat" => {
                if args.is_empty() {
                    crate::println!("cat: missing file operand");
                    return Err("Missing argument");
                }
                self.cat_file(args[0])
            },
            "echo" => {
                self.echo(args)
            },
            "ps" => {
                self.process_status()
            },
            "pwd" => {
                self.print_working_directory()
            },
            "cd" => {
                let path = if args.is_empty() { "/" } else { args[0] };
                self.change_directory(path)
            },
            "mkdir" => {
                if args.is_empty() {
                    crate::println!("mkdir: missing operand");
                    return Err("Missing argument");
                }
                self.make_directory(args[0])
            },
            "rm" => {
                if args.is_empty() {
                    crate::println!("rm: missing operand");
                    return Err("Missing argument");
                }
                self.remove_file(args[0])
            },
            "help" => {
                self.show_help();
                Ok(())
            },
            "exit" => {
                crate::println!("Shell exit requested");
                Err("exit")
            },
            _ => {
                crate::println!("{}: command not found", command);
                Err("Command not found")
            }
        }
    }

    /// Show help for available commands
    pub fn show_help(&self) {
        crate::println!("Available commands:");
        crate::println!("  ls [path]      - List directory contents");
        crate::println!("  cat <file>     - Display file contents");
        crate::println!("  echo [args]    - Print arguments to output");
        crate::println!("  ps             - Show running processes");
        crate::println!("  pwd            - Print working directory");
        crate::println!("  cd [path]      - Change directory");
        crate::println!("  mkdir <path>   - Create directory");
        crate::println!("  rm <file>      - Remove file");
        crate::println!("  help           - Show this help");
        crate::println!("  exit           - Exit shell");
    }

    /// Interactive shell session
    pub fn run_interactive_shell(&self) {
        crate::println!("Starting interactive shell...");
        crate::println!("Type 'help' for available commands, 'exit' to quit");
        crate::println!();

        // Simulate some interactive commands
        let demo_commands = [
            "help",
            "ls /",
            "ls /dev", 
            "cat /hello.txt",
            "echo Hello, Mach_R!",
            "ps",
            "pwd",
            "ls /bin",
        ];

        for cmd in &demo_commands {
            crate::println!("mach_r$ {}", cmd);
            if let Err(e) = self.execute_command(cmd) {
                if e == "exit" {
                    break;
                }
                // Continue on other errors
            }
            crate::println!();
        }

        crate::println!("Shell demonstration complete.");
    }
}

/// Global utilities instance
static mut SHELL_UTILITIES: Option<Utilities> = None;

/// Initialize shell utilities
pub fn init(task_id: TaskId) {
    unsafe {
        SHELL_UTILITIES = Some(Utilities::new(task_id));
    }
}

/// Get the shell utilities instance
pub fn utilities() -> &'static Utilities {
    unsafe {
        (*core::ptr::addr_of!(SHELL_UTILITIES)).as_ref().expect("Utilities not initialized")
    }
}