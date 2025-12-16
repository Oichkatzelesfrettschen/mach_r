//! POSIX sys/stat.h - file status

use super::types::*;

/// File status structure
#[repr(C)]
pub struct stat {
    pub st_dev: dev_t,         // Device ID
    pub st_ino: ino_t,         // Inode number
    pub st_mode: mode_t,       // File mode
    pub st_nlink: nlink_t,     // Number of hard links
    pub st_uid: uid_t,         // User ID
    pub st_gid: gid_t,         // Group ID
    pub st_rdev: dev_t,        // Device ID (if special file)
    pub st_size: off_t,        // File size in bytes
    pub st_blksize: blksize_t, // Block size
    pub st_blocks: blkcnt_t,   // Number of 512B blocks
    pub st_atime: time_t,      // Access time
    pub st_mtime: time_t,      // Modification time
    pub st_ctime: time_t,      // Status change time
}

// File type constants
pub const S_IFMT: mode_t = 0o170000; // File type mask
pub const S_IFREG: mode_t = 0o100000; // Regular file
pub const S_IFDIR: mode_t = 0o040000; // Directory
pub const S_IFCHR: mode_t = 0o020000; // Character special
pub const S_IFBLK: mode_t = 0o060000; // Block special
pub const S_IFIFO: mode_t = 0o010000; // FIFO special
pub const S_IFLNK: mode_t = 0o120000; // Symbolic link
pub const S_IFSOCK: mode_t = 0o140000; // Socket

// File type test macros (as functions) - POSIX standard names
#[allow(non_snake_case)]
pub const fn S_ISREG(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFREG
}

#[allow(non_snake_case)]
pub const fn S_ISDIR(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFDIR
}

#[allow(non_snake_case)]
pub const fn S_ISCHR(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFCHR
}

#[allow(non_snake_case)]
pub const fn S_ISBLK(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFBLK
}

#[allow(non_snake_case)]
pub const fn S_ISFIFO(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFIFO
}

#[allow(non_snake_case)]
pub const fn S_ISLNK(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFLNK
}

#[allow(non_snake_case)]
pub const fn S_ISSOCK(mode: mode_t) -> bool {
    (mode & S_IFMT) == S_IFSOCK
}

/// Get file status
#[no_mangle]
pub extern "C" fn stat(pathname: *const u8, statbuf: *mut stat) -> i32 {
    if pathname.is_null() || statbuf.is_null() {
        return crate::libc::set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [pathname as usize, statbuf as usize];
    crate::libc::trap_result_to_c(crate::trap::trap_dispatch(4, &args)) as i32 // SYS_stat
}

/// Get file status by file descriptor
#[no_mangle]
pub extern "C" fn fstat(fd: i32, statbuf: *mut stat) -> i32 {
    if statbuf.is_null() {
        return crate::libc::set_errno_and_fail(crate::trap::TrapError::InvalidArgument);
    }

    let args = [fd as usize, statbuf as usize];
    crate::libc::trap_result_to_c(crate::trap::trap_dispatch(5, &args)) as i32 // SYS_fstat
}
