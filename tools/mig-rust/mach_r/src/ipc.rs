//! Core Mach IPC types and constants
//!
//! This module provides the fundamental types used in Mach message passing:
//! - Port names and rights
//! - Message headers and type descriptors
//! - IPC constants and flags

use crate::error::Result;

#[cfg(feature = "async")]
use crate::error::IpcError;

#[cfg(feature = "async")]
pub use async_port::AsyncPort;

// ════════════════════════════════════════════════════════════
// Basic Types
// ════════════════════════════════════════════════════════════

/// A Mach port name (identifier)
///
/// Port names are process-local identifiers for ports. The same port
/// may have different names in different processes.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PortName(pub u32);

impl PortName {
    /// Create a new port name
    pub const fn new(name: u32) -> Self {
        PortName(name)
    }

    /// Check if this is a null port
    pub const fn is_null(&self) -> bool {
        self.0 == MACH_PORT_NULL.0
    }

    /// Get the raw port name value
    pub const fn as_raw(&self) -> u32 {
        self.0
    }
}

impl From<u32> for PortName {
    fn from(value: u32) -> Self {
        PortName(value)
    }
}

impl From<PortName> for u32 {
    fn from(port: PortName) -> Self {
        port.0
    }
}

/// Kernel return code
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KernReturn(pub i32);

impl KernReturn {
    /// Create a new kernel return code
    pub const fn new(code: i32) -> Self {
        KernReturn(code)
    }

    /// Check if this represents success
    pub const fn is_success(&self) -> bool {
        self.0 == KERN_SUCCESS.0
    }

    /// Convert to Result
    pub fn to_result(self) -> Result<()> {
        if self.is_success() {
            Ok(())
        } else {
            Err(crate::error::kern_to_error(self.0))
        }
    }

    /// Get the raw return code value
    pub const fn as_raw(&self) -> i32 {
        self.0
    }
}

impl From<i32> for KernReturn {
    fn from(value: i32) -> Self {
        KernReturn(value)
    }
}

impl From<KernReturn> for i32 {
    fn from(kr: KernReturn) -> Self {
        kr.0
    }
}

// ════════════════════════════════════════════════════════════
// Message Structures
// ════════════════════════════════════════════════════════════

/// Mach message header
///
/// Every Mach message starts with this header. It contains routing
/// information and message metadata.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MachMsgHeader {
    /// Message and port flags
    pub msgh_bits: u32,
    /// Message size in bytes
    pub msgh_size: u32,
    /// Remote (destination) port
    pub msgh_remote_port: PortName,
    /// Local (reply) port
    pub msgh_local_port: PortName,
    /// Voucher port (unused in most cases)
    pub msgh_voucher_port: PortName,
    /// Message ID
    pub msgh_id: u32,
}

impl MachMsgHeader {
    /// Create a new message header
    ///
    /// # Arguments
    ///
    /// * `msg_id` - Message identifier (subsystem-specific)
    /// * `msg_size` - Total message size including header
    pub fn new(msg_id: u32, msg_size: u32) -> Self {
        Self {
            msgh_bits: 0,
            msgh_size: msg_size,
            msgh_remote_port: MACH_PORT_NULL,
            msgh_local_port: MACH_PORT_NULL,
            msgh_voucher_port: MACH_PORT_NULL,
            msgh_id: msg_id,
        }
    }

    /// Set the remote (destination) port
    pub fn with_remote_port(mut self, port: PortName, disposition: u32) -> Self {
        self.msgh_remote_port = port;
        self.msgh_bits = (self.msgh_bits & !0xFF) | disposition;
        self
    }

    /// Set the local (reply) port
    pub fn with_local_port(mut self, port: PortName, disposition: u32) -> Self {
        self.msgh_local_port = port;
        self.msgh_bits = (self.msgh_bits & !0xFF00) | (disposition << 8);
        self
    }

    /// Set the message bits
    pub fn with_bits(mut self, bits: u32) -> Self {
        self.msgh_bits = bits;
        self
    }
}

