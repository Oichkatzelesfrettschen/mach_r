//! Mach-style Inter-Process Communication
//! 
//! This is the core of the microkernel - all communication happens through ports

use core::sync::atomic::{AtomicU32, Ordering};

pub mod port;
pub mod message;
pub mod rights;



/// Global port name counter
static NEXT_PORT_NAME: AtomicU32 = AtomicU32::new(1000);

/// Port name type (like Mach's mach_port_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortName(u32);

impl PortName {
    pub const NULL: Self = Self(0);
    
    pub fn new() -> Self {
        let name = NEXT_PORT_NAME.fetch_add(1, Ordering::SeqCst);
        Self(name)
    }
    
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Get the underlying ID of the port name.
    pub fn id(&self) -> u32 {
        self.0
    }
}

/// IPC error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    InvalidPort,
    PortDead,
    NoSpace,
    InvalidRight,
    WouldBlock,
    MessageTooLarge,
    NoMemory,
    InvalidThread,
}

/// Result type for IPC operations
pub type IpcResult<T> = Result<T, IpcError>;

/// Initialize the IPC subsystem
pub fn init() {
    port::init();
}