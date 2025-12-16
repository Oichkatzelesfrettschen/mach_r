//! Kernel String Utilities
//!
//! Based on Mach4 kern/strings.c (CMU 1987)
//!
//! This module provides low-level string operations for kernel use.
//! These functions are designed to be safe in interrupt context and
//! do not rely on standard library allocators.
//!
//! ## Design Notes
//!
//! Unlike userspace string functions, kernel string functions:
//! - Must never block or allocate
//! - Must handle untrusted input safely
//! - May operate in interrupt context
//! - Need bounded execution time

use core::ptr;

// ============================================================================
// String Length
// ============================================================================

/// Calculate the length of a null-terminated string
///
/// # Safety
/// The `s` pointer must point to a valid null-terminated string.
/// If the string is not null-terminated within addressable memory,
/// behavior is undefined.
#[inline]
pub unsafe fn strlen(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }

    let mut len = 0;
    while *s.add(len) != 0 {
        len += 1;
    }
    len
}

/// Calculate string length with maximum bound
///
/// Returns the length of the string, or `maxlen` if no null terminator
/// is found within that many bytes.
///
/// # Safety
/// The `s` pointer must point to at least `maxlen` readable bytes.
#[inline]
pub unsafe fn strnlen(s: *const u8, maxlen: usize) -> usize {
    if s.is_null() {
        return 0;
    }

    let mut len = 0;
    while len < maxlen && *s.add(len) != 0 {
        len += 1;
    }
    len
}

// ============================================================================
// String Comparison
// ============================================================================

/// Compare two null-terminated strings
///
/// Returns:
/// - 0 if strings are equal
/// - negative if s1 < s2
/// - positive if s1 > s2
///
/// # Safety
/// Both pointers must point to valid null-terminated strings.
#[inline]
pub unsafe fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() || s2.is_null() {
        if s1.is_null() && s2.is_null() {
            return 0;
        }
        return if s1.is_null() { -1 } else { 1 };
    }

    let mut i = 0;
    loop {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);

        if c1 != c2 {
            return c1 as i32 - c2 as i32;
        }

        if c1 == 0 {
            return 0;
        }

        i += 1;
    }
}

/// Compare two strings up to n characters
///
/// # Safety
/// Both pointers must point to at least `n` readable bytes or
/// be null-terminated within `n` bytes.
#[inline]
pub unsafe fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }

    if s1.is_null() || s2.is_null() {
        if s1.is_null() && s2.is_null() {
            return 0;
        }
        return if s1.is_null() { -1 } else { 1 };
    }

    for i in 0..n {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);

        if c1 != c2 {
            return c1 as i32 - c2 as i32;
        }

        if c1 == 0 {
            return 0;
        }
    }

    0
}

// ============================================================================
// String Copy
// ============================================================================

/// Copy a null-terminated string from src to dest
///
/// Returns pointer to dest.
///
/// # Safety
/// - `dest` must have enough space for the entire string including null terminator
/// - `src` must be a valid null-terminated string
/// - The buffers must not overlap
#[inline]
pub unsafe fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() {
        return dest;
    }

    if src.is_null() {
        *dest = 0;
        return dest;
    }

    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            break;
        }
        i += 1;
    }

    dest
}

/// Copy at most n characters from src to dest
///
/// If src is shorter than n, dest is padded with null bytes.
/// If src is n or longer, dest is NOT null-terminated.
///
/// # Safety
/// - `dest` must have space for at least `n` bytes
/// - `src` must be readable for at least min(strlen(src)+1, n) bytes
/// - The buffers must not overlap
#[inline]
pub unsafe fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || n == 0 {
        return dest;
    }

    let src_len = if src.is_null() { 0 } else { strnlen(src, n) };

    // Copy the string
    for i in 0..src_len.min(n) {
        *dest.add(i) = *src.add(i);
    }

    // Pad with nulls if needed
    for i in src_len..n {
        *dest.add(i) = 0;
    }

    dest
}

