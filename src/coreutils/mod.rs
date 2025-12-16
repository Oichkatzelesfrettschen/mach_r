//! Core utilities implementation for Mach_R
//! Pure Rust, no_std compatible implementations of essential POSIX utilities

use heapless::{String, Vec};

pub mod file_ops;
pub mod process_ops;
pub mod system_info;
pub mod text_ops;

/// Maximum command line argument length
const MAX_ARG_LEN: usize = 256;
/// Maximum number of arguments
const MAX_ARGS: usize = 64;
/// Maximum output buffer size
const MAX_OUTPUT: usize = 4096;

/// Command execution result
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String<MAX_OUTPUT>,
    /// Standard error
    pub stderr: String<MAX_OUTPUT>,
}

impl CommandResult {
    /// Create a new successful result
    pub fn success() -> Self {
        Self {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    /// Create a new error result
    pub fn error(code: i32, message: &str) -> Result<Self, &'static str> {
        let mut result = Self {
            exit_code: code,
            stdout: String::new(),
            stderr: String::new(),
        };
        result
            .stderr
            .push_str(message)
            .map_err(|_| "Error message too long")?;
        Ok(result)
    }

    /// Add to stdout
    pub fn add_output(&mut self, text: &str) -> Result<(), &'static str> {
        self.stdout.push_str(text).map_err(|_| "Output too long")
    }

    /// Add to stderr
    pub fn add_error(&mut self, text: &str) -> Result<(), &'static str> {
        self.stderr
            .push_str(text)
            .map_err(|_| "Error output too long")
    }
}

/// Core utility command handler
pub type UtilityHandler = fn(&[&str]) -> Result<CommandResult, &'static str>;

/// Core utility definition
pub struct CoreUtility {
    pub name: &'static str,
    pub handler: UtilityHandler,
    pub description: &'static str,
    pub usage: &'static str,
}

/// List of all core utilities
pub const CORE_UTILITIES: &[CoreUtility] = &[
    // File operations
    CoreUtility {
        name: "ls",
        handler: file_ops::ls,
        description: "List directory contents",
        usage: "ls [-la] [files...]",
    },
    CoreUtility {
        name: "cat",
        handler: file_ops::cat,
        description: "Print file contents",
        usage: "cat [files...]",
    },
    CoreUtility {
        name: "cp",
        handler: file_ops::cp,
        description: "Copy files",
        usage: "cp source dest",
    },
    CoreUtility {
        name: "mv",
        handler: file_ops::mv,
        description: "Move/rename files",
        usage: "mv source dest",
    },
    CoreUtility {
        name: "rm",
        handler: file_ops::rm,
        description: "Remove files",
        usage: "rm [-rf] files...",
    },
    CoreUtility {
        name: "mkdir",
        handler: file_ops::mkdir,
        description: "Create directories",
        usage: "mkdir [-p] dirs...",
    },
    CoreUtility {
        name: "rmdir",
        handler: file_ops::rmdir,
        description: "Remove directories",
        usage: "rmdir dirs...",
    },
    CoreUtility {
        name: "touch",
        handler: file_ops::touch,
        description: "Create empty files",
        usage: "touch files...",
    },
    CoreUtility {
        name: "chmod",
        handler: file_ops::chmod,
        description: "Change file permissions",
        usage: "chmod mode files...",
    },
    CoreUtility {
        name: "stat",
        handler: file_ops::stat,
        description: "Display file status",
        usage: "stat files...",
    },
    CoreUtility {
        name: "find",
        handler: file_ops::find,
        description: "Find files",
        usage: "find path -name pattern",
    },
    CoreUtility {
        name: "du",
        handler: file_ops::du,
        description: "Display disk usage",
        usage: "du [-sh] [files...]",
    },
    CoreUtility {
        name: "df",
        handler: file_ops::df,
        description: "Display filesystem usage",
        usage: "df [-h]",
    },
    // Text operations
    CoreUtility {
        name: "echo",
        handler: text_ops::echo,
        description: "Print arguments",
        usage: "echo [-n] text...",
    },
    CoreUtility {
        name: "head",
        handler: text_ops::head,
        description: "Print first lines",
        usage: "head [-n count] [files...]",
    },
    CoreUtility {
        name: "tail",
        handler: text_ops::tail,
        description: "Print last lines",
        usage: "tail [-n count] [files...]",
    },
    CoreUtility {
        name: "grep",
        handler: text_ops::grep,
        description: "Search text patterns",
        usage: "grep [-i] pattern [files...]",
    },
    CoreUtility {
        name: "sort",
        handler: text_ops::sort,
        description: "Sort lines",
        usage: "sort [-rn] [files...]",
    },
    CoreUtility {
        name: "uniq",
        handler: text_ops::uniq,
        description: "Remove duplicate lines",
        usage: "uniq [-c] [files...]",
    },
    CoreUtility {
        name: "wc",
        handler: text_ops::wc,
        description: "Count lines, words, chars",
        usage: "wc [-lwc] [files...]",
    },
    CoreUtility {
        name: "cut",
        handler: text_ops::cut,
        description: "Cut out columns",
        usage: "cut -f fields [files...]",
    },
    CoreUtility {
        name: "tr",
        handler: text_ops::tr,
        description: "Translate characters",
        usage: "tr set1 set2",
    },
    CoreUtility {
        name: "sed",
        handler: text_ops::sed,
        description: "Stream editor",
        usage: "sed 's/pattern/replacement/' [files...]",
    },
    // System information
    CoreUtility {
        name: "pwd",
        handler: system_info::pwd,
        description: "Print working directory",
        usage: "pwd",
    },
    CoreUtility {
        name: "whoami",
        handler: system_info::whoami,
        description: "Print current user",
        usage: "whoami",
    },
    CoreUtility {
        name: "id",
        handler: system_info::id,
        description: "Print user/group IDs",
        usage: "id [user]",
    },
    CoreUtility {
        name: "date",
        handler: system_info::date,
        description: "Print/set date",
        usage: "date [format]",
    },
    CoreUtility {
        name: "uptime",
        handler: system_info::uptime,
        description: "Show system uptime",
        usage: "uptime",
    },
    CoreUtility {
        name: "uname",
        handler: system_info::uname,
        description: "System information",
        usage: "uname [-a]",
    },
    CoreUtility {
        name: "hostname",
        handler: system_info::hostname,
        description: "Print/set hostname",
        usage: "hostname [name]",
    },
    CoreUtility {
        name: "env",
        handler: system_info::env,
        description: "Print environment",
        usage: "env",
    },
    // Process operations
    CoreUtility {
        name: "ps",
        handler: process_ops::ps,
        description: "List processes",
        usage: "ps [aux]",
    },
    CoreUtility {
        name: "kill",
        handler: process_ops::kill,
        description: "Terminate processes",
        usage: "kill [-signal] pids...",
    },
    CoreUtility {
        name: "sleep",
        handler: process_ops::sleep,
        description: "Sleep for duration",
        usage: "sleep seconds",
    },
    CoreUtility {
        name: "which",
        handler: process_ops::which,
        description: "Locate command",
        usage: "which commands...",
    },
    CoreUtility {
        name: "type",
        handler: process_ops::type_cmd,
        description: "Show command type",
        usage: "type commands...",
    },
];

