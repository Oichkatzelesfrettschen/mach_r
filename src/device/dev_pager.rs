//! Device Pager - Memory Object Interface for Devices
//!
//! Based on Mach4 device/dev_pager.c
//! Provides external pager interface for memory-mapped devices.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use crate::device::dev_hdr::{DeviceId, MachDevice};
use crate::ipc::PortName;
use crate::mach_vm::vm_page::PAGE_SIZE;

// ============================================================================
// Device Pager Entry
// ============================================================================

/// Device pager entry
///
/// Represents a memory-mapped region of a device.
#[derive(Debug)]
pub struct DevPagerEntry {
    /// Device being paged
    pub device_id: DeviceId,

    /// Offset into device
    pub offset: u64,

    /// Size of mapped region
    pub size: u64,

    /// Memory object port
    pub mem_obj: Mutex<Option<PortName>>,

    /// Memory object control port
    pub mem_obj_control: Mutex<Option<PortName>>,

    /// Protection flags
    pub protection: u32,

    /// Is mapping active?
    pub active: AtomicBool,

    /// Reference count
    ref_count: AtomicU32,
}

impl DevPagerEntry {
    pub fn new(device_id: DeviceId, offset: u64, size: u64) -> Self {
        Self {
            device_id,
            offset,
            size,
            mem_obj: Mutex::new(None),
            mem_obj_control: Mutex::new(None),
            protection: 0x3, // Read/Write
            active: AtomicBool::new(true),
            ref_count: AtomicU32::new(1),
        }
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Check if offset is within this mapping
    pub fn contains(&self, offset: u64) -> bool {
        offset >= self.offset && offset < self.offset + self.size
    }

    /// Translate VM offset to device offset
    pub fn translate(&self, vm_offset: u64) -> Option<u64> {
        if vm_offset < self.size {
            Some(self.offset + vm_offset)
        } else {
            None
        }
    }
}

// ============================================================================
// Device Pager
// ============================================================================

/// Device pager identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DevPagerId(pub u64);

/// Device pager
///
/// Implements the external pager interface for devices.
#[derive(Debug)]
pub struct DevPager {
    /// Pager ID
    pub id: DevPagerId,

    /// Associated device
    pub device: Arc<MachDevice>,

    /// Pager entries (by VM offset)
    entries: Mutex<BTreeMap<u64, Arc<DevPagerEntry>>>,

    /// Pager port (for VM system to call us)
    pub pager_port: Mutex<Option<PortName>>,

    /// Is pager active?
    pub active: AtomicBool,
}

impl DevPager {
    pub fn new(id: DevPagerId, device: Arc<MachDevice>) -> Self {
        Self {
            id,
            device,
            entries: Mutex::new(BTreeMap::new()),
            pager_port: Mutex::new(None),
            active: AtomicBool::new(true),
        }
    }

    /// Create a mapping
    pub fn create_mapping(&self, offset: u64, size: u64) -> Arc<DevPagerEntry> {
        let entry = Arc::new(DevPagerEntry::new(self.device.id, offset, size));
        self.entries.lock().insert(offset, Arc::clone(&entry));
        entry
    }

    /// Remove a mapping
    pub fn remove_mapping(&self, offset: u64) {
        self.entries.lock().remove(&offset);
    }

    /// Find entry containing offset
    pub fn find_entry(&self, offset: u64) -> Option<Arc<DevPagerEntry>> {
        let entries = self.entries.lock();

        // Find entry containing this offset
        for (_, entry) in entries.iter() {
            if entry.contains(offset) {
                return Some(Arc::clone(entry));
            }
        }
        None
    }

    /// Handle page-in request from VM system
    pub fn page_in(&self, offset: u64, _length: u64) -> Result<Vec<u8>, DevPagerError> {
        let entry = self
            .find_entry(offset)
            .ok_or(DevPagerError::InvalidOffset)?;

        let _dev_offset = entry
            .translate(offset - entry.offset)
            .ok_or(DevPagerError::InvalidOffset)?;

        // Read from device
        if let Some(ops) = self.device.get_ops() {
            if let Some(_read_fn) = &ops.read {
                // Would create an I/O request and read from device
                // For now, return zeros
                return Ok(vec![0u8; PAGE_SIZE]);
            }
        }

        Err(DevPagerError::DeviceError)
    }