/// Safe string copy with guaranteed null termination
///
/// Copies at most `n-1` characters from src to dest and always
/// null-terminates. Returns the length of src (like strlcpy).
///
/// # Safety
/// - `dest` must have space for at least `n` bytes
/// - `src` must be a valid null-terminated string
#[inline]
pub unsafe fn strlcpy(dest: *mut u8, src: *const u8, n: usize) -> usize {
    if n == 0 {
        return if src.is_null() { 0 } else { strlen(src) };
    }

    if dest.is_null() {
        return if src.is_null() { 0 } else { strlen(src) };
    }

    let src_len = if src.is_null() { 0 } else { strlen(src) };
    let copy_len = src_len.min(n - 1);

    if !src.is_null() {
        ptr::copy_nonoverlapping(src, dest, copy_len);
    }

    *dest.add(copy_len) = 0;

    src_len
}

// ============================================================================
// String Concatenation
// ============================================================================

/// Concatenate src onto the end of dest
///
/// # Safety
/// - `dest` must have enough space for the combined strings
/// - Both must be valid null-terminated strings
/// - The buffers must not overlap
#[inline]
pub unsafe fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() {
        return dest;
    }

    let dest_len = strlen(dest);
    strcpy(dest.add(dest_len), src)
}

/// Concatenate at most n characters from src onto dest
///
/// Always null-terminates the result.
///
/// # Safety
/// - `dest` must have enough space
/// - Both must be valid strings
#[inline]
pub unsafe fn strncat(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || n == 0 {
        return dest;
    }

    let dest_len = strlen(dest);
    let src_len = if src.is_null() { 0 } else { strnlen(src, n) };
    let copy_len = src_len.min(n);

    if !src.is_null() {
        for i in 0..copy_len {
            *dest.add(dest_len + i) = *src.add(i);
        }
    }

    *dest.add(dest_len + copy_len) = 0;

    dest
}

/// Safe concatenation with size limit
///
/// Returns total length of string that would be created.
///
/// # Safety
/// - `dest` must have space for `n` bytes
/// - Both must be valid null-terminated strings
#[inline]
pub unsafe fn strlcat(dest: *mut u8, src: *const u8, n: usize) -> usize {
    if n == 0 {
        return 0;
    }

    let dest_len = strnlen(dest, n);

    if dest_len >= n {
        // dest is already full
        return dest_len + (if src.is_null() { 0 } else { strlen(src) });
    }

    let remaining = n - dest_len - 1;
    let src_len = if src.is_null() { 0 } else { strlen(src) };
    let copy_len = src_len.min(remaining);

    if !src.is_null() {
        for i in 0..copy_len {
            *dest.add(dest_len + i) = *src.add(i);
        }
    }

    *dest.add(dest_len + copy_len) = 0;

    dest_len + src_len
}

// ============================================================================
// Character Search
// ============================================================================

/// Find first occurrence of character in string
///
/// Returns pointer to character or null if not found.
///
/// # Safety
/// `s` must be a valid null-terminated string.
#[inline]
pub unsafe fn strchr(s: *const u8, c: i32) -> *const u8 {
    if s.is_null() {
        return ptr::null();
    }

    let c = c as u8;
    let mut i = 0;
    loop {
        let ch = *s.add(i);
        if ch == c {
            return s.add(i);
        }
        if ch == 0 {
            return ptr::null();
        }
        i += 1;
    }
}

/// Find last occurrence of character in string
///
/// # Safety
/// `s` must be a valid null-terminated string.
#[inline]
pub unsafe fn strrchr(s: *const u8, c: i32) -> *const u8 {
    if s.is_null() {
        return ptr::null();
    }

    let c = c as u8;
    let mut last: *const u8 = ptr::null();
    let mut i = 0;

    loop {
        let ch = *s.add(i);
        if ch == c {
            last = s.add(i);
        }
        if ch == 0 {
            break;
        }
        i += 1;
    }

    last
}