/// Mach message type descriptor
///
/// Describes the type and size of message data. Used for both inline
/// data and out-of-line memory regions.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MachMsgType {
    /// Type name (MACH_MSG_TYPE_*)
    pub msgt_name: u8,
    /// Size of each element in bits
    pub msgt_size: u8,
    /// Number of elements
    pub msgt_number: u32,
    /// Flags (inline, longform, deallocate)
    pub msgt_inline: u8,
    /// Unused padding
    pub msgt_unused: u8,
}

impl MachMsgType {
    /// Create a new type descriptor for inline data
    ///
    /// # Arguments
    ///
    /// * `type_name` - Mach type constant (MACH_MSG_TYPE_*)
    /// * `size_bits` - Size of each element in bits
    /// * `number` - Number of elements
    pub fn new_inline(type_name: u8, size_bits: u8, number: u32) -> Self {
        Self {
            msgt_name: type_name,
            msgt_size: size_bits,
            msgt_number: number,
            msgt_inline: 1,
            msgt_unused: 0,
        }
    }

    /// Create a type descriptor for a 32-bit integer
    pub fn integer_32(count: u32) -> Self {
        Self::new_inline(MACH_MSG_TYPE_INTEGER_32, 32, count)
    }

    /// Create a type descriptor for a 64-bit integer
    pub fn integer_64(count: u32) -> Self {
        Self::new_inline(MACH_MSG_TYPE_INTEGER_64, 64, count)
    }

    /// Create a type descriptor for a port with COPY_SEND disposition
    pub fn port_copy_send() -> Self {
        Self::new_inline(MACH_MSG_TYPE_COPY_SEND, 32, 1)
    }

    /// Create a type descriptor for a port with MAKE_SEND_ONCE disposition
    pub fn port_make_send_once() -> Self {
        Self::new_inline(MACH_MSG_TYPE_MAKE_SEND_ONCE, 32, 1)
    }
}

// ════════════════════════════════════════════════════════════
// Constants - Kern Return Codes
// ════════════════════════════════════════════════════════════

/// Operation completed successfully
pub const KERN_SUCCESS: KernReturn = KernReturn(0);

/// Invalid address
pub const KERN_INVALID_ADDRESS: KernReturn = KernReturn(1);

/// Memory protection failure
pub const KERN_PROTECTION_FAILURE: KernReturn = KernReturn(2);

/// Out of memory
pub const KERN_NO_SPACE: KernReturn = KernReturn(3);

/// Invalid argument
pub const KERN_INVALID_ARGUMENT: KernReturn = KernReturn(4);

/// Operation failed
pub const KERN_FAILURE: KernReturn = KernReturn(5);

// ════════════════════════════════════════════════════════════
// Constants - Port Names
// ════════════════════════════════════════════════════════════

/// The null port (invalid port name)
pub const MACH_PORT_NULL: PortName = PortName(0);

/// Port representing the kernel
pub const MACH_PORT_DEAD: PortName = PortName(!0);

// ════════════════════════════════════════════════════════════
// Constants - Message Type Names
// ════════════════════════════════════════════════════════════

/// Untyped data
pub const MACH_MSG_TYPE_UNSTRUCTURED: u8 = 0;

/// Move receive right
pub const MACH_MSG_TYPE_MOVE_RECEIVE: u8 = 16;

/// Move send right
pub const MACH_MSG_TYPE_MOVE_SEND: u8 = 17;

/// Move send-once right
pub const MACH_MSG_TYPE_MOVE_SEND_ONCE: u8 = 18;

/// Copy send right
pub const MACH_MSG_TYPE_COPY_SEND: u8 = 19;

/// Make send right
pub const MACH_MSG_TYPE_MAKE_SEND: u8 = 20;

/// Make send-once right
pub const MACH_MSG_TYPE_MAKE_SEND_ONCE: u8 = 21;

/// Boolean value
pub const MACH_MSG_TYPE_BOOLEAN: u8 = 0;

/// 16-bit integer
pub const MACH_MSG_TYPE_INTEGER_16: u8 = 1;

/// 32-bit integer
pub const MACH_MSG_TYPE_INTEGER_32: u8 = 2;

