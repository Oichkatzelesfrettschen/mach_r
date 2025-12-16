//! Mach Message System Call
//!
//! Based on Mach4 ipc/mach_msg.c by CMU (1987-1991)
//!
//! This module implements the mach_msg() system call, which is the fundamental
//! IPC primitive in Mach. It handles both message sending and receiving in
//! a single system call for efficiency.
//!
//! ## Message Format
//!
//! Mach messages consist of:
//! - Header: Fixed-size metadata (destination, reply port, size)
//! - Body: Optional data, port rights, and out-of-line memory
//!
//! ## Options
//!
//! The mach_msg() call supports several options:
//! - MACH_SEND_MSG: Send a message
//! - MACH_RCV_MSG: Receive a message
//! - MACH_SEND_TIMEOUT: Timeout for send operation
//! - MACH_RCV_TIMEOUT: Timeout for receive operation
//! - MACH_RCV_LARGE: Allow receiving oversized messages
//!
//! ## Combined Send/Receive
//!
//! When both SEND and RCV are specified, the message is sent first,
//! then a response is received (RPC optimization).

use alloc::sync::Arc;
use core::time::Duration;
use spin::Mutex;

use crate::ipc::kmsg::{kmsg_alloc, IpcKmsg};
use crate::ipc::port::Port;
use crate::ipc::right::{copyin, copyout, MsgTypeName};
use crate::ipc::space::IpcSpace;
use crate::ipc::{IpcError, IpcResult, PortName};

// ============================================================================
// Message Option Flags
// ============================================================================

/// Send a message
pub const MACH_SEND_MSG: u32 = 0x00000001;

/// Receive a message
pub const MACH_RCV_MSG: u32 = 0x00000002;

/// Send/receive timeout is specified
pub const MACH_SEND_TIMEOUT: u32 = 0x00000010;
pub const MACH_RCV_TIMEOUT: u32 = 0x00000100;

/// Cancel any pending operation
pub const MACH_SEND_CANCEL: u32 = 0x00000080;
pub const MACH_RCV_CANCEL: u32 = 0x00000800;

/// Notify on failure
pub const MACH_SEND_NOTIFY: u32 = 0x00000020;

/// Use large buffer for oversized messages
pub const MACH_RCV_LARGE: u32 = 0x00000200;

/// Receive message without removing from queue (peek)
pub const MACH_PEEK_MSG: u32 = 0x00000400;

/// Interruptible operation
pub const MACH_SEND_INTERRUPT: u32 = 0x00000040;
pub const MACH_RCV_INTERRUPT: u32 = 0x00000400;

// ============================================================================
// Message Return Codes
// ============================================================================

/// Operation successful
pub const MACH_MSG_SUCCESS: i32 = 0;

/// Invalid data (message, buffer, etc.)
pub const MACH_SEND_INVALID_DATA: i32 = 0x10000002;
/// Invalid destination port
pub const MACH_SEND_INVALID_DEST: i32 = 0x10000003;
/// Operation timed out
pub const MACH_SEND_TIMED_OUT: i32 = 0x10000004;
/// Operation interrupted
pub const MACH_SEND_INTERRUPTED: i32 = 0x10000007;
/// Message too large for port queue
pub const MACH_SEND_MSG_TOO_SMALL: i32 = 0x10000008;
/// Invalid reply port
pub const MACH_SEND_INVALID_REPLY: i32 = 0x10000009;
/// Invalid port right
pub const MACH_SEND_INVALID_RIGHT: i32 = 0x1000000a;
/// Notification port dead
pub const MACH_SEND_NOTIFY_IN_PROGRESS: i32 = 0x1000000b;
/// Invalid header
pub const MACH_SEND_INVALID_HEADER: i32 = 0x1000000d;
/// Message size limit exceeded
pub const MACH_SEND_MSG_SIZE_INVALID: i32 = 0x1000000e;
/// Resource shortage
pub const MACH_SEND_NO_BUFFER: i32 = 0x1000000f;

/// Invalid receive name
pub const MACH_RCV_INVALID_NAME: i32 = 0x10004002;
/// Operation timed out
pub const MACH_RCV_TIMED_OUT: i32 = 0x10004003;
/// Message too large for buffer
pub const MACH_RCV_TOO_LARGE: i32 = 0x10004004;
/// Operation interrupted
pub const MACH_RCV_INTERRUPTED: i32 = 0x10004005;
/// Port changed (moved to different set)
pub const MACH_RCV_PORT_CHANGED: i32 = 0x10004006;
/// Invalid notification port
pub const MACH_RCV_INVALID_NOTIFY: i32 = 0x10004007;
/// Invalid data
pub const MACH_RCV_INVALID_DATA: i32 = 0x10004008;
/// Port died during receive
pub const MACH_RCV_PORT_DIED: i32 = 0x10004009;
/// In-progress operation
pub const MACH_RCV_IN_PROGRESS: i32 = 0x1000400d;
/// Header error
pub const MACH_RCV_HEADER_ERROR: i32 = 0x1000400e;
/// Body error
pub const MACH_RCV_BODY_ERROR: i32 = 0x1000400f;