// ============================================================================
// Substring Search
// ============================================================================

/// Find first occurrence of needle in haystack
///
/// # Safety
/// Both must be valid null-terminated strings.
#[inline]
pub unsafe fn strstr(haystack: *const u8, needle: *const u8) -> *const u8 {
    if haystack.is_null() {
        return ptr::null();
    }

    if needle.is_null() || *needle == 0 {
        return haystack;
    }

    let needle_len = strlen(needle);
    let haystack_len = strlen(haystack);

    if needle_len > haystack_len {
        return ptr::null();
    }

    for i in 0..=(haystack_len - needle_len) {
        if strncmp(haystack.add(i), needle, needle_len) == 0 {
            return haystack.add(i);
        }
    }

    ptr::null()
}

// ============================================================================
// Memory Operations
// ============================================================================

/// Set n bytes of memory to value c
///
/// # Safety
/// `s` must point to at least `n` writable bytes.
#[inline]
pub unsafe fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if s.is_null() || n == 0 {
        return s;
    }

    let c = c as u8;
    ptr::write_bytes(s, c, n);
    s
}

/// Copy n bytes from src to dest
///
/// # Safety
/// - Both pointers must be valid for n bytes
/// - Buffers must not overlap (use memmove for overlapping)
#[inline]
pub unsafe fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    ptr::copy_nonoverlapping(src, dest, n);
    dest
}

/// Copy n bytes handling overlapping buffers
///
/// # Safety
/// Both pointers must be valid for n bytes.
#[inline]
pub unsafe fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    ptr::copy(src, dest, n);
    dest
}

/// Compare n bytes of memory
///
/// # Safety
/// Both pointers must be valid for n bytes.
#[inline]
pub unsafe fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }

    if s1.is_null() || s2.is_null() {
        if s1.is_null() && s2.is_null() {
            return 0;
        }
        return if s1.is_null() { -1 } else { 1 };
    }

    for i in 0..n {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);
        if c1 != c2 {
            return c1 as i32 - c2 as i32;
        }
    }

    0
}

/// Find byte in memory region
///
/// # Safety
/// `s` must point to at least `n` readable bytes.
#[inline]
pub unsafe fn memchr(s: *const u8, c: i32, n: usize) -> *const u8 {
    if s.is_null() || n == 0 {
        return ptr::null();
    }

    let c = c as u8;
    for i in 0..n {
        if *s.add(i) == c {
            return s.add(i);
        }
    }

    ptr::null()
}

// ============================================================================
// Safe Rust Wrappers
// ============================================================================

/// Safe string length for byte slices
pub fn safe_strlen(s: &[u8]) -> usize {
    s.iter().position(|&c| c == 0).unwrap_or(s.len())
}

/// Safe string comparison for byte slices
pub fn safe_strcmp(s1: &[u8], s2: &[u8]) -> i32 {
    let len1 = safe_strlen(s1);
    let len2 = safe_strlen(s2);

    for i in 0..len1.min(len2) {
        if s1[i] != s2[i] {
            return s1[i] as i32 - s2[i] as i32;
        }
    }

    len1 as i32 - len2 as i32
}

/// Check if byte slice starts with prefix
pub fn starts_with(s: &[u8], prefix: &[u8]) -> bool {
    if prefix.len() > s.len() {
        return false;
    }
    &s[..prefix.len()] == prefix
}

/// Check if byte slice ends with suffix
pub fn ends_with(s: &[u8], suffix: &[u8]) -> bool {
    if suffix.len() > s.len() {
        return false;
    }
    &s[s.len() - suffix.len()..] == suffix
}

/// Find substring in byte slice
pub fn find_substr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }

    haystack.windows(needle.len()).position(|w| w == needle)
}

// ============================================================================
// Kernel-Specific String Functions
// ============================================================================

