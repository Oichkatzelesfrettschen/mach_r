//! Minimal POSIX libc implementation for Mach_R
//!
//! Provides a small C-compatible interface that maps POSIX calls to Mach messages.
//! Based on xv6-rust approach but adapted for Mach's port-based IPC.

pub mod errno;
pub mod stdio;
pub mod unistd;
pub mod stdlib;
pub mod string;
pub mod sys;

use crate::trap::TrapError;

/// libc errno - global error number
static mut ERRNO: i32 = 0;

/// Get current errno value
#[no_mangle]
pub extern "C" fn __errno_location() -> *mut i32 {
    unsafe { core::ptr::addr_of_mut!(ERRNO) }
}

/// Set errno and return -1
fn set_errno_and_fail(error: TrapError) -> i32 {
    unsafe {
        ERRNO = error.to_errno();
    }
    -1
}

/// Convert trap result to C return value
fn trap_result_to_c(result: Result<usize, TrapError>) -> isize {
    match result {
        Ok(value) => value as isize,
        Err(error) => {
            unsafe { ERRNO = error.to_errno(); }
            -1
        }
    }
}

// File descriptor constants
pub const STDIN_FILENO: i32 = 0;
pub const STDOUT_FILENO: i32 = 1;
pub const STDERR_FILENO: i32 = 2;

// Process constants
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;

// File flags
pub const O_RDONLY: i32 = 0;
pub const O_WRONLY: i32 = 1;
pub const O_RDWR: i32 = 2;
pub const O_CREAT: i32 = 64;
pub const O_TRUNC: i32 = 512;
pub const O_APPEND: i32 = 1024;

// File permissions
pub const S_IRUSR: u32 = 0o400;
pub const S_IWUSR: u32 = 0o200;
pub const S_IXUSR: u32 = 0o100;
pub const S_IRGRP: u32 = 0o040;
pub const S_IWGRP: u32 = 0o020;
pub const S_IXGRP: u32 = 0o010;
pub const S_IROTH: u32 = 0o004;
pub const S_IWOTH: u32 = 0o002;
pub const S_IXOTH: u32 = 0o001;