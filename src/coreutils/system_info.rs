//! System information utilities
//! pwd, whoami, id, date, uptime, uname, hostname, env

use super::{has_flag, parse_args, CommandResult};
use alloc::format;
use heapless::String;

/// Initialize system info subsystem
pub fn init() -> Result<(), &'static str> {
    Ok(())
}

/// pwd - Print working directory
pub fn pwd(_args: &[&str]) -> Result<CommandResult, &'static str> {
    let mut result = CommandResult::success();
    result.add_output("/\n")?; // TODO: Get actual working directory
    Ok(result)
}

/// whoami - Print current user
pub fn whoami(_args: &[&str]) -> Result<CommandResult, &'static str> {
    let mut result = CommandResult::success();
    result.add_output("root\n")?; // TODO: Get actual current user
    Ok(result)
}

/// id - Print user/group IDs
pub fn id(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;
    let mut result = CommandResult::success();

    if params.is_empty() {
        // Show current user
        result.add_output("uid=0(root) gid=0(root) groups=0(root)\n")?;
    } else {
        // Show specified user
        let username = params[0];
        result.add_output(&format!(
            "uid=1000({}) gid=1000({}) groups=1000({})\n",
            username, username, username
        ))?;
    }

    Ok(result)
}

/// date - Print/set date
pub fn date(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;
    let mut result = CommandResult::success();

    // TODO: Get actual system time
    if params.is_empty() {
        result.add_output("Mon Jan  1 00:00:00 UTC 2024\n")?;
    } else {
        // Custom format - TODO: implement format parsing
        result.add_output("2024-01-01 00:00:00\n")?;
    }

    Ok(result)
}

/// uptime - Show system uptime
pub fn uptime(_args: &[&str]) -> Result<CommandResult, &'static str> {
    let mut result = CommandResult::success();

    // TODO: Get actual uptime
    result.add_output(" 12:34:56 up  1:23,  2 users,  load average: 0.15, 0.20, 0.18\n")?;

    Ok(result)
}

/// uname - System information
pub fn uname(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, _params) = parse_args(args)?;
    let show_all = has_flag(&flags, "-a");
    let show_system = has_flag(&flags, "-s") || flags.is_empty();
    let show_node = has_flag(&flags, "-n");
    let show_release = has_flag(&flags, "-r");
    let show_version = has_flag(&flags, "-v");
    let show_machine = has_flag(&flags, "-m");

    let mut result = CommandResult::success();
    let mut output = String::<256>::new();

    if show_all || show_system {
        output.push_str("Mach_R").map_err(|_| "Output too long")?;
    }

    if show_all || show_node {
        if !output.is_empty() {
            output.push(' ').map_err(|_| "Output too long")?;
        }
        output.push_str("mach-r").map_err(|_| "Output too long")?;
    }

    if show_all || show_release {
        if !output.is_empty() {
            output.push(' ').map_err(|_| "Output too long")?;
        }
        output.push_str("0.1.0").map_err(|_| "Output too long")?;
    }

    if show_all || show_version {
        if !output.is_empty() {
            output.push(' ').map_err(|_| "Output too long")?;
        }
        output
            .push_str("#1 Mon Jan 1 00:00:00 UTC 2024")
            .map_err(|_| "Output too long")?;
    }

    if show_all || show_machine {
        if !output.is_empty() {
            output.push(' ').map_err(|_| "Output too long")?;
        }
        output.push_str("aarch64").map_err(|_| "Output too long")?;
    }

    result.add_output(&output)?;
    result.add_output("\n")?;

    Ok(result)
}

/// hostname - Print/set hostname
pub fn hostname(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;
    let mut result = CommandResult::success();

    if params.is_empty() {
        // Print hostname
        result.add_output("mach-r\n")?;
    } else {
        // Set hostname - TODO: implement hostname setting
        let new_hostname = params[0];
        result.add_output(&format!("Hostname set to {}\n", new_hostname))?;
    }

    Ok(result)
}

/// env - Print environment
pub fn env(_args: &[&str]) -> Result<CommandResult, &'static str> {
    let mut result = CommandResult::success();

    // TODO: Get actual environment variables from shell
    result.add_output("HOME=/root\n")?;
    result.add_output("PATH=/bin:/usr/bin\n")?;
    result.add_output("PWD=/\n")?;
    result.add_output("SHELL=/bin/mach_r_shell\n")?;
    result.add_output("USER=root\n")?;

    Ok(result)
}