// ============================================================================
// Message Size Constants
// ============================================================================

/// Minimum message size (just header)
pub const MACH_MSG_SIZE_MIN: usize = core::mem::size_of::<MachMsgHeader>();

/// Maximum inline message size
pub const MACH_MSG_SIZE_MAX: usize = 64 * 1024; // 64KB

/// Maximum number of descriptors
pub const MACH_MSG_DESC_MAX: usize = 256;

// ============================================================================
// Message Types
// ============================================================================

/// Message type bits (in msgh_bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MachMsgBits(pub u32);

impl MachMsgBits {
    /// Remote port type mask
    pub const REMOTE_MASK: u32 = 0x0000001f;
    /// Local port type mask
    pub const LOCAL_MASK: u32 = 0x00001f00;
    /// Voucher port type mask
    pub const VOUCHER_MASK: u32 = 0x001f0000;

    /// Complex message (has descriptors)
    pub const COMPLEX: u32 = 0x80000000;

    /// Create new message bits
    pub fn new(remote: MsgType, local: MsgType) -> Self {
        Self((remote as u32) | ((local as u32) << 8))
    }

    /// Get remote port type
    pub fn remote_type(self) -> MsgType {
        MsgType::from_raw(self.0 & Self::REMOTE_MASK)
    }

    /// Get local port type
    pub fn local_type(self) -> MsgType {
        MsgType::from_raw((self.0 >> 8) & 0x1f)
    }

    /// Check if message is complex
    pub fn is_complex(self) -> bool {
        (self.0 & Self::COMPLEX) != 0
    }

    /// Set complex bit
    pub fn set_complex(&mut self) {
        self.0 |= Self::COMPLEX;
    }
}

/// Port right types for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum MsgType {
    /// No port right
    #[default]
    None = 0,
    /// Move receive right
    MoveReceive = 16,
    /// Move send right
    MoveSend = 17,
    /// Move send-once right
    MoveSendOnce = 18,
    /// Copy send right
    CopySend = 19,
    /// Make send right
    MakeSend = 20,
    /// Make send-once right
    MakeSendOnce = 21,
}

impl MsgType {
    /// Convert from raw value
    pub fn from_raw(value: u32) -> Self {
        match value {
            16 => Self::MoveReceive,
            17 => Self::MoveSend,
            18 => Self::MoveSendOnce,
            19 => Self::CopySend,
            20 => Self::MakeSend,
            21 => Self::MakeSendOnce,
            _ => Self::None,
        }
    }

    /// Check if this transfers a right
    pub fn is_move(self) -> bool {
        matches!(
            self,
            Self::MoveReceive | Self::MoveSend | Self::MoveSendOnce
        )
    }

    /// Check if this copies a right
    pub fn is_copy(self) -> bool {
        matches!(self, Self::CopySend)
    }

    /// Check if this creates a right
    pub fn is_make(self) -> bool {
        matches!(self, Self::MakeSend | Self::MakeSendOnce)
    }
}

// ============================================================================
// Message Header
// ============================================================================

/// Mach message header
///
/// This is the fixed header that begins every Mach message.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct MachMsgHeader {
    /// Message bits (port types, complexity)
    pub msgh_bits: MachMsgBits,
    /// Total message size in bytes
    pub msgh_size: u32,
    /// Destination port name
    pub msgh_remote_port: PortName,
    /// Reply port name
    pub msgh_local_port: PortName,
    /// Voucher port (reserved)
    pub msgh_voucher_port: PortName,
    /// Message ID (application-defined)
    pub msgh_id: i32,
}

impl MachMsgHeader {
    /// Create a simple (non-complex) message header
    pub fn simple(
        dest: PortName,
        reply: PortName,
        remote_type: MsgType,
        local_type: MsgType,
        size: u32,
        id: i32,
    ) -> Self {
        Self {
            msgh_bits: MachMsgBits::new(remote_type, local_type),
            msgh_size: size,
            msgh_remote_port: dest,
            msgh_local_port: reply,
            msgh_voucher_port: PortName::NULL,
            msgh_id: id,
        }
    }

    /// Check if this is a valid header
    pub fn is_valid(&self) -> bool {
        // Size must be at least header size
        if (self.msgh_size as usize) < MACH_MSG_SIZE_MIN {
            return false;
        }

        // Size must not exceed maximum
        if (self.msgh_size as usize) > MACH_MSG_SIZE_MAX {
            return false;
        }

        // Destination must be valid for send
        // (PortName::NULL is allowed for some operations)

        true
    }
}

