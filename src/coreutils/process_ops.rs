//! Process operation utilities
//! ps, kill, sleep, which, type

use super::{has_flag, parse_args, CommandResult};
use alloc::format;

/// Initialize process operations subsystem
pub fn init() -> Result<(), &'static str> {
    Ok(())
}

/// ps - List processes
pub fn ps(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, _params) = parse_args(args)?;
    let _show_all = has_flag(&flags, "-a") || has_flag(&flags, "a");
    let show_users = has_flag(&flags, "-u") || has_flag(&flags, "u");
    let _show_extra = has_flag(&flags, "-x") || has_flag(&flags, "x");

    let mut result = CommandResult::success();

    // Print header
    if show_users {
        result.add_output(
            "  PID  USER     %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND\n",
        )?;
    } else {
        result.add_output("  PID TTY          TIME CMD\n")?;
    }

    // TODO: Get actual process list
    if show_users {
        result.add_output(
            "    1 root      0.0  0.1   1234   567 ?        S    00:00   0:00 init\n",
        )?;
        result.add_output(
            "    2 root      0.0  0.0      0     0 ?        S    00:00   0:00 [kthreadd]\n",
        )?;
        result.add_output(
            "   42 root      0.1  0.2   2345  1123 tty1     S    00:01   0:01 mach_r_shell\n",
        )?;
    } else {
        result.add_output("    1 ?        00:00:00 init\n")?;
        result.add_output("    2 ?        00:00:00 kthreadd\n")?;
        result.add_output("   42 tty1     00:00:01 mach_r_shell\n")?;
    }

    Ok(result)
}

/// kill - Terminate processes
pub fn kill(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let mut signal = "TERM";

    // Parse signal flag
    for flag in &flags {
        if flag.starts_with("-") && flag.len() > 1 {
            let sig = &flag[1..];
            if sig.chars().all(|c| c.is_alphabetic()) {
                signal = sig;
            }
        }
    }

    if params.is_empty() {
        return CommandResult::error(1, "kill: missing operand");
    }

    let mut result = CommandResult::success();

    for pid_str in &params {
        // TODO: Parse PID and send actual signal
        result.add_output(&format!("Sent {} signal to process {}\n", signal, pid_str))?;
    }

    Ok(result)
}

/// sleep - Sleep for duration
pub fn sleep(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    if params.is_empty() {
        return CommandResult::error(1, "sleep: missing operand");
    }

    let duration_str = params[0];

    // TODO: Parse duration and actually sleep
    // For now, just simulate
    let mut result = CommandResult::success();
    result.add_output(&format!("Slept for {} seconds\n", duration_str))?;

    Ok(result)
}

/// which - Locate command
pub fn which(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    if params.is_empty() {
        return CommandResult::error(1, "which: missing operand");
    }

    let mut result = CommandResult::success();

    for command in &params {
        // TODO: Search in PATH environment variable
        match *command {
            "ls" | "cat" | "grep" | "cp" | "mv" => {
                result.add_output(&format!("/bin/{}\n", command))?;
            }
            "gcc" | "clang" => {
                result.add_output(&format!("/usr/bin/{}\n", command))?;
            }
            "rustc" | "cargo" => {
                result.add_output(&format!("/usr/local/bin/{}\n", command))?;
            }
            _ => {
                result.add_error(&format!("{}: not found\n", command))?;
                result.exit_code = 1;
            }
        }
    }

    Ok(result)
}

/// type - Show command type
pub fn type_cmd(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    if params.is_empty() {
        return CommandResult::error(1, "type: missing operand");
    }

    let mut result = CommandResult::success();

    for command in &params {
        // TODO: Check if command is builtin, alias, function, or external
        match *command {
            "cd" | "echo" | "pwd" | "exit" | "export" => {
                result.add_output(&format!("{} is a shell builtin\n", command))?;
            }
            "ls" | "cat" | "grep" | "cp" | "mv" | "rm" => {
                result.add_output(&format!("{} is /bin/{}\n", command, command))?;
            }
            _ => {
                result.add_output(&format!("{}: not found\n", command))?;
                result.exit_code = 1;
            }
        }
    }

    Ok(result)
}
