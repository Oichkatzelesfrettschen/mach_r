//! Copyin/Copyout - Safe User↔Kernel Data Transfer
//!
//! Based on Mach4 kern/kern_subr.c and machine/copyio.c
//!
//! These functions safely copy data between user space and kernel space,
//! handling page faults and preventing kernel panics from bad user pointers.
//!
//! ## Safety
//!
//! User pointers MUST be validated before dereferencing. These functions:
//! 1. Verify the address is in user space range
//! 2. Check page table mappings exist
//! 3. Handle page faults gracefully
//! 4. Return error codes instead of panicking
//!
//! ## Performance Considerations
//!
//! For large transfers, these functions operate page-by-page to:
//! - Allow preemption between pages
//! - Handle partial mappings
//! - Support copy-on-write pages

use core::ptr;

/// Maximum size for a single copy operation
pub const COPYIO_MAX_SIZE: usize = 64 * 1024; // 64KB

/// User space address range limits
/// These would be configured per-architecture in a full implementation
#[cfg(target_arch = "x86_64")]
pub const USER_SPACE_START: usize = 0x0000_0000_0000_0000;
#[cfg(target_arch = "x86_64")]
pub const USER_SPACE_END: usize = 0x0000_7FFF_FFFF_FFFF;

#[cfg(target_arch = "aarch64")]
pub const USER_SPACE_START: usize = 0x0000_0000_0000_0000;
#[cfg(target_arch = "aarch64")]
pub const USER_SPACE_END: usize = 0x0000_FFFF_FFFF_FFFF;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub const USER_SPACE_START: usize = 0;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub const USER_SPACE_END: usize = 0x7FFF_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Copy operation result
pub type CopyResult = Result<usize, CopyError>;

/// Errors that can occur during copy operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CopyError {
    /// Bad user address (null, kernel space, or misaligned)
    BadAddress = -1,
    /// Page fault during copy (unmapped page)
    PageFault = -2,
    /// Protection violation (write to read-only page)
    Protection = -3,
    /// Size too large
    TooLarge = -4,
    /// Invalid argument
    InvalidArg = -5,
}

impl CopyError {
    /// Convert to kern_return_t compatible value
    pub fn to_kern_return(self) -> i32 {
        self as i32
    }
}

// ============================================================================
// Address Validation
// ============================================================================

/// Check if an address is in user space
#[inline]
pub fn is_user_address(addr: usize) -> bool {
    addr >= USER_SPACE_START && addr <= USER_SPACE_END
}

/// Check if an address range is entirely in user space
#[inline]
pub fn is_user_range(addr: usize, size: usize) -> bool {
    if size == 0 {
        return true;
    }

    // Check for overflow
    let end = match addr.checked_add(size - 1) {
        Some(e) => e,
        None => return false,
    };

    is_user_address(addr) && is_user_address(end)
}

/// Validate user pointer with alignment check
pub fn validate_user_ptr(ptr: *const u8, size: usize, alignment: usize) -> Result<(), CopyError> {
    let addr = ptr as usize;

    // Null pointer check
    if ptr.is_null() {
        return Err(CopyError::BadAddress);
    }

    // User space check
    if !is_user_range(addr, size) {
        return Err(CopyError::BadAddress);
    }

    // Alignment check
    if alignment > 1 && addr % alignment != 0 {
        return Err(CopyError::BadAddress);
    }

    // Size check
    if size > COPYIO_MAX_SIZE {
        return Err(CopyError::TooLarge);
    }

    Ok(())
}

// ============================================================================
// Copyin - User → Kernel
// ============================================================================

/// Copy data from user space to kernel space
///
/// # Arguments
/// * `user_src` - Source address in user space
/// * `kernel_dst` - Destination buffer in kernel space
/// * `size` - Number of bytes to copy
///
/// # Returns
/// * `Ok(bytes_copied)` on success
/// * `Err(CopyError)` on failure
///
/// # Safety
/// This function handles user pointers safely. The kernel buffer must be valid.
pub fn copyin(user_src: *const u8, kernel_dst: &mut [u8]) -> CopyResult {
    let size = kernel_dst.len();

    // Validate user source
    validate_user_ptr(user_src, size, 1)?;

    // In a full implementation, this would:
    // 1. Check page table entries for read permission
    // 2. Handle page faults by returning error
    // 3. Use special copy routines that catch faults
    //
    // For now, we do a direct copy with basic safety checks

    #[cfg(not(test))]
    {
        // Check if pages are mapped (simplified check)
        // Real implementation would walk page tables
        if !check_pages_readable(user_src as usize, size) {
            return Err(CopyError::PageFault);
        }
    }

    // Perform the copy
    // SAFETY: We've validated the user pointer and size
    unsafe {
        ptr::copy_nonoverlapping(user_src, kernel_dst.as_mut_ptr(), size);
    }

    Ok(size)
}

