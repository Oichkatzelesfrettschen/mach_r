//! Device Configuration - Operations Tables
//!
//! Based on Mach4 device/conf.h/c
//! Defines device operations and configuration.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::device::dev_hdr::{DeviceResult, MachDevice};
use crate::device::io_req::IoRequest;

// ============================================================================
// Device Operation Function Types
// ============================================================================

/// Device open function
pub type DevOpenFn = fn(&MachDevice) -> DeviceResult;

/// Device close function
pub type DevCloseFn = fn(&MachDevice) -> DeviceResult;

/// Device read function
pub type DevReadFn = fn(&MachDevice, &IoRequest) -> DeviceResult;

/// Device write function
pub type DevWriteFn = fn(&MachDevice, &IoRequest) -> DeviceResult;

/// Device get status function
pub type DevGetStatFn = fn(&MachDevice, u32, &mut [u32]) -> DeviceResult;

/// Device set status function
pub type DevSetStatFn = fn(&MachDevice, u32, &[u32]) -> DeviceResult;

/// Device memory map function (returns offset or error)
pub type DevMmapFn = fn(&MachDevice, u64, u32) -> Result<u64, DeviceResult>;

/// Device async input setup function
pub type DevAsyncInFn = fn(&MachDevice) -> DeviceResult;

/// Device reset function
pub type DevResetFn = fn(&MachDevice) -> DeviceResult;

/// Device port death function
pub type DevPortDeathFn = fn(&MachDevice, u32) -> DeviceResult;

/// Device info function
pub type DevInfoFn = fn(&MachDevice, u32) -> Result<u32, DeviceResult>;

// ============================================================================
// Device Operations
// ============================================================================

/// Device operations structure
///
/// Based on struct dev_ops from Mach4.
/// Contains pointers to device driver functions.
#[derive(Clone)]
pub struct DevOps {
    /// Device name
    pub name: String,

    /// Open device
    pub open: Option<DevOpenFn>,

    /// Close device
    pub close: Option<DevCloseFn>,

    /// Read from device
    pub read: Option<DevReadFn>,

    /// Write to device
    pub write: Option<DevWriteFn>,

    /// Get status/control
    pub getstat: Option<DevGetStatFn>,

    /// Set status/control
    pub setstat: Option<DevSetStatFn>,

    /// Map memory
    pub mmap: Option<DevMmapFn>,

    /// Asynchronous input setup
    pub async_in: Option<DevAsyncInFn>,

    /// Reset device
    pub reset: Option<DevResetFn>,

    /// Clean up reply ports
    pub port_death: Option<DevPortDeathFn>,

    /// Number of sub-devices per unit
    pub subdev: u32,

    /// Driver info for kernel
    pub dev_info: Option<DevInfoFn>,
}

impl DevOps {
    /// Create new device operations with name only
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            open: None,
            close: None,
            read: None,
            write: None,
            getstat: None,
            setstat: None,
            mmap: None,
            async_in: None,
            reset: None,
            port_death: None,
            subdev: 1,
            dev_info: None,
        }
    }

    /// Set open function
    pub fn with_open(mut self, f: DevOpenFn) -> Self {
        self.open = Some(f);
        self
    }

    /// Set close function
    pub fn with_close(mut self, f: DevCloseFn) -> Self {
        self.close = Some(f);
        self
    }

    /// Set read function
    pub fn with_read(mut self, f: DevReadFn) -> Self {
        self.read = Some(f);
        self
    }

    /// Set write function
    pub fn with_write(mut self, f: DevWriteFn) -> Self {
        self.write = Some(f);
        self
    }

    /// Set getstat function
    pub fn with_getstat(mut self, f: DevGetStatFn) -> Self {
        self.getstat = Some(f);
        self
    }

    /// Set setstat function
    pub fn with_setstat(mut self, f: DevSetStatFn) -> Self {
        self.setstat = Some(f);
        self
    }

    /// Set mmap function
    pub fn with_mmap(mut self, f: DevMmapFn) -> Self {
        self.mmap = Some(f);
        self
    }

    /// Set number of subdevices
    pub fn with_subdev(mut self, n: u32) -> Self {
        self.subdev = n;
        self
    }
}

impl core::fmt::Debug for DevOps {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DevOps")
            .field("name", &self.name)
            .field("subdev", &self.subdev)
            .field("has_open", &self.open.is_some())
            .field("has_close", &self.close.is_some())
            .field("has_read", &self.read.is_some())
            .field("has_write", &self.write.is_some())
            .finish()
    }
}

// ============================================================================
// Null Device Operations
// ============================================================================

/// Null device - does nothing, always succeeds
pub fn nulldev(_dev: &MachDevice) -> DeviceResult {
    DeviceResult::Success
}

/// No device - does nothing, returns error
pub fn nodev(_dev: &MachDevice) -> DeviceResult {
    DeviceResult::NotSupported
}