// ============================================================================
// Message Body Descriptor Types
// ============================================================================

/// Descriptor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum DescriptorType {
    #[default]
    Port = 0,
    OutOfLine = 1,
    OutOfLineVolatile = 2,
    OutOfLinePorts = 3,
}

/// Port descriptor (for transferring port rights)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct MachMsgPortDescriptor {
    /// Port name
    pub name: PortName,
    /// Padding
    pub _pad1: u32,
    /// Type disposition
    pub disposition: u32,
    /// Descriptor type (must be Port)
    pub dtype: DescriptorType,
}

/// Out-of-line data descriptor
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct MachMsgOolDescriptor {
    /// Address of data
    pub address: u64,
    /// Size of data
    pub size: u32,
    /// Deallocate source after copy?
    pub deallocate: bool,
    /// Copy method
    pub copy: u8,
    /// Padding
    pub _pad: u16,
    /// Descriptor type (must be OutOfLine)
    pub dtype: DescriptorType,
}

// ============================================================================
// Message Body
// ============================================================================

/// Complex message body header
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct MachMsgBody {
    /// Number of descriptors that follow
    pub msgh_descriptor_count: u32,
}

// ============================================================================
// mach_msg Options
// ============================================================================

/// Options for mach_msg operation
#[derive(Debug, Clone, Copy, Default)]
pub struct MachMsgOptions {
    /// Option flags
    pub flags: u32,
    /// Send timeout (if MACH_SEND_TIMEOUT set)
    pub send_timeout: Option<Duration>,
    /// Receive timeout (if MACH_RCV_TIMEOUT set)
    pub rcv_timeout: Option<Duration>,
    /// Notification port for async completion
    pub notify_port: PortName,
}

impl MachMsgOptions {
    /// Create options for send only
    pub fn send() -> Self {
        Self {
            flags: MACH_SEND_MSG,
            ..Default::default()
        }
    }

    /// Create options for receive only
    pub fn receive() -> Self {
        Self {
            flags: MACH_RCV_MSG,
            ..Default::default()
        }
    }

    /// Create options for send+receive (RPC)
    pub fn rpc() -> Self {
        Self {
            flags: MACH_SEND_MSG | MACH_RCV_MSG,
            ..Default::default()
        }
    }

    /// Add send timeout
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.flags |= MACH_SEND_TIMEOUT;
        self.send_timeout = Some(timeout);
        self
    }

    /// Add receive timeout
    pub fn with_rcv_timeout(mut self, timeout: Duration) -> Self {
        self.flags |= MACH_RCV_TIMEOUT;
        self.rcv_timeout = Some(timeout);
        self
    }

    /// Should send?
    pub fn should_send(&self) -> bool {
        (self.flags & MACH_SEND_MSG) != 0
    }

    /// Should receive?
    pub fn should_receive(&self) -> bool {
        (self.flags & MACH_RCV_MSG) != 0
    }

    /// Has send timeout?
    pub fn has_send_timeout(&self) -> bool {
        (self.flags & MACH_SEND_TIMEOUT) != 0
    }

    /// Has receive timeout?
    pub fn has_rcv_timeout(&self) -> bool {
        (self.flags & MACH_RCV_TIMEOUT) != 0
    }
}

// ============================================================================
// Message Return Type
// ============================================================================

/// Return value from mach_msg
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachMsgReturn(pub i32);

impl MachMsgReturn {
    /// Success
    pub const SUCCESS: Self = Self(MACH_MSG_SUCCESS);

    /// Check if successful
    pub fn is_success(self) -> bool {
        self.0 == MACH_MSG_SUCCESS
    }

    /// Check if send error
    pub fn is_send_error(self) -> bool {
        (self.0 & 0x10000000) != 0 && (self.0 & 0x10004000) == 0
    }

    /// Check if receive error
    pub fn is_rcv_error(self) -> bool {
        (self.0 & 0x10004000) != 0
    }

    /// Get error name for debugging
    pub fn name(self) -> &'static str {
        match self.0 {
            MACH_MSG_SUCCESS => "SUCCESS",
            MACH_SEND_INVALID_DATA => "SEND_INVALID_DATA",
            MACH_SEND_INVALID_DEST => "SEND_INVALID_DEST",
            MACH_SEND_TIMED_OUT => "SEND_TIMED_OUT",
            MACH_SEND_INTERRUPTED => "SEND_INTERRUPTED",
            MACH_SEND_MSG_TOO_SMALL => "SEND_MSG_TOO_SMALL",
            MACH_SEND_INVALID_REPLY => "SEND_INVALID_REPLY",
            MACH_SEND_INVALID_RIGHT => "SEND_INVALID_RIGHT",
            MACH_SEND_INVALID_HEADER => "SEND_INVALID_HEADER",
            MACH_RCV_INVALID_NAME => "RCV_INVALID_NAME",
            MACH_RCV_TIMED_OUT => "RCV_TIMED_OUT",
            MACH_RCV_TOO_LARGE => "RCV_TOO_LARGE",
            MACH_RCV_INTERRUPTED => "RCV_INTERRUPTED",
            MACH_RCV_PORT_DIED => "RCV_PORT_DIED",
            _ => "UNKNOWN",
        }
    }
}

