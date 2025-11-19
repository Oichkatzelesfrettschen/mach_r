//! Type-safe Rust runtime for Mach IPC
//!
//! This crate provides the runtime types and functions needed by code
//! generated from MIG .defs files.
//!
//! # Features
//!
//! - `async`: Enable async IPC support with Tokio
//! - `serde`: Enable serialization support
//!
//! # Example
//!
//! ```no_run
//! use mach_r::ipc::*;
//!
//! // Send a simple IPC message
//! let header = MachMsgHeader::new(1000, 64);
//! // ... send via mach_msg
//! ```

#![allow(non_camel_case_types)]

pub mod ipc;
pub mod error;

// Re-export commonly used items
pub use error::{IpcError, Result};
pub use ipc::{
    KernReturn, PortName, MachMsgHeader, MachMsgType,
    KERN_SUCCESS, MACH_PORT_NULL,
};
