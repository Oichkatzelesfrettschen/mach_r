//! Timer driver implementation

use super::DriverError;

#[cfg(target_arch = "aarch64")]
use super::{Driver, DeviceDriver, DeviceInfo, PollStatus};

#[cfg(target_arch = "aarch64")]
use alloc::boxed::Box;

#[cfg(target_arch = "aarch64")]
use core::arch::asm;

/// ARMv8 Generic Timer driver
#[cfg(target_arch = "aarch64")]
pub struct ArmV8TimerDriver {
    initialized: bool,
    frequency: u64,
    last_tick: u64,
}

#[cfg(target_arch = "aarch64")]
impl ArmV8TimerDriver {
    /// Create new ARMv8 timer driver
    pub fn new() -> Self {
        Self {
            initialized: false,
            frequency: 0,
            last_tick: 0,
        }
    }
    
    /// Read system counter frequency
    fn read_frequency(&mut self) -> u64 {
        let mut freq: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) freq);
        }
        freq
    }
    
    /// Read current counter value
    pub fn read_counter(&self) -> u64 {
        let mut count: u64;
        unsafe {
            asm!("mrs {}, cntpct_el0", out(reg) count);
        }
        count
    }
    
    /// Read virtual counter value
    pub fn read_virtual_counter(&self) -> u64 {
        let mut count: u64;
        unsafe {
            asm!("mrs {}, cntvct_el0", out(reg) count);
        }
        count
    }
    
    /// Set physical timer compare value
    pub fn set_physical_compare(&self, value: u64) {
        unsafe {
            asm!("msr cntps_cval_el1, {}", in(reg) value);
        }
    }
    
    /// Set virtual timer compare value
    pub fn set_virtual_compare(&self, value: u64) {
        unsafe {
            asm!("msr cntv_cval_el0, {}", in(reg) value);
        }
    }
    
    /// Enable physical timer
    pub fn enable_physical_timer(&self) {
        let ctl: u32 = 1; // Enable bit
        unsafe {
            asm!("msr cntps_ctl_el1, {0:w}", in(reg) ctl);
        }
    }
    
    /// Disable physical timer
    pub fn disable_physical_timer(&self) {
        let ctl: u32 = 0;
        unsafe {
            asm!("msr cntps_ctl_el1, {0:w}", in(reg) ctl);
        }
    }
    
    /// Enable virtual timer
    pub fn enable_virtual_timer(&self) {
        let ctl: u32 = 1; // Enable bit
        unsafe {
            asm!("msr cntv_ctl_el0, {0:w}", in(reg) ctl);
        }
    }
    
    /// Disable virtual timer
    pub fn disable_virtual_timer(&self) {
        let ctl: u32 = 0;
        unsafe {
            asm!("msr cntv_ctl_el0, {0:w}", in(reg) ctl);
        }
    }
    
    /// Read physical timer control
    pub fn read_physical_control(&self) -> u32 {
        let mut ctl: u32;
        unsafe {
            asm!("mrs {0:w}, cntps_ctl_el1", out(reg) ctl);
        }
        ctl
    }
    
    /// Read virtual timer control
    pub fn read_virtual_control(&self) -> u32 {
        let mut ctl: u32;
        unsafe {
            asm!("mrs {0:w}, cntv_ctl_el0", out(reg) ctl);
        }
        ctl
    }
    
    /// Convert counter ticks to microseconds
    pub fn ticks_to_us(&self, ticks: u64) -> u64 {
        if self.frequency == 0 {
            return 0;
        }
        (ticks * 1_000_000) / self.frequency
    }
    
    /// Convert microseconds to counter ticks
    pub fn us_to_ticks(&self, us: u64) -> u64 {
        if self.frequency == 0 {
            return 0;
        }
        (us * self.frequency) / 1_000_000
    }
    
    /// Get elapsed time since initialization in microseconds
    pub fn elapsed_us(&mut self) -> u64 {
        let current = self.read_counter();
        let elapsed = current.saturating_sub(self.last_tick);
        self.ticks_to_us(elapsed)
    }
    
    /// Reset elapsed time counter
    pub fn reset_elapsed(&mut self) {
        self.last_tick = self.read_counter();
    }
    
    /// Delay for specified microseconds (busy wait)
    pub fn delay_us(&self, us: u64) {
        let start = self.read_counter();
        let delay_ticks = self.us_to_ticks(us);
        let target = start + delay_ticks;
        
        while self.read_counter() < target {
            core::hint::spin_loop();
        }
    }
    
    /// Set up periodic timer interrupt
    pub fn setup_periodic_timer(&self, interval_us: u64) -> Result<(), DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        let current = self.read_counter();
        let interval_ticks = self.us_to_ticks(interval_us);
        
        // Set compare value
        self.set_virtual_compare(current + interval_ticks);
        
        // Enable timer
        self.enable_virtual_timer();
        
        Ok(())
    }
    
    /// Get timer frequency in Hz
    pub fn frequency(&self) -> u64 {
        self.frequency
    }
    
    /// Get current timestamp in microseconds since boot
    pub fn timestamp_us(&self) -> u64 {
        self.ticks_to_us(self.read_counter())
    }
}