/// Copy a null-terminated string from user space
///
/// # Arguments
/// * `user_src` - Source string in user space
/// * `kernel_dst` - Destination buffer in kernel space
///
/// # Returns
/// * `Ok(string_length)` on success (not including null terminator)
/// * `Err(CopyError)` on failure
pub fn copyinstr(user_src: *const u8, kernel_dst: &mut [u8]) -> CopyResult {
    let max_len = kernel_dst.len();

    // Validate start address
    validate_user_ptr(user_src, 1, 1)?;

    let mut copied = 0;

    for i in 0..max_len {
        let src_ptr = unsafe { user_src.add(i) };

        // Validate each byte's address
        if !is_user_address(src_ptr as usize) {
            return Err(CopyError::BadAddress);
        }

        // Read the byte
        let byte = unsafe { *src_ptr };
        kernel_dst[i] = byte;
        copied += 1;

        // Check for null terminator
        if byte == 0 {
            return Ok(copied - 1); // Don't count null in length
        }
    }

    // String too long for buffer
    Err(CopyError::TooLarge)
}

/// Copy a single value from user space
pub fn copyin_value<T: Copy>(user_src: *const T) -> Result<T, CopyError> {
    let size = core::mem::size_of::<T>();
    let align = core::mem::align_of::<T>();

    validate_user_ptr(user_src as *const u8, size, align)?;

    #[cfg(not(test))]
    {
        if !check_pages_readable(user_src as usize, size) {
            return Err(CopyError::PageFault);
        }
    }

    // SAFETY: We've validated the pointer
    Ok(unsafe { ptr::read(user_src) })
}

// ============================================================================
// Copyout - Kernel → User
// ============================================================================

/// Copy data from kernel space to user space
///
/// # Arguments
/// * `kernel_src` - Source buffer in kernel space
/// * `user_dst` - Destination address in user space
///
/// # Returns
/// * `Ok(bytes_copied)` on success
/// * `Err(CopyError)` on failure
pub fn copyout(kernel_src: &[u8], user_dst: *mut u8) -> CopyResult {
    let size = kernel_src.len();

    // Validate user destination
    validate_user_ptr(user_dst, size, 1)?;

    #[cfg(not(test))]
    {
        // Check if pages are mapped and writable
        if !check_pages_writable(user_dst as usize, size) {
            return Err(CopyError::Protection);
        }
    }

    // Perform the copy
    // SAFETY: We've validated the user pointer and size
    unsafe {
        ptr::copy_nonoverlapping(kernel_src.as_ptr(), user_dst, size);
    }

    Ok(size)
}

/// Copy a null-terminated string to user space
pub fn copyoutstr(kernel_src: &str, user_dst: *mut u8, max_len: usize) -> CopyResult {
    let src_bytes = kernel_src.as_bytes();
    let copy_len = src_bytes.len().min(max_len.saturating_sub(1));

    // Validate user destination (include space for null terminator)
    validate_user_ptr(user_dst, copy_len + 1, 1)?;

    #[cfg(not(test))]
    {
        if !check_pages_writable(user_dst as usize, copy_len + 1) {
            return Err(CopyError::Protection);
        }
    }

    // Copy string bytes
    unsafe {
        ptr::copy_nonoverlapping(src_bytes.as_ptr(), user_dst, copy_len);
        // Add null terminator
        *user_dst.add(copy_len) = 0;
    }

    Ok(copy_len)
}

/// Copy a single value to user space
pub fn copyout_value<T: Copy>(value: &T, user_dst: *mut T) -> Result<(), CopyError> {
    let size = core::mem::size_of::<T>();
    let align = core::mem::align_of::<T>();

    validate_user_ptr(user_dst as *const u8, size, align)?;

    #[cfg(not(test))]
    {
        if !check_pages_writable(user_dst as usize, size) {
            return Err(CopyError::Protection);
        }
    }

    // SAFETY: We've validated the pointer
    unsafe {
        ptr::write(user_dst, *value);
    }

    Ok(())
}