/// Copy string from user space with bounds checking
///
/// # Safety
/// - `user_src` must be a valid user-space address
/// - `kern_dest` must have space for `max_len` bytes
pub unsafe fn copyinstr(
    user_src: *const u8,
    kern_dest: *mut u8,
    max_len: usize,
) -> Result<usize, StringError> {
    if user_src.is_null() || kern_dest.is_null() || max_len == 0 {
        return Err(StringError::InvalidArgument);
    }

    // In real kernel, would validate user_src is in user address space
    // and handle page faults gracefully

    for i in 0..max_len {
        let c = *user_src.add(i);
        *kern_dest.add(i) = c;
        if c == 0 {
            return Ok(i + 1); // Include null terminator in count
        }
    }

    // String too long - null terminate anyway
    *kern_dest.add(max_len - 1) = 0;
    Err(StringError::NameTooLong)
}

/// Copy string to user space with bounds checking
///
/// # Safety
/// - `kern_src` must be a valid kernel string
/// - `user_dest` must be a valid user-space address with `max_len` bytes
pub unsafe fn copyoutstr(
    kern_src: *const u8,
    user_dest: *mut u8,
    max_len: usize,
) -> Result<usize, StringError> {
    if kern_src.is_null() || user_dest.is_null() || max_len == 0 {
        return Err(StringError::InvalidArgument);
    }

    // In real kernel, would validate user_dest is in user address space

    for i in 0..max_len {
        let c = *kern_src.add(i);
        *user_dest.add(i) = c;
        if c == 0 {
            return Ok(i + 1);
        }
    }

    *user_dest.add(max_len - 1) = 0;
    Err(StringError::NameTooLong)
}