#[cfg(target_arch = "aarch64")]
impl Driver for ArmV8TimerDriver {
    fn name(&self) -> &str {
        "armv8-timer"
    }
    
    fn init(&mut self) -> Result<(), DriverError> {
        if self.initialized {
            return Ok(());
        }
        
        // Read system counter frequency
        self.frequency = self.read_frequency();
        
        if self.frequency == 0 {
            return Err(DriverError::DeviceNotReady);
        }
        
        // Initialize last tick counter
        self.last_tick = self.read_counter();
        
        // Disable timers initially
        self.disable_physical_timer();
        self.disable_virtual_timer();
        
        self.initialized = true;
        
        // Print initialization info
        crate::boot::serial_println(&format_timer_info(self.frequency));
        
        Ok(())
    }
    
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        if buffer.len() < 8 {
            return Err(DriverError::BufferTooSmall);
        }
        
        // Return current counter value as bytes
        let counter = self.read_counter();
        let bytes = counter.to_le_bytes();
        buffer[..8].copy_from_slice(&bytes);
        
        Ok(8)
    }
    
    fn write(&mut self, buffer: &[u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        if buffer.len() < 8 {
            return Err(DriverError::BufferTooSmall);
        }
        
        // Set compare value from buffer
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&buffer[..8]);
        let compare_value = u64::from_le_bytes(bytes);
        
        self.set_virtual_compare(compare_value);
        
        Ok(8)
    }
    
    fn control(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        match cmd {
            0 => {
                // Get frequency
                Ok(self.frequency as usize)
            }
            1 => {
                // Get current counter
                Ok(self.read_counter() as usize)
            }
            2 => {
                // Delay microseconds
                self.delay_us(arg as u64);
                Ok(0)
            }
            3 => {
                // Setup periodic timer
                self.setup_periodic_timer(arg as u64)?;
                Ok(0)
            }
            4 => {
                // Enable virtual timer
                self.enable_virtual_timer();
                Ok(0)
            }
            5 => {
                // Disable virtual timer
                self.disable_virtual_timer();
                Ok(0)
            }
            6 => {
                // Get timestamp in microseconds
                Ok(self.timestamp_us() as usize)
            }
            _ => Err(DriverError::NotSupported),
        }
    }
    
    fn poll(&self) -> PollStatus {
        PollStatus {
            can_read: self.initialized,
            can_write: self.initialized,
            has_error: false,
        }
    }
}

#[cfg(target_arch = "aarch64")]
impl DeviceDriver for ArmV8TimerDriver {
    fn name(&self) -> &str {
        "armv8-timer"
    }
    
    fn version(&self) -> (u32, u32, u32) {
        (1, 0, 0)
    }
    
    fn init(&mut self) -> Result<(), DriverError> {
        Driver::init(self).map_err(|_| DriverError::HardwareError)
    }
    
    fn shutdown(&mut self) -> Result<(), DriverError> {
        if !self.initialized {
            return Ok(());
        }
        
        // Disable both timers
        self.disable_physical_timer();
        self.disable_virtual_timer();
        
        self.initialized = false;
        Ok(())
    }
    
    fn ioctl(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        self.control(cmd, arg)
            .map_err(|_| DriverError::HardwareError)
    }
    
