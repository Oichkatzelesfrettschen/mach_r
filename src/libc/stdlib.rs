//! POSIX stdlib.h - standard library functions
//!
//! # Safety
//!
//! All functions in this module are FFI-compatible C library functions.
//! Callers must ensure pointers are valid and memory allocations are tracked.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(not(test))]
use alloc::alloc::{alloc, dealloc, realloc, Layout};
use core::ptr;

/// Allocate memory
/// Note: Disabled in test mode to avoid conflicting with system malloc
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }

    let layout = match Layout::from_size_align(size, 8) {
        Ok(layout) => layout,
        Err(_) => return ptr::null_mut(),
    };

    unsafe { alloc(layout) }
}

/// Free memory
/// Note: Disabled in test mode to avoid conflicting with system free
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    // Note: In real implementation, we'd need to store size info
    // For now, assume 8-byte alignment
    unsafe {
        let layout = Layout::from_size_align_unchecked(1, 8);
        dealloc(ptr, layout);
    }
}

/// Reallocate memory
/// Note: Disabled in test mode to avoid conflicting with system realloc
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn realloc_impl(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(new_size);
    }

    if new_size == 0 {
        free(ptr);
        return ptr::null_mut();
    }

    // Simplified realloc - in real implementation, would preserve data
    let _new_layout = match Layout::from_size_align(new_size, 8) {
        Ok(layout) => layout,
        Err(_) => return ptr::null_mut(),
    };

    let old_layout = Layout::from_size_align(1, 8).unwrap(); // Simplified

    unsafe { realloc(ptr, old_layout, new_size) }
}

/// Convert string to integer
#[no_mangle]
pub extern "C" fn atoi(nptr: *const u8) -> i32 {
    if nptr.is_null() {
        return 0;
    }

    let mut result = 0i32;
    let mut sign = 1i32;
    let mut i = 0usize;

    unsafe {
        // Skip whitespace
        while *nptr.add(i) == b' ' || *nptr.add(i) == b'\t' {
            i += 1;
        }

        // Handle sign
        if *nptr.add(i) == b'-' {
            sign = -1;
            i += 1;
        } else if *nptr.add(i) == b'+' {
            i += 1;
        }

        // Parse digits
        while *nptr.add(i) != 0 {
            let digit = *nptr.add(i);
            if digit.is_ascii_digit() {
                result = result * 10 + (digit - b'0') as i32;
                i += 1;
            } else {
                break;
            }
        }
    }

    result * sign
}

/// Convert integer to string
#[no_mangle]
pub extern "C" fn itoa(value: i32, str_ptr: *mut u8, base: i32) -> *mut u8 {
    if str_ptr.is_null() || !(2..=36).contains(&base) {
        return ptr::null_mut();
    }

    let mut num = value;
    let mut is_negative = false;
    let mut i = 0usize;

    if num == 0 {
        unsafe {
            *str_ptr.add(0) = b'0';
            *str_ptr.add(1) = 0;
        }
        return str_ptr;
    }

    if num < 0 && base == 10 {
        is_negative = true;
        num = -num;
    }

    // Generate digits in reverse order
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    while num > 0 {
        unsafe {
            *str_ptr.add(i) = digits[(num % base) as usize];
        }
        num /= base;
        i += 1;
    }

    if is_negative {
        unsafe {
            *str_ptr.add(i) = b'-';
        }
        i += 1;
    }

    // Null terminate
    unsafe {
        *str_ptr.add(i) = 0;
    }

    // Reverse string
    let len = i;
    for j in 0..(len / 2) {
        unsafe {
            let temp = *str_ptr.add(j);
            *str_ptr.add(j) = *str_ptr.add(len - 1 - j);
            *str_ptr.add(len - 1 - j) = temp;
        }
    }

    str_ptr
}

/// Abort program
/// Note: Disabled in test mode to avoid conflicting with system abort
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn abort() -> ! {
    // Signal abnormal termination
    super::unistd::exit(134); // SIGABRT = 6, exit code = 128 + 6
}

/// Normal program termination (wrapper for unistd::exit)
pub fn libc_exit(status: i32) -> ! {
    super::unistd::exit(status)
}

/// Get environment variable
#[no_mangle]
pub extern "C" fn getenv(_name: *const u8) -> *mut u8 {
    // Simplified - no environment support yet
    ptr::null_mut()
}
