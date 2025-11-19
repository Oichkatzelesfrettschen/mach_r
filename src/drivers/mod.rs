//! Device driver framework for Mach_R
//!
//! Provides abstractions for both in-kernel and userspace drivers

pub mod serial;
pub mod scheme;
pub mod virtio;
pub mod uart;
pub mod timer;
pub mod interrupt;

use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::Mutex;

/// Driver trait that all drivers must implement
pub trait Driver {
    /// Driver name
    fn name(&self) -> &str;
    
    /// Initialize the driver
    fn init(&mut self) -> Result<(), DriverError>;
    
    /// Read data from device
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, DriverError>;
    
    /// Write data to device
    fn write(&mut self, buffer: &[u8]) -> Result<usize, DriverError>;
    
    /// Control operations (ioctl-like)
    fn control(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError>;
    
    /// Check if device is ready
    fn poll(&self) -> PollStatus;
}

/// Driver errors - extended for enhanced framework
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriverError {
    NotInitialized,
    InvalidOperation,
    DeviceNotReady,
    BufferTooSmall,
    IoError,
    NotSupported,
    // Enhanced error types
    DeviceNotFound,
    DriverNotFound,
    DeviceBusy,
    InvalidConfig,
    HardwareError,
    NoResources,
    PermissionDenied,
    NotReady,
    Generic(&'static str),
}

/// Poll status for async operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PollStatus {
    pub can_read: bool,
    pub can_write: bool,
    pub has_error: bool,
}

/// Device manager - tracks all registered drivers
pub struct DeviceManager {
    drivers: Mutex<Vec<Box<dyn Driver + Send>>>,
}

impl DeviceManager {
    /// Create new device manager
    pub const fn new() -> Self {
        DeviceManager {
            drivers: Mutex::new(Vec::new()),
        }
    }
    
    /// Register a driver
    pub fn register(&self, mut driver: Box<dyn Driver + Send>) -> Result<(), DriverError> {
        driver.init()?;
        let mut drivers = self.drivers.lock();
        drivers.push(driver);
        Ok(())
    }
    
    /// Find driver by name
    pub fn find(&self, name: &str) -> Option<usize> {
        let drivers = self.drivers.lock();
        drivers.iter().position(|d| d.name() == name)
    }
}

/// Global device manager instance
static DEVICE_MANAGER: DeviceManager = DeviceManager::new();

/// Register a device driver
pub fn register_driver(driver: Box<dyn Driver + Send>) -> Result<(), DriverError> {
    DEVICE_MANAGER.register(driver)
}

// Advanced driver types for the new framework
use heapless::{String, Vec as HeaplessVec, FnvIndexMap};

/// Enhanced device driver trait
pub trait DeviceDriver: Send + Sync {
    /// Driver name
    fn name(&self) -> &str;
    
    /// Driver version
    fn version(&self) -> (u32, u32, u32);
    
    /// Initialize the driver
    fn init(&mut self) -> Result<(), DriverError>;
    
    /// Shutdown the driver
    fn shutdown(&mut self) -> Result<(), DriverError>;
    
    /// Handle device-specific operations
    fn ioctl(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError>;
    
    /// Check if driver can handle this device
    fn can_handle(&self, device: &DeviceInfo) -> bool;
    
    /// Bind to a specific device
    fn bind(&mut self, device: &DeviceInfo) -> Result<(), DriverError>;
    
    /// Unbind from device
    fn unbind(&mut self, device: &DeviceInfo) -> Result<(), DriverError>;
}

/// Enhanced device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: DeviceId,
    pub name: String<32>,
    pub device_type: DeviceType,
    pub addresses: HeaplessVec<DeviceAddress, 8>,
    pub interrupts: HeaplessVec<u32, 8>,
    pub properties: FnvIndexMap<String<16>, String<32>, 16>,
    pub parent: Option<DeviceId>,
    pub state: DeviceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceType {
    Character,
    Block,
    Network,
    Input,
    Output,
    Gpio,
    Timer,
    InterruptController,
    Power,
    Bus,
    Memory,
    Platform,
}

#[derive(Debug, Clone, Copy)]
pub enum DeviceAddress {
    Mmio { base: u64, size: usize },
    Io { port: u16 },
    Physical { addr: u64, size: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceState {
    Discovered,
    Initializing,
    Active,
    Suspended,
    Failed,
    Removing,
}

// DriverError already defined above

/// Enhanced device manager
pub struct EnhancedDeviceManager {
    devices: HeaplessVec<DeviceInfo, 128>,
    device_index: FnvIndexMap<DeviceId, usize, 128>,
    enhanced_drivers: HeaplessVec<Box<dyn DeviceDriver>, 64>,
    next_device_id: u64,
}

impl EnhancedDeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HeaplessVec::new(),
            device_index: FnvIndexMap::new(),
            enhanced_drivers: HeaplessVec::new(),
            next_device_id: 1,
        }
    }
    
    pub fn register_driver(&mut self, driver: Box<dyn DeviceDriver>) -> Result<(), DriverError> {
        self.enhanced_drivers.push(driver).map_err(|_| DriverError::NoResources)
    }
}

/// Global enhanced device manager
static mut ENHANCED_DEVICE_MANAGER: Option<EnhancedDeviceManager> = None;

/// Get enhanced device manager
pub fn device_manager() -> Option<&'static EnhancedDeviceManager> {
    unsafe { ENHANCED_DEVICE_MANAGER.as_ref() }
}

/// Get mutable enhanced device manager
pub fn device_manager_mut() -> Option<&'static mut EnhancedDeviceManager> {
    unsafe { ENHANCED_DEVICE_MANAGER.as_mut() }
}

/// Initialize enhanced device manager
pub fn init_enhanced_device_manager() -> Result<(), DriverError> {
    unsafe {
        if ENHANCED_DEVICE_MANAGER.is_some() {
            return Ok(());
        }
        
        let mut manager = EnhancedDeviceManager::new();
        
        // Register new drivers
        if let Ok(uart_driver) = create_uart_driver() {
            manager.register_driver(uart_driver)?;
        }
        #[cfg(target_arch = "aarch64")]
        if let Ok(timer_driver) = create_timer_driver() {
            manager.register_driver(timer_driver)?;
        }
        if let Ok(interrupt_driver) = create_interrupt_driver() {
            manager.register_driver(interrupt_driver)?;
        }
        
        ENHANCED_DEVICE_MANAGER = Some(manager);
    }
    Ok(())
}

/// Create UART driver instance
fn create_uart_driver() -> Result<Box<dyn DeviceDriver>, DriverError> {
    Ok(Box::new(uart::Pl011Driver::new()))
}

/// Create timer driver instance
#[cfg(target_arch = "aarch64")]
fn create_timer_driver() -> Result<Box<dyn DeviceDriver>, DriverError> {
    Ok(Box::new(timer::ArmV8TimerDriver::new()))
}

/// Create interrupt controller driver instance
fn create_interrupt_driver() -> Result<Box<dyn DeviceDriver>, DriverError> {
    Ok(Box::new(interrupt::Gic400Driver::new()))
}

/// Initialize driver subsystem
pub fn init() {
    // Initialize enhanced device manager
    let _ = init_enhanced_device_manager();
    
    // Initialize serial console first
    serial::init();
    
    // Initialize new pure Rust drivers
    let _ = uart::init();
    #[cfg(target_arch = "aarch64")]
    let _ = timer::init();
    let _ = interrupt::init();
    
    // Initialize VirtIO drivers for QEMU
    virtio::init();
    
    crate::println!("Driver subsystem initialized with enhanced framework");
}