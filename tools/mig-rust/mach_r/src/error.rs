//! Error types for Mach IPC operations

use thiserror::Error;

/// Result type for IPC operations
pub type Result<T> = std::result::Result<T, IpcError>;

/// Errors that can occur during IPC operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum IpcError {
    /// Invalid port name
    #[error("invalid port: {0}")]
    InvalidPort(u32),

    /// Message too large for buffer
    #[error("message too large: {size} bytes (max: {max})")]
    MessageTooLarge { size: usize, max: usize },

    /// Array exceeds maximum size
    #[error("array too large: {actual} elements (max: {max})")]
    ArrayTooLarge { actual: usize, max: usize },

    /// Invalid message format
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    /// Kernel return code error
    #[error("kernel error: {0} ({1})")]
    KernelError(i32, &'static str),

    /// Send timeout
    #[error("send timeout after {0}ms")]
    SendTimeout(u32),

    /// Receive timeout
    #[error("receive timeout after {0}ms")]
    ReceiveTimeout(u32),

    /// No reply received
    #[error("no reply from server")]
    NoReply,

    /// Port deallocated
    #[error("port was deallocated")]
    PortDeallocated,

    /// Invalid right type
    #[error("invalid port right: expected {expected}, got {actual}")]
    InvalidRight { expected: String, actual: String },

    /// Remote died
    #[error("remote process died")]
    RemoteDied,

    /// Out of memory
    #[error("out of memory")]
    OutOfMemory,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for IpcError {
    fn from(err: std::io::Error) -> Self {
        IpcError::Io(err.to_string())
    }
}

/// Convert kern_return_t to IpcError
pub fn kern_to_error(kr: i32) -> IpcError {
    match kr {
        0 => panic!("KERN_SUCCESS is not an error"),
        1 => IpcError::KernelError(kr, "KERN_INVALID_ADDRESS"),
        2 => IpcError::KernelError(kr, "KERN_PROTECTION_FAILURE"),
        3 => IpcError::OutOfMemory,
        4 => IpcError::KernelError(kr, "KERN_INVALID_ARGUMENT"),
        5 => IpcError::KernelError(kr, "KERN_FAILURE"),
        15 => IpcError::InvalidPort(0),
        46 => IpcError::SendTimeout(0),
        47 => IpcError::ReceiveTimeout(0),
        48 => IpcError::KernelError(kr, "MACH_SEND_INTERRUPTED"),
        49 => IpcError::MessageTooLarge { size: 0, max: 0 },
        _ => IpcError::KernelError(kr, "UNKNOWN_ERROR"),
    }
}