    /// Handle page-out request from VM system
    pub fn page_out(&self, offset: u64, data: &[u8]) -> Result<(), DevPagerError> {
        let entry = self
            .find_entry(offset)
            .ok_or(DevPagerError::InvalidOffset)?;

        let dev_offset = entry
            .translate(offset - entry.offset)
            .ok_or(DevPagerError::InvalidOffset)?;

        // Write to device
        if let Some(ops) = self.device.get_ops() {
            if let Some(_write_fn) = &ops.write {
                // Would create an I/O request and write to device
                let _ = (dev_offset, data);
                return Ok(());
            }
        }

        Err(DevPagerError::DeviceError)
    }

    /// Terminate the pager
    pub fn terminate(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.entries.lock().clear();
    }
}

/// Device pager errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevPagerError {
    /// Invalid offset
    InvalidOffset,
    /// Device error
    DeviceError,
    /// Not supported
    NotSupported,
    /// Pager terminated
    Terminated,
}

// ============================================================================
// Device Pager Manager
// ============================================================================

/// Device pager manager
pub struct DevPagerManager {
    /// All pagers
    pagers: BTreeMap<DevPagerId, Arc<DevPager>>,
    /// Pagers by device
    by_device: BTreeMap<DeviceId, DevPagerId>,
    /// Pagers by port
    by_port: BTreeMap<PortName, DevPagerId>,
    /// Next pager ID
    next_id: u64,
}

impl DevPagerManager {
    pub fn new() -> Self {
        Self {
            pagers: BTreeMap::new(),
            by_device: BTreeMap::new(),
            by_port: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Create a pager for a device
    pub fn create(&mut self, device: Arc<MachDevice>) -> Arc<DevPager> {
        // Check if pager already exists for this device
        if let Some(&pager_id) = self.by_device.get(&device.id) {
            if let Some(pager) = self.pagers.get(&pager_id) {
                return Arc::clone(pager);
            }
        }

        let id = DevPagerId(self.next_id);
        self.next_id += 1;

        let pager = Arc::new(DevPager::new(id, Arc::clone(&device)));
        self.pagers.insert(id, Arc::clone(&pager));
        self.by_device.insert(device.id, id);

        pager
    }

    /// Find pager by device
    pub fn find_by_device(&self, device_id: DeviceId) -> Option<Arc<DevPager>> {
        let pager_id = self.by_device.get(&device_id)?;
        self.pagers.get(pager_id).cloned()
    }

    /// Find pager by port
    pub fn find_by_port(&self, port: PortName) -> Option<Arc<DevPager>> {
        let pager_id = self.by_port.get(&port)?;
        self.pagers.get(pager_id).cloned()
    }

    /// Register pager port
    pub fn register_port(&mut self, pager_id: DevPagerId, port: PortName) {
        if let Some(pager) = self.pagers.get(&pager_id) {
            *pager.pager_port.lock() = Some(port);
            self.by_port.insert(port, pager_id);
        }
    }

    /// Destroy a pager
    pub fn destroy(&mut self, pager_id: DevPagerId) {
        if let Some(pager) = self.pagers.remove(&pager_id) {
            pager.terminate();
            self.by_device.remove(&pager.device.id);
            if let Some(port) = *pager.pager_port.lock() {
                self.by_port.remove(&port);
            }
        }
    }
}

impl Default for DevPagerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static DEV_PAGER_MANAGER: spin::Once<Mutex<DevPagerManager>> = spin::Once::new();

fn pager_manager() -> &'static Mutex<DevPagerManager> {
    DEV_PAGER_MANAGER.call_once(|| Mutex::new(DevPagerManager::new()));
    DEV_PAGER_MANAGER.get().unwrap()
}

/// Create a device pager
pub fn device_pager_create(device: Arc<MachDevice>) -> Arc<DevPager> {
    pager_manager().lock().create(device)
}

/// Find pager by device
pub fn device_pager_lookup(device_id: DeviceId) -> Option<Arc<DevPager>> {
    pager_manager().lock().find_by_device(device_id)
}

/// Destroy device pager
pub fn device_pager_destroy(pager_id: DevPagerId) {
    pager_manager().lock().destroy(pager_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::dev_hdr::DeviceId;

    #[test]
    fn test_dev_pager_entry() {
        let entry = DevPagerEntry::new(DeviceId(1), 0x1000, 0x4000);
        assert!(entry.contains(0x1000));
        assert!(entry.contains(0x4FFF));
        assert!(!entry.contains(0x5000));
    }

    #[test]
    fn test_translate() {
        let entry = DevPagerEntry::new(DeviceId(1), 0x1000, 0x4000);
        assert_eq!(entry.translate(0), Some(0x1000));
        assert_eq!(entry.translate(0x100), Some(0x1100));
        assert_eq!(entry.translate(0x5000), None);
    }
}