/// 64-bit integer
pub const MACH_MSG_TYPE_INTEGER_64: u8 = 11;

/// Character (8-bit)
pub const MACH_MSG_TYPE_CHAR: u8 = 8;

/// Byte (8-bit)
pub const MACH_MSG_TYPE_BYTE: u8 = 9;

/// String (null-terminated)
pub const MACH_MSG_TYPE_STRING: u8 = 12;

// ════════════════════════════════════════════════════════════
// Constants - Message Header Bits
// ════════════════════════════════════════════════════════════

/// Mask for remote port disposition
pub const MACH_MSGH_BITS_REMOTE_MASK: u32 = 0x000000ff;

/// Mask for local port disposition
pub const MACH_MSGH_BITS_LOCAL_MASK: u32 = 0x0000ff00;

/// Create message header bits from port dispositions
///
/// # Arguments
///
/// * `remote` - Remote port disposition
/// * `local` - Local port disposition
#[allow(non_snake_case)]
pub const fn MACH_MSGH_BITS(remote: u32, local: u32) -> u32 {
    remote | (local << 8)
}

/// Extract remote port disposition from header bits
#[allow(non_snake_case)]
pub const fn MACH_MSGH_BITS_REMOTE(bits: u32) -> u32 {
    bits & MACH_MSGH_BITS_REMOTE_MASK
}

/// Extract local port disposition from header bits
#[allow(non_snake_case)]
pub const fn MACH_MSGH_BITS_LOCAL(bits: u32) -> u32 {
    (bits & MACH_MSGH_BITS_LOCAL_MASK) >> 8
}

// ════════════════════════════════════════════════════════════
// Constants - Message Options
// ════════════════════════════════════════════════════════════

/// Send message (mach_msg option)
pub const MACH_SEND_MSG: u32 = 0x00000001;

/// Receive message (mach_msg option)
pub const MACH_RCV_MSG: u32 = 0x00000002;

/// Timeout on send
pub const MACH_SEND_TIMEOUT: u32 = 0x00000010;

/// Timeout on receive
pub const MACH_RCV_TIMEOUT: u32 = 0x00000020;

/// Never timeout
pub const MACH_MSG_TIMEOUT_NONE: u32 = 0;

// ════════════════════════════════════════════════════════════
// FFI Bridge (platform-specific)
// ════════════════════════════════════════════════════════════

#[cfg(target_os = "macos")]
extern "C" {
    /// Low-level mach_msg system call
    ///
    /// # Safety
    ///
    /// This function performs raw IPC and requires properly formatted
    /// message buffers. Misuse can lead to undefined behavior.
    pub fn mach_msg(
        msg: *mut MachMsgHeader,
        option: u32,
        send_size: u32,
        rcv_size: u32,
        rcv_name: u32,
        timeout: u32,
        notify: u32,
    ) -> i32;
}

#[cfg(not(target_os = "macos"))]
/// Stub implementation for non-macOS platforms
///
/// # Safety
///
/// This is a stub that always returns KERN_FAILURE.
pub unsafe fn mach_msg(
    _msg: *mut MachMsgHeader,
    _option: u32,
    _send_size: u32,
    _rcv_size: u32,
    _rcv_name: u32,
    _timeout: u32,
    _notify: u32,
) -> i32 {
    KERN_FAILURE.0
}

// ════════════════════════════════════════════════════════════
// Helper Functions
// ════════════════════════════════════════════════════════════

/// Send a message with timeout
///
/// # Safety
///
/// The message buffer must be properly formatted and valid for the
/// duration of the send operation.
pub unsafe fn send_msg(
    msg: *mut MachMsgHeader,
    size: u32,
    timeout_ms: u32,
) -> Result<()> {
    let kr = mach_msg(
        msg,
        MACH_SEND_MSG | MACH_SEND_TIMEOUT,
        size,
        0,
        MACH_PORT_NULL.0,
        timeout_ms,
        MACH_PORT_NULL.0,
    );

    KernReturn(kr).to_result()
}