impl Default for MachMsgReturn {
    fn default() -> Self {
        Self::SUCCESS
    }
}

impl From<i32> for MachMsgReturn {
    fn from(code: i32) -> Self {
        Self(code)
    }
}

// ============================================================================
// mach_msg Implementation
// ============================================================================

/// Statistics for mach_msg operations
#[derive(Debug, Clone, Default)]
pub struct MachMsgStats {
    /// Total send operations
    pub sends: u64,
    /// Total receive operations
    pub receives: u64,
    /// Combined send+receive (RPC)
    pub rpcs: u64,
    /// Send timeouts
    pub send_timeouts: u64,
    /// Receive timeouts
    pub rcv_timeouts: u64,
    /// Send errors
    pub send_errors: u64,
    /// Receive errors
    pub rcv_errors: u64,
}

static STATS: Mutex<MachMsgStats> = Mutex::new(MachMsgStats {
    sends: 0,
    receives: 0,
    rpcs: 0,
    send_timeouts: 0,
    rcv_timeouts: 0,
    send_errors: 0,
    rcv_errors: 0,
});

/// Get mach_msg statistics
pub fn stats() -> MachMsgStats {
    STATS.lock().clone()
}

/// The mach_msg system call
///
/// This is the main IPC primitive. It can:
/// - Send a message (MACH_SEND_MSG)
/// - Receive a message (MACH_RCV_MSG)
/// - Send then receive (both flags - RPC optimization)
///
/// # Arguments
///
/// * `header` - Message header (and buffer for receive)
/// * `options` - Operation options
/// * `send_size` - Size of message to send (if sending)
/// * `rcv_size` - Size of receive buffer (if receiving)
/// * `rcv_name` - Port to receive from (if receiving)
/// * `space` - The task's IPC space for port name resolution
///
/// # Returns
///
/// `MachMsgReturn` indicating success or error code.
pub fn mach_msg_with_space(
    header: &mut MachMsgHeader,
    options: MachMsgOptions,
    send_size: u32,
    rcv_size: u32,
    rcv_name: PortName,
    space: &IpcSpace,
) -> MachMsgReturn {
    let mut stats = STATS.lock();

    // Combined send+receive?
    if options.should_send() && options.should_receive() {
        stats.rpcs += 1;
    }

    // Send phase
    if options.should_send() {
        stats.sends += 1;

        let result = mach_msg_send(header, &options, send_size, space);
        if !result.is_success() {
            stats.send_errors += 1;
            if result.0 == MACH_SEND_TIMED_OUT {
                stats.send_timeouts += 1;
            }
            return result;
        }
    }

    // Receive phase
    if options.should_receive() {
        stats.receives += 1;

        let result = mach_msg_receive(header, &options, rcv_size, rcv_name, space);
        if !result.is_success() {
            stats.rcv_errors += 1;
            if result.0 == MACH_RCV_TIMED_OUT {
                stats.rcv_timeouts += 1;
            }
            return result;
        }
    }

    MachMsgReturn::SUCCESS
}

/// Legacy mach_msg that uses kernel space (for simple testing)
pub fn mach_msg(
    header: &mut MachMsgHeader,
    options: MachMsgOptions,
    send_size: u32,
    rcv_size: u32,
    rcv_name: PortName,
) -> MachMsgReturn {
    // Use kernel space by default for backward compatibility
    let space = crate::ipc::space::kernel_space();
    mach_msg_with_space(header, options, send_size, rcv_size, rcv_name, space)
}