/// Execute a core utility command
pub fn execute_utility(name: &str, args: &[&str]) -> Result<CommandResult, &'static str> {
    if let Some(utility) = CORE_UTILITIES.iter().find(|u| u.name == name) {
        (utility.handler)(args)
    } else {
        CommandResult::error(127, "Command not found")
    }
}

/// Check if a command is a core utility
pub fn is_core_utility(name: &str) -> bool {
    CORE_UTILITIES.iter().any(|u| u.name == name)
}

/// Get utility by name
pub fn get_utility(name: &str) -> Option<&'static CoreUtility> {
    CORE_UTILITIES.iter().find(|u| u.name == name)
}

/// List all available utilities
pub fn list_utilities() -> &'static [CoreUtility] {
    CORE_UTILITIES
}

/// Parse command line arguments into flags and parameters
pub fn parse_args<'a>(
    args: &'a [&'a str],
) -> Result<(Vec<&'a str, 16>, Vec<&'a str, 48>), &'static str> {
    let mut flags = Vec::new();
    let mut params = Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            flags.push(*arg).map_err(|_| "Too many flags")?;
        } else {
            params.push(*arg).map_err(|_| "Too many parameters")?;
        }
    }

    Ok((flags, params))
}

/// Check if a flag is present in the flags list
pub fn has_flag(flags: &[&str], flag: &str) -> bool {
    flags.iter().any(|f| f.contains(&flag[1..]))
}

/// Get numeric value from flag (e.g., -n10)
pub fn get_flag_value(flags: &[&str], flag_prefix: &str) -> Option<i32> {
    for flag in flags {
        if let Some(value_str) = flag.strip_prefix(flag_prefix) {
            if let Ok(value) = value_str.parse::<i32>() {
                return Some(value);
            }
        }
    }
    None
}

static mut COREUTILS_INITIALIZED: bool = false;

/// Initialize the coreutils subsystem
pub fn init() -> Result<(), &'static str> {
    unsafe {
        if COREUTILS_INITIALIZED {
            return Ok(());
        }

        // Initialize subsystems
        file_ops::init()?;
        text_ops::init()?;
        system_info::init()?;
        process_ops::init()?;

        COREUTILS_INITIALIZED = true;
    }

    Ok(())
}
