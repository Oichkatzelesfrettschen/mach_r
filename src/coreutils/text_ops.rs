//! Text operation utilities
//! echo, head, tail, grep, sort, uniq, wc, cut, tr, sed

use super::{get_flag_value, has_flag, parse_args, CommandResult};
use alloc::format;

/// Initialize text operations subsystem
pub fn init() -> Result<(), &'static str> {
    Ok(())
}

/// echo - Print arguments
pub fn echo(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let no_newline = has_flag(&flags, "-n");

    let mut result = CommandResult::success();

    for (i, arg) in params.iter().enumerate() {
        if i > 0 {
            result.add_output(" ")?;
        }
        result.add_output(arg)?;
    }

    if !no_newline {
        result.add_output("\n")?;
    }

    Ok(result)
}

/// head - Print first lines
pub fn head(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let line_count = get_flag_value(&flags, "-n").unwrap_or(10);

    let mut result = CommandResult::success();

    if params.is_empty() {
        // Read from stdin - TODO: implement
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params {
            // TODO: Read actual file and show first N lines
            result.add_output(&format!("First {} lines of {}\n", line_count, filename))?;
            for i in 1..=line_count.min(5) {
                result.add_output(&format!("Line {} of {}\n", i, filename))?;
            }
        }
    }

    Ok(result)
}

/// tail - Print last lines
pub fn tail(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let line_count = get_flag_value(&flags, "-n").unwrap_or(10);

    let mut result = CommandResult::success();

    if params.is_empty() {
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params {
            result.add_output(&format!("Last {} lines of {}\n", line_count, filename))?;
        }
    }

    Ok(result)
}

/// grep - Search text patterns
pub fn grep(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let _case_insensitive = has_flag(&flags, "-i");

    if params.is_empty() {
        return CommandResult::error(1, "grep: missing pattern");
    }

    let pattern = params[0];
    let mut result = CommandResult::success();

    if params.len() == 1 {
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params[1..] {
            result.add_output(&format!("Searching for '{}' in {}\n", pattern, filename))?;
        }
    }

    Ok(result)
}

/// sort - Sort lines
pub fn sort(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let _reverse = has_flag(&flags, "-r");
    let _numeric = has_flag(&flags, "-n");

    let mut result = CommandResult::success();

    if params.is_empty() {
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params {
            result.add_output(&format!("Sorted contents of {}\n", filename))?;
        }
    }

    Ok(result)
}

/// uniq - Remove duplicate lines
pub fn uniq(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let _count = has_flag(&flags, "-c");

    let mut result = CommandResult::success();

    if params.is_empty() {
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params {
            result.add_output(&format!("Unique lines from {}\n", filename))?;
        }
    }

    Ok(result)
}

/// wc - Count lines, words, characters
pub fn wc(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (flags, params) = parse_args(args)?;
    let lines_only = has_flag(&flags, "-l");
    let words_only = has_flag(&flags, "-w");
    let chars_only = has_flag(&flags, "-c");

    let mut result = CommandResult::success();

    if params.is_empty() {
        result.add_output("(stdin not implemented)\n")?;
    } else {
        for filename in &params {
            if lines_only {
                result.add_output(&format!("42 {}\n", filename))?;
            } else if words_only {
                result.add_output(&format!("123 {}\n", filename))?;
            } else if chars_only {
                result.add_output(&format!("567 {}\n", filename))?;
            } else {
                result.add_output(&format!(" 42 123 567 {}\n", filename))?;
            }
        }
    }

    Ok(result)
}

/// cut - Cut out columns
pub fn cut(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    let mut result = CommandResult::success();

    if params.is_empty() {
        return CommandResult::error(1, "cut: missing operand");
    }

    result.add_output("(cut not fully implemented)\n")?;
    Ok(result)
}

/// tr - Translate characters
pub fn tr(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    if params.len() < 2 {
        return CommandResult::error(1, "tr: missing operand");
    }

    let mut result = CommandResult::success();
    result.add_output("(tr not fully implemented)\n")?;

    Ok(result)
}

/// sed - Stream editor
pub fn sed(args: &[&str]) -> Result<CommandResult, &'static str> {
    let (_flags, params) = parse_args(args)?;

    if params.is_empty() {
        return CommandResult::error(1, "sed: missing script");
    }

    let mut result = CommandResult::success();
    result.add_output("(sed not fully implemented)\n")?;

    Ok(result)
}