/// Send a message (internal implementation)
///
/// This performs the actual send operation:
/// 1. Validate the message header
/// 2. Look up and copyin the destination port
/// 3. Look up and copyin the reply port (if any)
/// 4. Allocate a kernel message (kmsg)
/// 5. Copy the message body to the kmsg
/// 6. Enqueue the kmsg on the destination port's message queue
/// 7. Wake up any waiting receivers
fn mach_msg_send(
    header: &MachMsgHeader,
    options: &MachMsgOptions,
    send_size: u32,
    space: &IpcSpace,
) -> MachMsgReturn {
    // Validate header
    if !header.is_valid() {
        return MachMsgReturn(MACH_SEND_INVALID_HEADER);
    }

    // Check destination is specified
    if header.msgh_remote_port.is_null() {
        return MachMsgReturn(MACH_SEND_INVALID_DEST);
    }

    // Get the destination port type from header bits
    let remote_type_bits = header.msgh_bits.remote_type();
    let remote_msg_type = match msg_type_from_bits(remote_type_bits) {
        Some(t) => t,
        None => return MachMsgReturn(MACH_SEND_INVALID_RIGHT),
    };

    // Copyin the destination port
    let dest_result = match copyin(space, header.msgh_remote_port.0, remote_msg_type) {
        Ok(r) => r,
        Err(IpcError::InvalidPort) => return MachMsgReturn(MACH_SEND_INVALID_DEST),
        Err(IpcError::InvalidRight) => return MachMsgReturn(MACH_SEND_INVALID_RIGHT),
        Err(IpcError::PortDead) => return MachMsgReturn(MACH_SEND_INVALID_DEST),
        Err(_) => return MachMsgReturn(MACH_SEND_INVALID_DATA),
    };

    let dest_port = match dest_result.port {
        Some(p) => p,
        None => return MachMsgReturn(MACH_SEND_INVALID_DEST),
    };

    // Copyin the reply port (if any)
    let reply_port: Option<Arc<Mutex<Port>>> = if !header.msgh_local_port.is_null() {
        let local_type_bits = header.msgh_bits.local_type();
        let local_msg_type = match msg_type_from_bits(local_type_bits) {
            Some(t) => t,
            None => return MachMsgReturn(MACH_SEND_INVALID_REPLY),
        };

        match copyin(space, header.msgh_local_port.0, local_msg_type) {
            Ok(r) => r.port,
            Err(_) => return MachMsgReturn(MACH_SEND_INVALID_REPLY),
        }
    } else {
        None
    };

    // Allocate a kernel message
    let mut kmsg = kmsg_alloc(send_size);

    // Set up the kmsg header with actual port references
    kmsg.set_header(
        header.msgh_bits.0,
        send_size,
        Some(Arc::clone(&dest_port)),
        reply_port,
        header.msgh_id,
    );

    // Copy message body if there's data beyond the header
    let body_size = send_size as usize - MACH_MSG_SIZE_MIN;
    if body_size > 0 {
        // In a real implementation, we'd copy from user space here
        // For now, just resize the body
        kmsg.body_mut().resize(body_size, 0);
    }

    // If message is complex, process descriptors
    if header.msgh_bits.is_complex() {
        if let Err(_e) = process_complex_send(&mut kmsg, space) {
            return MachMsgReturn(MACH_SEND_INVALID_DATA);
        }
    }

    // Now enqueue the message on the destination port
    let enqueue_result = {
        let mut port_guard = dest_port.lock();

        // Check if port is alive
        if port_guard.is_dead() {
            return MachMsgReturn(MACH_SEND_INVALID_DEST);
        }

        // Check if port queue is full
        if port_guard.mqueue().is_full() {
            if options.has_send_timeout() {
                // Would need to block with timeout
                return MachMsgReturn(MACH_SEND_TIMED_OUT);
            }
            // Non-blocking send to full queue
            return MachMsgReturn(MACH_SEND_NO_BUFFER);
        }

        // Enqueue the message
        let result = port_guard.mqueue_mut().send(kmsg);

        // If successful, get the port address for wakeup before dropping guard
        let port_addr = if result.is_ok() {
            Some(&*dest_port as *const _ as u64)
        } else {
            None
        };

        (result, port_addr)
    };

    match enqueue_result {
        (Ok(()), Some(port_addr)) => {
            // Wake up one thread waiting for a message on this port
            crate::kern::sched_prim::thread_wakeup_one(port_addr);
            MachMsgReturn::SUCCESS
        }
        (Ok(()), None) => MachMsgReturn::SUCCESS,
        (Err((IpcError::PortDead, _kmsg)), _) => MachMsgReturn(MACH_SEND_INVALID_DEST),
        (Err((IpcError::NoSpace, _kmsg)), _) => MachMsgReturn(MACH_SEND_NO_BUFFER),
        (Err((_, _kmsg)), _) => MachMsgReturn(MACH_SEND_INVALID_DATA),
    }
}

/// Process complex message descriptors for send
///
/// For complex messages (MACH_MSGH_BITS_COMPLEX set), this parses the message body
/// to extract embedded port rights and OOL data references.
///
/// The message body must already contain the descriptor data. This function:
/// 1. Parses the message body for type descriptors
/// 2. For each port descriptor:
///    - Copies in the port right from the sender's space
///    - Stores the port reference in kmsg.port_rights
/// 3. For each OOL descriptor:
///    - Records the OOL data reference (actual copyin would happen in syscall layer)
///
/// Note: In a full implementation with user-space syscalls, the body data would
/// be copied in from user space before calling this function.
fn process_complex_send(kmsg: &mut IpcKmsg, space: &IpcSpace) -> Result<(), IpcError> {
    // Get the current body data (should have been populated by the caller)
    let body_data = kmsg.body().to_vec();

    // Re-parse the body with space context for port copyin
    // This leverages the comprehensive parsing in copyin_body
    kmsg.copyin_body(&body_data, space)
}