/// Receive a message with timeout
///
/// # Safety
///
/// The message buffer must be large enough to hold the received message.
pub unsafe fn recv_msg(
    msg: *mut MachMsgHeader,
    max_size: u32,
    rcv_port: PortName,
    timeout_ms: u32,
) -> Result<()> {
    let kr = mach_msg(
        msg,
        MACH_RCV_MSG | MACH_RCV_TIMEOUT,
        0,
        max_size,
        rcv_port.0,
        timeout_ms,
        MACH_PORT_NULL.0,
    );

    KernReturn(kr).to_result()
}

/// Send a message and wait for reply
///
/// # Safety
///
/// Both the send and receive buffers must be properly formatted.
pub unsafe fn send_recv_msg(
    msg: *mut MachMsgHeader,
    send_size: u32,
    rcv_size: u32,
    rcv_port: PortName,
    timeout_ms: u32,
) -> Result<()> {
    let kr = mach_msg(
        msg,
        MACH_SEND_MSG | MACH_RCV_MSG | MACH_SEND_TIMEOUT | MACH_RCV_TIMEOUT,
        send_size,
        rcv_size,
        rcv_port.0,
        timeout_ms,
        MACH_PORT_NULL.0,
    );

    KernReturn(kr).to_result()
}

// ════════════════════════════════════════════════════════════
// Async Support (optional)
// ════════════════════════════════════════════════════════════

#[cfg(feature = "async")]
mod async_port {
    use super::*;
    use tokio::sync::mpsc;

    /// Async wrapper around a Mach port
    ///
    /// Provides async send/receive operations using Tokio channels.
    pub struct AsyncPort {
        port: PortName,
        tx: mpsc::UnboundedSender<Vec<u8>>,
        rx: mpsc::UnboundedReceiver<Vec<u8>>,
    }

    impl AsyncPort {
        /// Create a new async port
        pub fn new(port: PortName) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            Self { port, tx, rx }
        }

        /// Get the underlying port name
        pub fn port_name(&self) -> PortName {
            self.port
        }

        /// Send a message asynchronously
        pub async fn send(&self, data: Vec<u8>) -> Result<()> {
            self.tx
                .send(data)
                .map_err(|_| IpcError::PortDeallocated)?;
            Ok(())
        }

        /// Receive a message asynchronously
        pub async fn recv(&mut self) -> Result<Vec<u8>> {
            self.rx
                .recv()
                .await
                .ok_or(IpcError::PortDeallocated)
        }
    }
}

// ════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_name() {
        let port = PortName::new(42);
        assert_eq!(port.as_raw(), 42);
        assert!(!port.is_null());

        let null_port = MACH_PORT_NULL;
        assert!(null_port.is_null());
    }

    #[test]
    fn test_kern_return() {
        let kr = KERN_SUCCESS;
        assert!(kr.is_success());
        assert!(kr.to_result().is_ok());

        let kr_fail = KERN_FAILURE;
        assert!(!kr_fail.is_success());
        assert!(kr_fail.to_result().is_err());
    }

    #[test]
    fn test_message_header() {
        let header = MachMsgHeader::new(1000, 64);
        assert_eq!(header.msgh_id, 1000);
        assert_eq!(header.msgh_size, 64);
        assert!(header.msgh_remote_port.is_null());
    }

    #[test]
    fn test_message_type() {
        let ty = MachMsgType::integer_32(1);
        assert_eq!(ty.msgt_name, MACH_MSG_TYPE_INTEGER_32);
        assert_eq!(ty.msgt_size, 32);
        assert_eq!(ty.msgt_number, 1);
        assert_eq!(ty.msgt_inline, 1);
    }

    #[test]
    fn test_msgh_bits() {
        let bits = MACH_MSGH_BITS(
            MACH_MSG_TYPE_COPY_SEND as u32,
            MACH_MSG_TYPE_MAKE_SEND_ONCE as u32,
        );

        assert_eq!(
            MACH_MSGH_BITS_REMOTE(bits),
            MACH_MSG_TYPE_COPY_SEND as u32
        );
        assert_eq!(
            MACH_MSGH_BITS_LOCAL(bits),
            MACH_MSG_TYPE_MAKE_SEND_ONCE as u32
        );
    }
}