    fn can_handle(&self, device: &DeviceInfo) -> bool {
        // Check if device is compatible with ARMv8 timer
        if let Ok(compat_key) = heapless::String::try_from("compatible") {
            if let Some(compatible) = device.properties.get(&compat_key) {
                return compatible == "arm,armv8-timer" || compatible.starts_with("arm,armv8");
            }
        }
        false
    }
    
    fn bind(&mut self, _device: &DeviceInfo) -> Result<(), DriverError> {
        Driver::init(self)
    }
    
    fn unbind(&mut self, _device: &DeviceInfo) -> Result<(), DriverError> {
        self.shutdown()
    }
}

/// Format timer information for display
#[allow(dead_code)]
fn format_timer_info(frequency: u64) -> heapless::String<64> {
    let mut result = heapless::String::new();
    
    result.push_str("ARMv8 Timer: ").unwrap();
    result.push_str(&format_frequency(frequency)).unwrap();
    result.push_str(" Hz").unwrap();
    
    result
}

/// Simple frequency formatting
#[allow(dead_code)]
fn format_frequency(freq: u64) -> heapless::String<16> {
    let mut result = heapless::String::new();
    
    if freq >= 1_000_000 {
        let mhz = freq / 1_000_000;
        format_number(&mut result, mhz as usize).unwrap();
        result.push('M').unwrap();
    } else if freq >= 1_000 {
        let khz = freq / 1_000;
        format_number(&mut result, khz as usize).unwrap();
        result.push('K').unwrap();
    } else {
        format_number(&mut result, freq as usize).unwrap();
    }
    
    result
}

/// Format number into string
#[allow(dead_code)]
fn format_number(s: &mut heapless::String<16>, n: usize) -> Result<(), ()> {
    if n == 0 {
        s.push('0')?;
        return Ok(());
    }
    
    let mut num = n;
    let mut digits = [0u8; 16];
    let mut count = 0;
    
    while num > 0 && count < digits.len() {
        digits[count] = (num % 10) as u8 + b'0';
        num /= 10;
        count += 1;
    }
    
    for i in (0..count).rev() {
        s.push(digits[i] as char)?;
    }
    
    Ok(())
}

/// Initialize ARMv8 timer driver
#[cfg(target_arch = "aarch64")]
pub fn init() -> Result<(), DriverError> {
    let mut timer = ArmV8TimerDriver::new();
    Driver::init(&mut timer)?;

    // Register with device manager if available
    if let Some(manager) = crate::drivers::device_manager_mut() {
        let _ = manager.register_driver(Box::new(timer));
    }

    Ok(())
}

#[cfg(not(target_arch = "aarch64"))]
pub fn init() -> Result<(), DriverError> {
    // No timer driver on non-ARM platforms yet
    Ok(())
}

/// Global timer instance
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
static mut SYSTEM_TIMER: Option<ArmV8TimerDriver> = None;

#[cfg(not(target_arch = "aarch64"))]
#[allow(dead_code)]
static mut SYSTEM_TIMER: Option<()> = None;

/// Initialize system timer
#[cfg(target_arch = "aarch64")]
pub fn init_system_timer() -> Result<(), DriverError> {
    unsafe {
        let mut timer = ArmV8TimerDriver::new();
        Driver::init(&mut timer)?;
        SYSTEM_TIMER = Some(timer);
    }
    Ok(())
}

#[cfg(not(target_arch = "aarch64"))]
pub fn init_system_timer() -> Result<(), DriverError> {
    Ok(())
}

/// Get current system time in microseconds
#[cfg(target_arch = "aarch64")]
pub fn system_time_us() -> u64 {
    unsafe {
        if let Some(ref timer) = SYSTEM_TIMER {
            timer.timestamp_us()
        } else {
            0
        }
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn system_time_us() -> u64 {
    0
}

/// Delay for specified microseconds
#[cfg(target_arch = "aarch64")]
pub fn delay_us(us: u64) {
    unsafe {
        if let Some(ref timer) = SYSTEM_TIMER {
            timer.delay_us(us);
        } else {
            // Fallback busy wait
            for _ in 0..(us * 100) {
                core::hint::spin_loop();
            }
        }
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn delay_us(us: u64) {
    // Fallback busy wait
    for _ in 0..(us * 100) {
        core::hint::spin_loop();
    }
}

/// Delay for specified milliseconds
pub fn delay_ms(ms: u64) {
    delay_us(ms * 1000);
}