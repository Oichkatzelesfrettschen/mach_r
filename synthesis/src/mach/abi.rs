//! Core Mach message constants and types (clean-room definitions)
//!
//! Reference: message.h, mach_types.h from OSFMK/Mach3 archives.
//! Used to align our IPC and MIG layer with well-known Mach semantics.

pub type MachMsgBits = u32;
pub type MachMsgSize = u32;
pub type MachMsgId = i32;

// Header bit layout helpers (subset)
pub const MACH_MSGH_BITS_REMOTE_MASK: u32 = 0x000000ff;
pub const MACH_MSGH_BITS_LOCAL_MASK: u32 = 0x0000ff00;
pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x8000_0000;

#[inline]
pub const fn mach_msgh_bits(remote: u32, local: u32) -> u32 {
    (remote & 0xff) | ((local & 0xff) << 8)
}
#[inline]
pub const fn mach_msgh_bits_remote(bits: u32) -> u32 { bits & MACH_MSGH_BITS_REMOTE_MASK }
#[inline]
pub const fn mach_msgh_bits_local(bits: u32) -> u32 { (bits & MACH_MSGH_BITS_LOCAL_MASK) >> 8 }

// Port right disposition (subset)
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MachMsgTypeName {
    MoveReceive = 16,
    MoveSend = 17,
    MoveSendOnce = 18,
    CopySend = 19,
    MakeSend = 20,
    MakeSendOnce = 21,
}

// Descriptor type (subset)
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MachMsgDescriptorType {
    Port = 0,
    Ool = 1,
    OolPorts = 2,
}

// Copy semantics (subset)
pub const MACH_MSG_PHYSICAL_COPY: u32 = 0;
pub const MACH_MSG_VIRTUAL_COPY: u32 = 1;
pub const MACH_MSG_ALLOCATE: u32 = 2;
pub const MACH_MSG_OVERWRITE: u32 = 3;
