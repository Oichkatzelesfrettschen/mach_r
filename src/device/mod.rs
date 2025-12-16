//! Mach Device Subsystem
//!
//! Based on Mach4 device/ subsystem.
//! Provides device management including:
//! - Device headers and state management
//! - I/O request handling
//! - Device operations table
//! - Device pager interface
//!
//! This is the kernel's interface to device drivers.

pub mod conf;
pub mod dev_hdr;
pub mod dev_pager;
pub mod ds_routines;
pub mod io_req;

pub use conf::{DevIndirect, DevOps};
pub use dev_hdr::{DeviceId, DeviceState, MachDevice};
pub use io_req::{IoMode, IoOp, IoRequest};

/// Initialize the device subsystem
pub fn init() {
    dev_hdr::init();
    io_req::init();
    ds_routines::init();
}