/// Convert message bits to MsgTypeName
fn msg_type_from_bits(bits: MsgType) -> Option<MsgTypeName> {
    match bits {
        MsgType::MoveReceive => Some(MsgTypeName::MoveReceive),
        MsgType::MoveSend => Some(MsgTypeName::MoveSend),
        MsgType::MoveSendOnce => Some(MsgTypeName::MoveSendOnce),
        MsgType::CopySend => Some(MsgTypeName::CopySend),
        MsgType::MakeSend => Some(MsgTypeName::MakeSend),
        MsgType::MakeSendOnce => Some(MsgTypeName::MakeSendOnce),
        MsgType::None => None,
    }
}

/// Receive a message (internal implementation)
///
/// This performs the actual receive operation:
/// 1. Look up the receive port in the task's space
/// 2. Verify we have receive rights
/// 3. Dequeue a message from the port's queue (or wait)
/// 4. Copy out port rights to the receiver's space
/// 5. Copy message to user buffer
/// 6. Deallocate the kernel message
fn mach_msg_receive(
    header: &mut MachMsgHeader,
    options: &MachMsgOptions,
    rcv_size: u32,
    rcv_name: PortName,
    space: &IpcSpace,
) -> MachMsgReturn {
    // Check receive port is specified
    if rcv_name.is_null() {
        return MachMsgReturn(MACH_RCV_INVALID_NAME);
    }

    // Look up the receive port in our space
    // We need receive rights to receive from a port
    let rcv_port = match space.get_port(rcv_name.0) {
        Ok(p) => p,
        Err(IpcError::InvalidPort) => return MachMsgReturn(MACH_RCV_INVALID_NAME),
        Err(IpcError::PortDead) => return MachMsgReturn(MACH_RCV_PORT_DIED),
        Err(_) => return MachMsgReturn(MACH_RCV_INVALID_NAME),
    };

    // Verify we have receive right (not just send right)
    {
        let entry = match space.lookup(rcv_name.0) {
            Ok(e) => e,
            Err(_) => return MachMsgReturn(MACH_RCV_INVALID_NAME),
        };

        if !entry.has_receive() {
            return MachMsgReturn(MACH_RCV_INVALID_NAME);
        }
    }

    // Try to dequeue a message
    let kmsg = loop {
        let mut port_guard = rcv_port.lock();

        // Check if port is alive
        if port_guard.is_dead() {
            return MachMsgReturn(MACH_RCV_PORT_DIED);
        }

        // Try to receive
        match port_guard.mqueue_mut().receive() {
            Ok(kmsg) => break kmsg,
            Err(IpcError::WouldBlock) => {
                // No message available - check if we should block
                // Check for zero timeout (non-blocking)
                if let Some(ref timeout) = options.rcv_timeout {
                    if timeout.is_zero() {
                        return MachMsgReturn(MACH_RCV_TIMED_OUT);
                    }
                }

                // Set up blocking on the port
                // Use the port address as the wait event
                let wait_event = &*rcv_port as *const _ as u64;
                let thread_id = crate::scheduler::current_thread()
                    .map(|t| t.thread_id)
                    .unwrap_or(crate::types::ThreadId(0));

                // Drop the port lock before blocking
                drop(port_guard);

                // Assert the wait on the port
                if let Some(ref timeout) = options.rcv_timeout {
                    // Block with timeout (convert Duration to ticks - assume 1 tick = 1ms)
                    let timeout_ticks = timeout.as_millis() as u64;
                    crate::kern::sched_prim::assert_wait_timeout(
                        thread_id,
                        wait_event,
                        true, // interruptible
                        timeout_ticks,
                    );
                } else {
                    // Block indefinitely
                    crate::kern::sched_prim::assert_wait(
                        thread_id,
                        wait_event,
                        true, // interruptible
                    );
                }

                // Actually block the thread
                let result = crate::kern::sched_prim::thread_block(
                    crate::kern::sched_prim::WaitReason::IpcReceive,
                );

                // Check the wait result
                match result {
                    crate::kern::sched_prim::WaitResult::Normal => {
                        // Woken normally, loop to try receiving again
                        continue;
                    }
                    crate::kern::sched_prim::WaitResult::TimedOut => {
                        return MachMsgReturn(MACH_RCV_TIMED_OUT);
                    }
                    crate::kern::sched_prim::WaitResult::Interrupted => {
                        return MachMsgReturn(MACH_RCV_INTERRUPTED);
                    }
                    _ => {
                        return MachMsgReturn(MACH_RCV_TIMED_OUT);
                    }
                }
            }
            Err(IpcError::PortDead) => {
                return MachMsgReturn(MACH_RCV_PORT_DIED);
            }
            Err(_) => {
                return MachMsgReturn(MACH_RCV_INVALID_DATA);
            }
        }
    };

    // Check if message fits in receive buffer
    if kmsg.msg_size() > rcv_size {
        if (options.flags & MACH_RCV_LARGE) != 0 {
            // Return the size needed
            header.msgh_size = kmsg.msg_size();
            // In real implementation, we'd re-queue the message
            return MachMsgReturn(MACH_RCV_TOO_LARGE);
        }
        return MachMsgReturn(MACH_RCV_TOO_LARGE);
    }

    // Copy out the message header
    header.msgh_bits = MachMsgBits(kmsg.header_bits());
    header.msgh_size = kmsg.msg_size();
    header.msgh_id = kmsg.msg_id();
    header.msgh_voucher_port = PortName::NULL;

    // Copy out the remote (destination) port - becomes local on receive
    if let Some(remote_port) = kmsg.remote_port() {
        let remote_type_bits = (kmsg.header_bits() >> 8) & 0x1f;
        let msg_type = msg_type_from_bits(MsgType::from_raw(remote_type_bits));

        if let Some(mtype) = msg_type {
            match copyout(space, Arc::clone(remote_port), mtype) {
                Ok(name) => header.msgh_remote_port = PortName(name),
                Err(_) => header.msgh_remote_port = PortName::NULL,
            }
        } else {
            header.msgh_remote_port = PortName::NULL;
        }
    } else {
        header.msgh_remote_port = PortName::NULL;
    }

    // Copy out the local (reply) port - this is the sender's reply port
    if let Some(local_port) = kmsg.local_port() {
        let local_type_bits = kmsg.header_bits() & 0x1f;
        let msg_type = msg_type_from_bits(MsgType::from_raw(local_type_bits));

        if let Some(mtype) = msg_type {
            match copyout(space, Arc::clone(local_port), mtype) {
                Ok(name) => header.msgh_local_port = PortName(name),
                Err(_) => header.msgh_local_port = PortName::NULL,
            }
        } else {
            header.msgh_local_port = PortName::NULL;
        }
    } else {
        header.msgh_local_port = PortName::NULL;
    }

    // If message is complex, process descriptors
    if header.msgh_bits.is_complex() {
        if let Err(_e) = process_complex_receive(&kmsg, space) {
            return MachMsgReturn(MACH_RCV_BODY_ERROR);
        }
    }

    // The kmsg is automatically dropped here, cleaning up port references

    MachMsgReturn::SUCCESS
}

