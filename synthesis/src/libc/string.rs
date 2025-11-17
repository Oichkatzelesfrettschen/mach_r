//! POSIX string.h - string manipulation functions

use core::ptr;

/// String length
#[no_mangle]
pub extern "C" fn strlen(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

/// String copy
#[no_mangle]
pub extern "C" fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    
    let mut i = 0;
    unsafe {
        loop {
            *dest.add(i) = *src.add(i);
            if *src.add(i) == 0 {
                break;
            }
            i += 1;
        }
    }
    dest
}

/// String copy with limit
#[no_mangle]
pub extern "C" fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    
    let mut i = 0;
    unsafe {
        while i < n {
            *dest.add(i) = *src.add(i);
            if *src.add(i) == 0 {
                // Pad remaining with zeros
                while i < n {
                    *dest.add(i) = 0;
                    i += 1;
                }
                break;
            }
            i += 1;
        }
    }
    dest
}

/// String concatenation
#[no_mangle]
pub extern "C" fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    
    let dest_len = strlen(dest);
    strcpy(unsafe { dest.add(dest_len) }, src);
    dest
}

/// String comparison
#[no_mangle]
pub extern "C" fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    if s1.is_null() {
        return -1;
    }
    if s2.is_null() {
        return 1;
    }
    
    let mut i = 0;
    unsafe {
        loop {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            
            if c1 != c2 {
                return (c1 as i32) - (c2 as i32);
            }
            
            if c1 == 0 {
                break;
            }
            
            i += 1;
        }
    }
    0
}

/// String comparison with limit
#[no_mangle]
pub extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    if s1.is_null() {
        return -1;
    }
    if s2.is_null() {
        return 1;
    }
    
    let mut i = 0;
    unsafe {
        while i < n {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            
            if c1 != c2 {
                return (c1 as i32) - (c2 as i32);
            }
            
            if c1 == 0 {
                break;
            }
            
            i += 1;
        }
    }
    0
}

/// Find character in string
#[no_mangle]
pub extern "C" fn strchr(s: *const u8, c: i32) -> *mut u8 {
    if s.is_null() {
        return ptr::null_mut();
    }
    
    let target = c as u8;
    let mut i = 0;
    
    unsafe {
        loop {
            let current = *s.add(i);
            if current == target {
                return s.add(i) as *mut u8;
            }
            if current == 0 {
                break;
            }
            i += 1;
        }
    }
    
    ptr::null_mut()
}

/// Find substring in string
#[no_mangle]
pub extern "C" fn strstr(haystack: *const u8, needle: *const u8) -> *mut u8 {
    if haystack.is_null() || needle.is_null() {
        return ptr::null_mut();
    }
    
    let needle_len = strlen(needle);
    if needle_len == 0 {
        return haystack as *mut u8;
    }
    
    let haystack_len = strlen(haystack);
    if needle_len > haystack_len {
        return ptr::null_mut();
    }
    
    unsafe {
        for i in 0..=(haystack_len - needle_len) {
            if strncmp(haystack.add(i), needle, needle_len) == 0 {
                return haystack.add(i) as *mut u8;
            }
        }
    }
    
    ptr::null_mut()
}

/// Memory copy
#[no_mangle]
pub extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
    
    unsafe {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
    }
    
    dest
}

/// Memory move (handles overlapping regions)
#[no_mangle]
pub extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }
    
    unsafe {
        if dest < src as *mut u8 {
            // Copy forward
            for i in 0..n {
                *dest.add(i) = *src.add(i);
            }
        } else {
            // Copy backward
            for i in (0..n).rev() {
                *dest.add(i) = *src.add(i);
            }
        }
    }
    
    dest
}

/// Memory set
#[no_mangle]
pub extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if s.is_null() || n == 0 {
        return s;
    }
    
    let value = c as u8;
    unsafe {
        for i in 0..n {
            *s.add(i) = value;
        }
    }
    
    s
}

/// Memory compare
#[no_mangle]
pub extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    if s1.is_null() {
        return -1;
    }
    if s2.is_null() {
        return 1;
    }
    
    unsafe {
        for i in 0..n {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            if c1 != c2 {
                return (c1 as i32) - (c2 as i32);
            }
        }
    }
    
    0
}