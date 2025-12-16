//! POSIX stdio.h - standard I/O functions
//!
//! # Safety
//!
//! All functions in this module are FFI-compatible C library functions.
//! Callers must ensure pointers are valid and buffers are properly sized.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use alloc::boxed::Box;

/// Simple printf implementation
#[no_mangle]
pub extern "C" fn printf(format: *const u8) -> i32 {
    if format.is_null() {
        return -1;
    }

    // Convert C string to Rust string
    let c_str = unsafe {
        let mut len = 0;
        while *format.add(len) != 0 {
            len += 1;
        }
        core::slice::from_raw_parts(format, len)
    };

    if let Ok(format_str) = core::str::from_utf8(c_str) {
        // Simple implementation without format specifiers
        let bytes_written = format_str.len();
        for &byte in format_str.as_bytes() {
            super::unistd::write(STDOUT_FILENO, &byte as *const u8, 1);
        }
        bytes_written as i32
    } else {
        -1
    }
}

/// Print string to stdout
#[no_mangle]
pub extern "C" fn puts(s: *const u8) -> i32 {
    if s.is_null() {
        return -1;
    }

    // Convert C string to Rust string
    let c_str = unsafe {
        let mut len = 0;
        while *s.add(len) != 0 {
            len += 1;
        }
        core::slice::from_raw_parts(s, len)
    };

    if let Ok(s_str) = core::str::from_utf8(c_str) {
        // Write string
        for &byte in s_str.as_bytes() {
            super::unistd::write(STDOUT_FILENO, &byte as *const u8, 1);
        }
        // Write newline
        super::unistd::write(STDOUT_FILENO, b"\n".as_ptr(), 1);
        0
    } else {
        -1
    }
}

/// Put single character to stdout
#[no_mangle]
pub extern "C" fn putchar(c: i32) -> i32 {
    let byte = c as u8;
    if super::unistd::write(STDOUT_FILENO, &byte as *const u8, 1) == 1 {
        c
    } else {
        -1
    }
}

/// Get single character from stdin
#[no_mangle]
pub extern "C" fn getchar() -> i32 {
    let mut byte: u8 = 0;
    if super::unistd::read(STDIN_FILENO, &mut byte as *mut u8, 1) == 1 {
        byte as i32
    } else {
        -1 // EOF
    }
}

/// Simple FILE structure (placeholder)
#[repr(C)]
pub struct FILE {
    fd: i32,
    _unused: [u8; 64], // Padding for compatibility
}

/// Standard streams
static mut STDIN_FILE: FILE = FILE {
    fd: STDIN_FILENO,
    _unused: [0; 64],
};
static mut STDOUT_FILE: FILE = FILE {
    fd: STDOUT_FILENO,
    _unused: [0; 64],
};
static mut STDERR_FILE: FILE = FILE {
    fd: STDERR_FILENO,
    _unused: [0; 64],
};

/// Get stdin
#[no_mangle]
pub extern "C" fn stdin() -> *mut FILE {
    unsafe { &mut *core::ptr::addr_of_mut!(STDIN_FILE) }
}

/// Get stdout
#[no_mangle]
pub extern "C" fn stdout() -> *mut FILE {
    unsafe { &mut *core::ptr::addr_of_mut!(STDOUT_FILE) }
}

/// Get stderr
#[no_mangle]
pub extern "C" fn stderr() -> *mut FILE {
    unsafe { &mut *core::ptr::addr_of_mut!(STDERR_FILE) }
}

/// File open
#[no_mangle]
pub extern "C" fn fopen(filename: *const u8, mode: *const u8) -> *mut FILE {
    if filename.is_null() || mode.is_null() {
        return core::ptr::null_mut();
    }

    // Parse mode string - simple implementation
    let flags = match unsafe { *mode } {
        b'r' => super::O_RDONLY,
        b'w' => super::O_WRONLY | super::O_CREAT | super::O_TRUNC,
        b'a' => super::O_WRONLY | super::O_CREAT | super::O_APPEND,
        _ => return core::ptr::null_mut(),
    };

    let fd = super::unistd::open(filename, flags, 0o644);
    if fd < 0 {
        return core::ptr::null_mut();
    }

    // Allocate FILE structure (simplified)
    let file = FILE {
        fd,
        _unused: [0; 64],
    };
    Box::into_raw(Box::new(file))
}

/// File close
#[no_mangle]
pub extern "C" fn fclose(stream: *mut FILE) -> i32 {
    if stream.is_null() {
        return -1;
    }

    let fd = unsafe { (*stream).fd };
    let result = super::unistd::close(fd);

    // Free the FILE structure (if it was allocated)
    if stream != stdin() && stream != stdout() && stream != stderr() {
        unsafe {
            let _ = Box::from_raw(stream);
        }
    }

    result
}

/// File read
#[no_mangle]
pub extern "C" fn fread(ptr: *mut u8, size: usize, nmemb: usize, stream: *mut FILE) -> usize {
    if ptr.is_null() || stream.is_null() || size == 0 {
        return 0;
    }

    let fd = unsafe { (*stream).fd };
    let total_size = size * nmemb;

    let bytes_read = super::unistd::read(fd, ptr, total_size);
    if bytes_read < 0 {
        0
    } else {
        (bytes_read as usize) / size
    }
}

/// File write
#[no_mangle]
pub extern "C" fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut FILE) -> usize {
    if ptr.is_null() || stream.is_null() || size == 0 {
        return 0;
    }

    let fd = unsafe { (*stream).fd };
    let total_size = size * nmemb;

    let bytes_written = super::unistd::write(fd, ptr, total_size);
    if bytes_written < 0 {
        0
    } else {
        (bytes_written as usize) / size
    }
}

/// Flush stream
#[no_mangle]
pub extern "C" fn fflush(_stream: *mut FILE) -> i32 {
    // In a real implementation, would flush internal buffers
    // For now, just return success
    0
}