/// Process complex message descriptors for receive
///
/// For complex messages, this converts kernel port references back to
/// user-visible port names in the receiver's space, and handles OOL data mapping.
///
/// This function:
/// 1. Iterates through kmsg.port_rights
/// 2. For each port right, copies out to receiver's space (creates entry if needed)
/// 3. Iterates through kmsg.ool_regions
/// 4. Maps or copies OOL data into receiver's address space
///
/// Note: The OOL data mapping is currently a placeholder - in a full implementation
/// with VM support (Phase 4), this would use vm_map_copyout.
fn process_complex_receive(kmsg: &IpcKmsg, space: &IpcSpace) -> Result<(), IpcError> {
    // Use the comprehensive copyout logic in IpcKmsg
    // This handles converting port references to names and OOL address mapping
    let _output_body = kmsg.copyout_body_complex(space)?;

    // The output body now has:
    // - Port descriptors with names valid in receiver's space
    // - OOL descriptors with addresses in receiver's address space (placeholder)

    // Note: The actual body update happens through the header copyout path
    // This function primarily ensures all port rights are properly transferred

    Ok(())
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Send a simple message
pub fn mach_msg_send_simple(dest: PortName, msg_id: i32, inline_data: &[u8]) -> MachMsgReturn {
    let size = (MACH_MSG_SIZE_MIN + inline_data.len()) as u32;

    let mut header = MachMsgHeader::simple(
        dest,
        PortName::NULL,
        MsgType::CopySend,
        MsgType::None,
        size,
        msg_id,
    );

    mach_msg(&mut header, MachMsgOptions::send(), size, 0, PortName::NULL)
}

/// Receive a simple message
pub fn mach_msg_receive_simple(
    rcv_port: PortName,
    buffer: &mut [u8],
) -> Result<(MachMsgHeader, usize), MachMsgReturn> {
    let rcv_size = buffer.len() as u32;

    let mut header = MachMsgHeader::default();

    let result = mach_msg(
        &mut header,
        MachMsgOptions::receive(),
        0,
        rcv_size,
        rcv_port,
    );

    if result.is_success() {
        let data_size = (header.msgh_size as usize).saturating_sub(MACH_MSG_SIZE_MIN);
        Ok((header, data_size))
    } else {
        Err(result)
    }
}

/// Perform an RPC (send request, receive reply)
pub fn mach_msg_rpc(
    dest: PortName,
    reply: PortName,
    msg_id: i32,
    request_data: &[u8],
    reply_buffer: &mut [u8],
) -> Result<(MachMsgHeader, usize), MachMsgReturn> {
    let send_size = (MACH_MSG_SIZE_MIN + request_data.len()) as u32;
    let rcv_size = reply_buffer.len() as u32;

    let mut header = MachMsgHeader::simple(
        dest,
        reply,
        MsgType::CopySend,
        MsgType::MakeSendOnce,
        send_size,
        msg_id,
    );

    let result = mach_msg(
        &mut header,
        MachMsgOptions::rpc(),
        send_size,
        rcv_size,
        reply,
    );

    if result.is_success() {
        let data_size = (header.msgh_size as usize).saturating_sub(MACH_MSG_SIZE_MIN);
        Ok((header, data_size))
    } else {
        Err(result)
    }
}

// ============================================================================
// Error Conversion
// ============================================================================

impl From<IpcError> for MachMsgReturn {
    fn from(err: IpcError) -> Self {
        match err {
            IpcError::InvalidPort => Self(MACH_SEND_INVALID_DEST),
            IpcError::PortDead => Self(MACH_RCV_PORT_DIED),
            IpcError::NoSpace => Self(MACH_SEND_NO_BUFFER),
            IpcError::InvalidRight => Self(MACH_SEND_INVALID_RIGHT),
            IpcError::WouldBlock => Self(MACH_SEND_TIMED_OUT),
            IpcError::MessageTooLarge => Self(MACH_RCV_TOO_LARGE),
            IpcError::NoMemory => Self(MACH_SEND_NO_BUFFER),
            IpcError::InvalidThread => Self(MACH_SEND_INVALID_DATA),
        }
    }
}

impl From<MachMsgReturn> for IpcResult<()> {
    fn from(ret: MachMsgReturn) -> Self {
        if ret.is_success() {
            Ok(())
        } else if ret.is_send_error() {
            Err(IpcError::InvalidPort)
        } else {
            // Receive error
            Err(IpcError::PortDead)
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the mach_msg subsystem
pub fn init() {
    // mach_msg is stateless except for statistics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_bits() {
        let bits = MachMsgBits::new(MsgType::CopySend, MsgType::MakeSendOnce);
        assert_eq!(bits.remote_type(), MsgType::CopySend);
        assert_eq!(bits.local_type(), MsgType::MakeSendOnce);
        assert!(!bits.is_complex());
    }

    #[test]
    fn test_msg_type() {
        assert!(MsgType::MoveSend.is_move());
        assert!(MsgType::CopySend.is_copy());
        assert!(MsgType::MakeSend.is_make());
        assert!(!MsgType::None.is_move());
    }

    #[test]
    fn test_header_validation() {
        let header = MachMsgHeader::simple(
            PortName::new(),
            PortName::NULL,
            MsgType::CopySend,
            MsgType::None,
            32,
            100,
        );
        assert!(header.is_valid());

        // Too small
        let mut bad = header;
        bad.msgh_size = 0;
        assert!(!bad.is_valid());
    }

    #[test]
    fn test_options() {
        let send = MachMsgOptions::send();
        assert!(send.should_send());
        assert!(!send.should_receive());

        let rcv = MachMsgOptions::receive();
        assert!(!rcv.should_send());
        assert!(rcv.should_receive());

        let rpc = MachMsgOptions::rpc();
        assert!(rpc.should_send());
        assert!(rpc.should_receive());
    }

    #[test]
    fn test_return_codes() {
        assert!(MachMsgReturn::SUCCESS.is_success());
        assert!(!MachMsgReturn(MACH_SEND_TIMED_OUT).is_success());
        assert!(MachMsgReturn(MACH_SEND_TIMED_OUT).is_send_error());
        assert!(MachMsgReturn(MACH_RCV_TIMED_OUT).is_rcv_error());
    }

    #[test]
    fn test_mach_msg_basic() {
        let mut header = MachMsgHeader::simple(
            PortName::new(),
            PortName::NULL,
            MsgType::CopySend,
            MsgType::None,
            32,
            100,
        );

        // Send only (will fail because no real port)
        // In real implementation this would work
        let _result = mach_msg(&mut header, MachMsgOptions::send(), 32, 0, PortName::NULL);
    }
}
