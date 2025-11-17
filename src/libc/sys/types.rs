//! POSIX sys/types.h - basic system data types

#[allow(non_camel_case_types)]
pub type size_t = usize;
#[allow(non_camel_case_types)]
pub type ssize_t = isize;
#[allow(non_camel_case_types)]
pub type off_t = i64;
#[allow(non_camel_case_types)]
pub type pid_t = i32;
#[allow(non_camel_case_types)]
pub type uid_t = u32;
#[allow(non_camel_case_types)]
pub type gid_t = u32;
#[allow(non_camel_case_types)]
pub type mode_t = u32;
#[allow(non_camel_case_types)]
pub type dev_t = u32;
#[allow(non_camel_case_types)]
pub type ino_t = u64;
#[allow(non_camel_case_types)]
pub type nlink_t = u32;
#[allow(non_camel_case_types)]
pub type blksize_t = i32;
#[allow(non_camel_case_types)]
pub type blkcnt_t = i64;
#[allow(non_camel_case_types)]
pub type time_t = i64;