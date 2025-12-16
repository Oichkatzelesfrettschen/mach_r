//! POSIX unistd.h - standard Unix functions

use super::{set_errno_and_fail, trap_result_to_c};
use crate::trap::trap_dispatch;

/// Read from file descriptor
#[no_mangle]
pub extern "C" fn read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument) as isize;
    }

    let args = [fd as usize, buf as usize, count];
    trap_result_to_c(trap_dispatch(0, &args)) // SYS_read
}

/// Write to file descriptor
#[no_mangle]
pub extern "C" fn write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument) as isize;
    }

    let args = [fd as usize, buf as usize, count];
    trap_result_to_c(trap_dispatch(1, &args)) // SYS_write
}

/// Open file
#[no_mangle]
pub extern "C" fn open(pathname: *const u8, flags: i32, mode: u32) -> i32 {
    if pathname.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [pathname as usize, flags as usize, mode as usize];
    trap_result_to_c(trap_dispatch(2, &args)) as i32 // SYS_open
}

/// Close file descriptor
#[no_mangle]
pub extern "C" fn close(fd: i32) -> i32 {
    let args = [fd as usize];
    trap_result_to_c(trap_dispatch(3, &args)) as i32 // SYS_close
}

/// Get process ID
#[no_mangle]
pub extern "C" fn getpid() -> i32 {
    let args = [];
    trap_result_to_c(trap_dispatch(39, &args)) as i32 // SYS_getpid
}

/// Fork process
#[no_mangle]
pub extern "C" fn fork() -> i32 {
    let args = [];
    trap_result_to_c(trap_dispatch(57, &args)) as i32 // SYS_fork
}

/// Execute program
#[no_mangle]
pub extern "C" fn execve(
    filename: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
) -> i32 {
    if filename.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [filename as usize, argv as usize, envp as usize];
    trap_result_to_c(trap_dispatch(59, &args)) as i32 // SYS_execve
}

/// Exit process
#[no_mangle]
pub extern "C" fn exit(status: i32) -> ! {
    let args = [status as usize];
    let _ = trap_dispatch(60, &args); // SYS_exit
                                      // Should never return, but just in case...
    loop {
        core::hint::spin_loop();
    }
}

/// Duplicate file descriptor
#[no_mangle]
pub extern "C" fn dup(fd: i32) -> i32 {
    let args = [fd as usize];
    trap_result_to_c(trap_dispatch(32, &args)) as i32 // SYS_dup
}

/// Duplicate file descriptor to specific fd
#[no_mangle]
pub extern "C" fn dup2(oldfd: i32, newfd: i32) -> i32 {
    let args = [oldfd as usize, newfd as usize];
    trap_result_to_c(trap_dispatch(33, &args)) as i32 // SYS_dup2
}

/// Change working directory
#[no_mangle]
pub extern "C" fn chdir(path: *const u8) -> i32 {
    if path.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [path as usize];
    trap_result_to_c(trap_dispatch(80, &args)) as i32 // SYS_chdir
}

/// Get current working directory
#[no_mangle]
pub extern "C" fn getcwd(buf: *mut u8, size: usize) -> *mut u8 {
    if buf.is_null() {
        return core::ptr::null_mut();
    }

    let args = [buf as usize, size];
    match trap_dispatch(79, &args) {
        // SYS_getcwd
        Ok(_) => buf,
        Err(error) => {
            unsafe {
                super::ERRNO = error.to_errno();
            }
            core::ptr::null_mut()
        }
    }
}

/// Create directory
#[no_mangle]
pub extern "C" fn mkdir(pathname: *const u8, mode: u32) -> i32 {
    if pathname.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [pathname as usize, mode as usize];
    trap_result_to_c(trap_dispatch(83, &args)) as i32 // SYS_mkdir
}

/// Remove directory
#[no_mangle]
pub extern "C" fn rmdir(pathname: *const u8) -> i32 {
    if pathname.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [pathname as usize];
    trap_result_to_c(trap_dispatch(84, &args)) as i32 // SYS_rmdir
}

/// Unlink (delete) file
#[no_mangle]
pub extern "C" fn unlink(pathname: *const u8) -> i32 {
    if pathname.is_null() {
        return set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [pathname as usize];
    trap_result_to_c(trap_dispatch(87, &args)) as i32 // SYS_unlink
}

/// Get user ID
#[no_mangle]
pub extern "C" fn getuid() -> u32 {
    let args = [];
    trap_result_to_c(trap_dispatch(102, &args)) as u32 // SYS_getuid
}

/// Get group ID
#[no_mangle]
pub extern "C" fn getgid() -> u32 {
    let args = [];
    trap_result_to_c(trap_dispatch(104, &args)) as u32 // SYS_getgid
}