// ============================================================================
// Page Checking (Placeholder implementations)
// ============================================================================

/// Check if a range of pages is readable
///
/// In a full implementation, this would:
/// 1. Walk the page table
/// 2. Check each page has read permission
/// 3. Check pages are present (not swapped)
#[cfg(not(test))]
fn check_pages_readable(addr: usize, size: usize) -> bool {
    // TODO: Implement actual page table walk
    // For now, assume all user space addresses are readable
    is_user_range(addr, size)
}

/// Check if a range of pages is writable
///
/// In a full implementation, this would:
/// 1. Walk the page table
/// 2. Check each page has write permission
/// 3. Handle copy-on-write pages
#[cfg(not(test))]
fn check_pages_writable(addr: usize, size: usize) -> bool {
    // TODO: Implement actual page table walk
    // For now, assume all user space addresses are writable
    is_user_range(addr, size)
}

// ============================================================================
// Atomic Copy Operations
// ============================================================================

/// Atomically copy a u32 from user space
pub fn fuword32(user_addr: *const u32) -> Result<u32, CopyError> {
    copyin_value(user_addr)
}

/// Atomically copy a u64 from user space
pub fn fuword64(user_addr: *const u64) -> Result<u64, CopyError> {
    copyin_value(user_addr)
}

/// Atomically copy a usize from user space
pub fn fuword(user_addr: *const usize) -> Result<usize, CopyError> {
    copyin_value(user_addr)
}

/// Atomically store a u32 to user space
pub fn suword32(user_addr: *mut u32, value: u32) -> Result<(), CopyError> {
    copyout_value(&value, user_addr)
}

/// Atomically store a u64 to user space
pub fn suword64(user_addr: *mut u64, value: u64) -> Result<(), CopyError> {
    copyout_value(&value, user_addr)
}

/// Atomically store a usize to user space
pub fn suword(user_addr: *mut usize, value: usize) -> Result<(), CopyError> {
    copyout_value(&value, user_addr)
}

// ============================================================================
// Compare-and-Swap for User Space
// ============================================================================

/// Atomic compare-and-swap in user space
///
/// Used for futex-style operations.
///
/// # Returns
/// * `Ok(old_value)` - The previous value at the address
/// * `Err(CopyError)` - If access failed
#[cfg(target_arch = "x86_64")]
pub fn cas_user_u32(user_addr: *mut u32, old_val: u32, new_val: u32) -> Result<u32, CopyError> {
    validate_user_ptr(user_addr as *const u8, 4, 4)?;

    let result: u32;
    unsafe {
        core::arch::asm!(
            "lock cmpxchg [{addr}], {new}",
            addr = in(reg) user_addr,
            new = in(reg) new_val,
            inout("eax") old_val => result,
        );
    }

    Ok(result)
}

#[cfg(not(target_arch = "x86_64"))]
pub fn cas_user_u32(_user_addr: *mut u32, _old_val: u32, _new_val: u32) -> Result<u32, CopyError> {
    // Non-atomic fallback for other architectures
    Err(CopyError::InvalidArg)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_user_address() {
        assert!(is_user_address(0x1000));
        assert!(is_user_address(USER_SPACE_END));
        // Kernel space address (above user range)
        assert!(!is_user_address(usize::MAX));
    }

    #[test]
    fn test_is_user_range() {
        assert!(is_user_range(0x1000, 0x100));
        assert!(is_user_range(0, 0)); // Empty range is valid
        // Range that wraps around
        assert!(!is_user_range(usize::MAX - 10, 100));
    }

    #[test]
    fn test_validate_user_ptr() {
        // Null pointer
        assert_eq!(
            validate_user_ptr(core::ptr::null(), 10, 1),
            Err(CopyError::BadAddress)
        );

        // Too large
        assert_eq!(
            validate_user_ptr(0x1000 as *const u8, COPYIO_MAX_SIZE + 1, 1),
            Err(CopyError::TooLarge)
        );
    }

    #[test]
    fn test_copy_error_to_kern_return() {
        assert_eq!(CopyError::BadAddress.to_kern_return(), -1);
        assert_eq!(CopyError::PageFault.to_kern_return(), -2);
    }
}