/// Error types for string operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringError {
    /// Invalid argument (null pointer, zero length)
    InvalidArgument,
    /// String exceeds maximum length
    NameTooLong,
    /// Memory access fault
    Fault,
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the strings module
pub fn init() {
    // String functions are pure computations, no initialization needed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strlen() {
        unsafe {
            let s = b"hello\0";
            assert_eq!(strlen(s.as_ptr()), 5);

            let empty = b"\0";
            assert_eq!(strlen(empty.as_ptr()), 0);
        }
    }

    #[test]
    fn test_strnlen() {
        unsafe {
            let s = b"hello\0";
            assert_eq!(strnlen(s.as_ptr(), 10), 5);
            assert_eq!(strnlen(s.as_ptr(), 3), 3);
            assert_eq!(strnlen(s.as_ptr(), 0), 0);
        }
    }

    #[test]
    fn test_strcmp() {
        unsafe {
            let a = b"abc\0";
            let b = b"abc\0";
            let c = b"abd\0";
            let d = b"ab\0";

            assert_eq!(strcmp(a.as_ptr(), b.as_ptr()), 0);
            assert!(strcmp(a.as_ptr(), c.as_ptr()) < 0);
            assert!(strcmp(c.as_ptr(), a.as_ptr()) > 0);
            assert!(strcmp(a.as_ptr(), d.as_ptr()) > 0);
        }
    }

    #[test]
    fn test_strncmp() {
        unsafe {
            let a = b"abcdef\0";
            let b = b"abcxyz\0";

            assert_eq!(strncmp(a.as_ptr(), b.as_ptr(), 3), 0);
            assert!(strncmp(a.as_ptr(), b.as_ptr(), 4) < 0);
        }
    }

    #[test]
    fn test_strcpy() {
        unsafe {
            let mut dest = [0u8; 16];
            let src = b"hello\0";

            strcpy(dest.as_mut_ptr(), src.as_ptr());
            assert_eq!(&dest[..6], b"hello\0");
        }
    }

    #[test]
    fn test_strncpy() {
        unsafe {
            let mut dest = [0xFFu8; 16];
            let src = b"hello\0";

            strncpy(dest.as_mut_ptr(), src.as_ptr(), 10);
            assert_eq!(&dest[..10], b"hello\0\0\0\0\0");
        }
    }

    #[test]
    fn test_strlcpy() {
        unsafe {
            let mut dest = [0u8; 4];
            let src = b"hello\0";

            let len = strlcpy(dest.as_mut_ptr(), src.as_ptr(), 4);
            assert_eq!(len, 5);
            assert_eq!(&dest[..4], b"hel\0");
        }
    }

    #[test]
    fn test_strchr() {
        unsafe {
            let s = b"hello\0";

            let result = strchr(s.as_ptr(), 'l' as i32);
            assert!(!result.is_null());
            assert_eq!(result.offset_from(s.as_ptr()), 2);

            let not_found = strchr(s.as_ptr(), 'x' as i32);
            assert!(not_found.is_null());
        }
    }

    #[test]
    fn test_strrchr() {
        unsafe {
            let s = b"hello\0";

            let result = strrchr(s.as_ptr(), 'l' as i32);
            assert!(!result.is_null());
            assert_eq!(result.offset_from(s.as_ptr()), 3);
        }
    }

    #[test]
    fn test_strstr() {
        unsafe {
            let haystack = b"hello world\0";
            let needle = b"world\0";

            let result = strstr(haystack.as_ptr(), needle.as_ptr());
            assert!(!result.is_null());
            assert_eq!(result.offset_from(haystack.as_ptr()), 6);

            let not_found = strstr(haystack.as_ptr(), b"xyz\0".as_ptr());
            assert!(not_found.is_null());
        }
    }

    #[test]
    fn test_memset() {
        unsafe {
            let mut buf = [0u8; 8];
            memset(buf.as_mut_ptr(), 0xAA, 4);
            assert_eq!(&buf, &[0xAA, 0xAA, 0xAA, 0xAA, 0, 0, 0, 0]);
        }
    }

    #[test]
    fn test_memcpy() {
        unsafe {
            let src = [1u8, 2, 3, 4, 5];
            let mut dest = [0u8; 5];

            memcpy(dest.as_mut_ptr(), src.as_ptr(), 5);
            assert_eq!(dest, src);
        }
    }

    #[test]
    fn test_memcmp() {
        unsafe {
            let a = [1u8, 2, 3, 4];
            let b = [1u8, 2, 3, 4];
            let c = [1u8, 2, 4, 4];

            assert_eq!(memcmp(a.as_ptr(), b.as_ptr(), 4), 0);
            assert!(memcmp(a.as_ptr(), c.as_ptr(), 4) < 0);
        }
    }

    #[test]
    fn test_memchr() {
        unsafe {
            let buf = [1u8, 2, 3, 4, 5];

            let result = memchr(buf.as_ptr(), 3, 5);
            assert!(!result.is_null());
            assert_eq!(result.offset_from(buf.as_ptr()), 2);

            let not_found = memchr(buf.as_ptr(), 9, 5);
            assert!(not_found.is_null());
        }
    }

    #[test]
    fn test_safe_strlen() {
        assert_eq!(safe_strlen(b"hello\0world"), 5);
        assert_eq!(safe_strlen(b"hello"), 5);
        assert_eq!(safe_strlen(b"\0"), 0);
    }

    #[test]
    fn test_safe_strcmp() {
        assert_eq!(safe_strcmp(b"abc\0", b"abc\0"), 0);
        assert!(safe_strcmp(b"abc\0", b"abd\0") < 0);
        assert!(safe_strcmp(b"abd\0", b"abc\0") > 0);
    }

    #[test]
    fn test_starts_ends_with() {
        assert!(starts_with(b"hello world", b"hello"));
        assert!(!starts_with(b"hello world", b"world"));
        assert!(ends_with(b"hello world", b"world"));
        assert!(!ends_with(b"hello world", b"hello"));
    }

    #[test]
    fn test_find_substr() {
        assert_eq!(find_substr(b"hello world", b"world"), Some(6));
        assert_eq!(find_substr(b"hello world", b"hello"), Some(0));
        assert_eq!(find_substr(b"hello world", b"xyz"), None);
        assert_eq!(find_substr(b"hello world", b""), Some(0));
    }
}
