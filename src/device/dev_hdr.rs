//! Device Header - Generic Device Abstraction
//!
//! Based on Mach4 device/dev_hdr.h/c
//! Provides the core device structure and management.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use crate::device::conf::DevOps;
use crate::ipc::PortName;

// ============================================================================
// Device ID
// ============================================================================

/// Device identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub u32);

impl DeviceId {
    pub const NULL: Self = Self(0);
}

// ============================================================================
// Device State
// ============================================================================

/// Device state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u16)]
pub enum DeviceState {
    /// Not open
    #[default]
    Init = 0,
    /// Being opened
    Opening = 1,
    /// Open and ready
    Open = 2,
    /// Being closed
    Closing = 3,
}

// ============================================================================
// Device Flags
// ============================================================================

/// Device flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceFlags(u16);

impl DeviceFlags {
    /// No flags
    pub const NONE: Self = Self(0);
    /// Open only once (exclusive)
    pub const EXCL_OPEN: Self = Self(0x0001);
    /// Block device
    pub const BLOCK: Self = Self(0x0002);
    /// Character device
    pub const CHAR: Self = Self(0x0004);
    /// Network device
    pub const NET: Self = Self(0x0008);
    /// Memory mapped device
    pub const MMAP: Self = Self(0x0010);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(&self) -> u16 {
        self.0
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Default for DeviceFlags {
    fn default() -> Self {
        Self::NONE
    }
}

// ============================================================================
// Mach Device Structure
// ============================================================================

/// Mach Device - generic device header
///
/// Based on struct mach_device from Mach4.
/// May be allocated with the device, or built when opened.
#[derive(Debug)]
pub struct MachDevice {
    /// Device identifier
    pub id: DeviceId,

    /// Reference count
    ref_count: AtomicU32,

    /// Device state
    pub state: Mutex<DeviceState>,

    /// Device flags
    pub flags: Mutex<DeviceFlags>,

    /// Number of times open
    pub open_count: AtomicU32,

    /// Number of IOs in progress
    pub io_in_progress: AtomicU32,

    /// Someone waiting for IO to finish
    pub io_wait: AtomicBool,

    /// Associated port for device operations
    pub port: Mutex<Option<PortName>>,

    /// Device number (major/minor encoded)
    pub dev_number: u32,

    /// Block size (for block devices)
    pub bsize: u32,

    /// Device operations
    pub dev_ops: Mutex<Option<Arc<DevOps>>>,

    /// Device name
    pub name: String,

    /// Unit number (minor device)
    pub unit: u32,

    /// Private driver data
    pub driver_data: Mutex<Option<u64>>,
}

impl MachDevice {
    /// Create a new device
    pub fn new(id: DeviceId, name: String, dev_number: u32) -> Self {
        Self {
            id,
            ref_count: AtomicU32::new(1),
            state: Mutex::new(DeviceState::Init),
            flags: Mutex::new(DeviceFlags::NONE),
            open_count: AtomicU32::new(0),
            io_in_progress: AtomicU32::new(0),
            io_wait: AtomicBool::new(false),
            port: Mutex::new(None),
            dev_number,
            bsize: 512, // Default block size
            dev_ops: Mutex::new(None),
            name,
            unit: dev_number & 0xFF, // Lower 8 bits = minor
            driver_data: Mutex::new(None),
        }
    }

    /// Create a block device
    pub fn block(id: DeviceId, name: String, dev_number: u32, bsize: u32) -> Self {
        let mut dev = Self::new(id, name, dev_number);
        *dev.flags.lock() = DeviceFlags::BLOCK;
        dev.bsize = bsize;
        dev
    }

    /// Create a character device
    pub fn character(id: DeviceId, name: String, dev_number: u32) -> Self {
        let dev = Self::new(id, name, dev_number);
        *dev.flags.lock() = DeviceFlags::CHAR;
        dev
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if device should be freed
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Check if device is open
    pub fn is_open(&self) -> bool {
        *self.state.lock() == DeviceState::Open
    }

    /// Get open count
    pub fn open_count(&self) -> u32 {
        self.open_count.load(Ordering::SeqCst)
    }

    /// Set device operations
    pub fn set_ops(&self, ops: Arc<DevOps>) {
        *self.dev_ops.lock() = Some(ops);
    }

    /// Get device operations
    pub fn get_ops(&self) -> Option<Arc<DevOps>> {
        self.dev_ops.lock().clone()
    }

    /// Begin an I/O operation
    pub fn io_start(&self) {
        self.io_in_progress.fetch_add(1, Ordering::SeqCst);
    }

    /// End an I/O operation
    pub fn io_done(&self) {
        let prev = self.io_in_progress.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 && self.io_wait.load(Ordering::SeqCst) {
            // Wake waiters
            self.io_wait.store(false, Ordering::SeqCst);
            // Would wake blocked threads here
        }
    }

    /// Wait for all I/O to complete
    pub fn io_wait_complete(&self) {
        while self.io_in_progress.load(Ordering::SeqCst) > 0 {
            self.io_wait.store(true, Ordering::SeqCst);
            // Would block here
        }
    }

    /// Get major device number
    pub fn major(&self) -> u32 {
        (self.dev_number >> 8) & 0xFF
    }

    /// Get minor device number
    pub fn minor(&self) -> u32 {
        self.dev_number & 0xFF
    }

    /// Set device port
    pub fn set_port(&self, port: PortName) {
        *self.port.lock() = Some(port);
    }

    /// Get device port
    pub fn get_port(&self) -> Option<PortName> {
        *self.port.lock()
    }
}

// ============================================================================
// Device Open/Close
// ============================================================================

/// Result of device operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceResult {
    Success,
    DeviceNotFound,
    DeviceBusy,
    InvalidArgument,
    NoMemory,
    IoError,
    NotSupported,
    AlreadyOpen,
    NotOpen,
}

/// Open a device
pub fn device_open(device: &MachDevice) -> DeviceResult {
    let mut state = device.state.lock();

    match *state {
        DeviceState::Init => {
            // Check for exclusive open
            if device.flags.lock().contains(DeviceFlags::EXCL_OPEN) && device.open_count() > 0 {
                return DeviceResult::DeviceBusy;
            }

            *state = DeviceState::Opening;
            drop(state);

            // Call driver open if available
            if let Some(ops) = device.get_ops() {
                if let Some(open_fn) = &ops.open {
                    let result = open_fn(device);
                    if result != DeviceResult::Success {
                        *device.state.lock() = DeviceState::Init;
                        return result;
                    }
                }
            }

            *device.state.lock() = DeviceState::Open;
            device.open_count.fetch_add(1, Ordering::SeqCst);
            DeviceResult::Success
        }
        DeviceState::Open => {
            // Already open - increment count
            if device.flags.lock().contains(DeviceFlags::EXCL_OPEN) {
                DeviceResult::DeviceBusy
            } else {
                device.open_count.fetch_add(1, Ordering::SeqCst);
                DeviceResult::Success
            }
        }
        _ => DeviceResult::DeviceBusy,
    }
}

/// Close a device
pub fn device_close(device: &MachDevice) -> DeviceResult {
    let count = device.open_count.fetch_sub(1, Ordering::SeqCst);

    if count == 1 {
        // Last close
        let mut state = device.state.lock();
        *state = DeviceState::Closing;
        drop(state);

        // Wait for IO to complete
        device.io_wait_complete();

        // Call driver close if available
        if let Some(ops) = device.get_ops() {
            if let Some(close_fn) = &ops.close {
                let _ = close_fn(device);
            }
        }

        *device.state.lock() = DeviceState::Init;
    }

    DeviceResult::Success
}

// ============================================================================
// Device Registry
// ============================================================================

/// Device registry
pub struct DeviceRegistry {
    /// Devices by ID
    devices: BTreeMap<DeviceId, Arc<MachDevice>>,
    /// Devices by name
    by_name: BTreeMap<String, DeviceId>,
    /// Devices by port
    by_port: BTreeMap<PortName, DeviceId>,
    /// Next device ID
    next_id: u32,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            by_name: BTreeMap::new(),
            by_port: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Register a new device
    pub fn register(&mut self, name: String, dev_number: u32) -> Arc<MachDevice> {
        let id = DeviceId(self.next_id);
        self.next_id += 1;

        let device = Arc::new(MachDevice::new(id, name.clone(), dev_number));
        self.devices.insert(id, Arc::clone(&device));
        self.by_name.insert(name, id);
        device
    }

    /// Register a block device
    pub fn register_block(&mut self, name: String, dev_number: u32, bsize: u32) -> Arc<MachDevice> {
        let id = DeviceId(self.next_id);
        self.next_id += 1;

        let device = Arc::new(MachDevice::block(id, name.clone(), dev_number, bsize));
        self.devices.insert(id, Arc::clone(&device));
        self.by_name.insert(name, id);
        device
    }

    /// Lookup device by name
    pub fn lookup(&self, name: &str) -> Option<Arc<MachDevice>> {
        let id = self.by_name.get(name)?;
        self.devices.get(id).cloned()
    }

    /// Lookup device by ID
    pub fn lookup_by_id(&self, id: DeviceId) -> Option<Arc<MachDevice>> {
        self.devices.get(&id).cloned()
    }

    /// Lookup device by port
    pub fn lookup_by_port(&self, port: PortName) -> Option<Arc<MachDevice>> {
        let id = self.by_port.get(&port)?;
        self.devices.get(id).cloned()
    }

    /// Associate a port with a device
    pub fn port_enter(&mut self, port: PortName, device_id: DeviceId) {
        self.by_port.insert(port, device_id);
        if let Some(device) = self.devices.get(&device_id) {
            device.set_port(port);
        }
    }

    /// Remove port association
    pub fn port_remove(&mut self, port: PortName) {
        self.by_port.remove(&port);
    }

    /// List all devices
    pub fn list(&self) -> Vec<Arc<MachDevice>> {
        self.devices.values().cloned().collect()
    }

    /// Iterate over all devices
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&MachDevice),
    {
        for device in self.devices.values() {
            f(device);
        }
    }
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static DEVICE_REGISTRY: spin::Once<Mutex<DeviceRegistry>> = spin::Once::new();

/// Initialize device subsystem
pub fn init() {
    DEVICE_REGISTRY.call_once(|| Mutex::new(DeviceRegistry::new()));
}

/// Get device registry
pub fn registry() -> &'static Mutex<DeviceRegistry> {
    DEVICE_REGISTRY
        .get()
        .expect("Device registry not initialized")
}

/// Register a device
pub fn register(name: String, dev_number: u32) -> Arc<MachDevice> {
    registry().lock().register(name, dev_number)
}

/// Lookup device by name
pub fn lookup(name: &str) -> Option<Arc<MachDevice>> {
    registry().lock().lookup(name)
}

/// Lookup device by port
pub fn dev_port_lookup(port: PortName) -> Option<Arc<MachDevice>> {
    registry().lock().lookup_by_port(port)
}

/// Enter port-device mapping
pub fn dev_port_enter(port: PortName, device_id: DeviceId) {
    registry().lock().port_enter(port, device_id);
}

/// Remove port-device mapping
pub fn dev_port_remove(port: PortName) {
    registry().lock().port_remove(port);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id() {
        let id = DeviceId(42);
        assert_eq!(id.0, 42);
        assert_ne!(id, DeviceId::NULL);
    }

    #[test]
    fn test_device_state() {
        assert_eq!(DeviceState::default(), DeviceState::Init);
    }

    #[test]
    fn test_mach_device() {
        let dev = MachDevice::new(DeviceId(1), "test".into(), 0x0100);
        assert_eq!(dev.major(), 1);
        assert_eq!(dev.minor(), 0);
        assert!(!dev.is_open());
    }
}