/// No map - returns error for mmap
pub fn nomap(_dev: &MachDevice, _off: u64, _prot: u32) -> Result<u64, DeviceResult> {
    Err(DeviceResult::NotSupported)
}

// ============================================================================
// Device Indirect
// ============================================================================

/// Device indirection entry
///
/// Maps a device name to actual operations and unit.
#[derive(Debug, Clone)]
pub struct DevIndirect {
    /// Device name
    pub name: String,
    /// Operations (major device)
    pub ops: Arc<DevOps>,
    /// Unit number
    pub unit: u32,
}

impl DevIndirect {
    pub fn new(name: &str, ops: Arc<DevOps>, unit: u32) -> Self {
        Self {
            name: String::from(name),
            ops,
            unit,
        }
    }
}

// ============================================================================
// Device Configuration Manager
// ============================================================================

/// Device configuration manager
pub struct DevConfig {
    /// Device operations list
    dev_ops_list: Vec<Arc<DevOps>>,
    /// Indirect device list
    indirect_list: Vec<DevIndirect>,
}

impl DevConfig {
    pub fn new() -> Self {
        Self {
            dev_ops_list: Vec::new(),
            indirect_list: Vec::new(),
        }
    }

    /// Register device operations
    pub fn register_ops(&mut self, ops: DevOps) -> Arc<DevOps> {
        let arc_ops = Arc::new(ops);
        self.dev_ops_list.push(Arc::clone(&arc_ops));
        arc_ops
    }

    /// Find device operations by name
    pub fn find_ops(&self, name: &str) -> Option<Arc<DevOps>> {
        self.dev_ops_list
            .iter()
            .find(|ops| ops.name == name)
            .cloned()
    }

    /// Register indirect device
    pub fn set_indirect(&mut self, name: &str, ops: Arc<DevOps>, unit: u32) {
        // Remove existing entry
        self.indirect_list.retain(|di| di.name != name);
        self.indirect_list.push(DevIndirect::new(name, ops, unit));
    }

    /// Find indirect device by name
    pub fn find_indirect(&self, name: &str) -> Option<&DevIndirect> {
        self.indirect_list.iter().find(|di| di.name == name)
    }

    /// List all registered device operations
    pub fn list_ops(&self) -> Vec<String> {
        self.dev_ops_list
            .iter()
            .map(|ops| ops.name.clone())
            .collect()
    }

    /// Search for device (direct or indirect)
    pub fn search(&self, name: &str) -> Option<(Arc<DevOps>, u32)> {
        // Check indirect first
        if let Some(indirect) = self.find_indirect(name) {
            return Some((Arc::clone(&indirect.ops), indirect.unit));
        }

        // Check direct ops
        if let Some(ops) = self.find_ops(name) {
            return Some((ops, 0));
        }

        None
    }
}

impl Default for DevConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Device Info Flavors
// ============================================================================

/// Device info flavor constants
pub mod dev_info {
    /// Block size
    pub const BLOCK_SIZE: u32 = 1;
    /// Device size
    pub const DEVICE_SIZE: u32 = 2;
    /// Device type
    pub const DEVICE_TYPE: u32 = 3;
}

// ============================================================================
// Global State
// ============================================================================

static DEV_CONFIG: spin::Once<Mutex<DevConfig>> = spin::Once::new();

/// Initialize device configuration
pub fn init_config() {
    DEV_CONFIG.call_once(|| Mutex::new(DevConfig::new()));
}

/// Get device configuration
pub fn config() -> &'static Mutex<DevConfig> {
    if DEV_CONFIG.get().is_none() {
        init_config();
    }
    DEV_CONFIG.get().unwrap()
}

/// Register device operations
pub fn register_ops(ops: DevOps) -> Arc<DevOps> {
    config().lock().register_ops(ops)
}

/// Find device operations
pub fn find_ops(name: &str) -> Option<Arc<DevOps>> {
    config().lock().find_ops(name)
}

/// Set indirect device
pub fn dev_set_indirect(name: &str, ops: Arc<DevOps>, unit: u32) {
    config().lock().set_indirect(name, ops, unit);
}

/// Search for device
pub fn dev_search(name: &str) -> Option<(Arc<DevOps>, u32)> {
    config().lock().search(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_ops() {
        let ops = DevOps::new("test").with_subdev(4);

        assert_eq!(ops.name, "test");
        assert_eq!(ops.subdev, 4);
        assert!(ops.open.is_none());
    }

    #[test]
    fn test_nulldev() {
        let dev = MachDevice::new(crate::device::dev_hdr::DeviceId(1), "null".into(), 0);
        assert_eq!(nulldev(&dev), DeviceResult::Success);
    }

    #[test]
    fn test_dev_config() {
        let mut cfg = DevConfig::new();
        let ops = cfg.register_ops(DevOps::new("disk"));

        assert!(cfg.find_ops("disk").is_some());
        assert!(cfg.find_ops("nonexistent").is_none());
    }
}
