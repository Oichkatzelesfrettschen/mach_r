//! ARM GIC (Generic Interrupt Controller) driver
//! Pure Rust driver for ARM GIC-400 and compatible interrupt controllers

use super::{DeviceAddress, DeviceDriver, DeviceInfo, Driver, DriverError, PollStatus};
use alloc::boxed::Box;
use core::ptr::{read_volatile, write_volatile};
use heapless::Vec;

/// Maximum number of interrupts supported
const MAX_INTERRUPTS: usize = 1024;
/// Maximum number of interrupt handlers
const MAX_HANDLERS: usize = 256;

/// GIC Distributor register offsets
const GICD_CTLR: usize = 0x000; // Distributor Control Register
const GICD_TYPER: usize = 0x004; // Interrupt Controller Type Register
const GICD_ISENABLER: usize = 0x100; // Interrupt Set-Enable Registers
const GICD_ICENABLER: usize = 0x180; // Interrupt Clear-Enable Registers
const GICD_ISPENDR: usize = 0x200; // Interrupt Set-Pending Registers
const GICD_ICPENDR: usize = 0x280; // Interrupt Clear-Pending Registers
const GICD_ISACTIVER: usize = 0x300; // Interrupt Set-Active Registers
const GICD_ICACTIVER: usize = 0x380; // Interrupt Clear-Active Registers
const GICD_IPRIORITYR: usize = 0x400; // Interrupt Priority Registers
const GICD_ITARGETSR: usize = 0x800; // Interrupt Processor Targets Registers
const GICD_ICFGR: usize = 0xC00; // Interrupt Configuration Registers

/// GIC CPU Interface register offsets
const GICC_CTLR: usize = 0x000; // CPU Interface Control Register
const GICC_PMR: usize = 0x004; // Interrupt Priority Mask Register
const GICC_BPR: usize = 0x008; // Binary Point Register
const GICC_IAR: usize = 0x00C; // Interrupt Acknowledge Register
const GICC_EOIR: usize = 0x010; // End of Interrupt Register
const GICC_RPR: usize = 0x014; // Running Priority Register
const GICC_HPPIR: usize = 0x018; // Highest Priority Pending Interrupt Register

/// GIC control register bits
const GICD_CTLR_ENABLE: u32 = 1 << 0;
const GICC_CTLR_ENABLE: u32 = 1 << 0;

/// Special interrupt numbers
const SGI_MAX: u32 = 15; // Software Generated Interrupts
const PPI_MAX: u32 = 31; // Private Peripheral Interrupts
const SPI_BASE: u32 = 32; // Shared Peripheral Interrupts start
const SPURIOUS_INTERRUPT: u32 = 1023;

/// Interrupt priority levels
const IRQ_PRIORITY_HIGH: u8 = 0x40;
const IRQ_PRIORITY_NORMAL: u8 = 0x80;
const IRQ_PRIORITY_LOW: u8 = 0xC0;

/// Interrupt handler function type
pub type InterruptHandler = fn(irq: u32);

/// Interrupt configuration
#[derive(Debug, Clone, Copy)]
pub enum InterruptTrigger {
    /// Level-sensitive interrupt
    Level,
    /// Edge-triggered interrupt
    Edge,
}

/// ARM GIC-400 interrupt controller driver
pub struct Gic400Driver {
    distributor_base: usize,
    cpu_interface_base: usize,
    initialized: bool,
    max_interrupts: u32,
    handlers: Vec<Option<InterruptHandler>, MAX_HANDLERS>,
}

impl Gic400Driver {
    /// Create new GIC-400 driver
    pub fn new() -> Self {
        Self {
            distributor_base: 0x08000000,   // Default distributor base
            cpu_interface_base: 0x08010000, // Default CPU interface base
            initialized: false,
            max_interrupts: 0,
            handlers: Vec::new(),
        }
    }

    /// Create GIC-400 driver with custom base addresses
    pub fn new_with_bases(dist_base: usize, cpu_base: usize) -> Self {
        Self {
            distributor_base: dist_base,
            cpu_interface_base: cpu_base,
            initialized: false,
            max_interrupts: 0,
            handlers: Vec::new(),
        }
    }

    /// Read distributor register
    fn read_dist_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.distributor_base + offset) as *const u32) }
    }

    /// Write distributor register
    fn write_dist_reg(&self, offset: usize, value: u32) {
        unsafe {
            write_volatile((self.distributor_base + offset) as *mut u32, value);
        }
    }

    /// Read CPU interface register
    fn read_cpu_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.cpu_interface_base + offset) as *const u32) }
    }

    /// Write CPU interface register
    fn write_cpu_reg(&self, offset: usize, value: u32) {
        unsafe {
            write_volatile((self.cpu_interface_base + offset) as *mut u32, value);
        }
    }

    /// Get maximum number of interrupts supported
    fn get_max_interrupts(&self) -> u32 {
        let typer = self.read_dist_reg(GICD_TYPER);
        // IT_LINES_NUMBER field gives (N+1)*32 interrupts
        let it_lines = (typer & 0x1F) + 1;
        it_lines * 32
    }

    /// Enable interrupt
    pub fn enable_interrupt(&self, irq: u32) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 32) as usize;
        let bit_idx = irq % 32;

        self.write_dist_reg(GICD_ISENABLER + (reg_idx * 4), 1 << bit_idx);

        Ok(())
    }

    /// Disable interrupt
    pub fn disable_interrupt(&self, irq: u32) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 32) as usize;
        let bit_idx = irq % 32;

        self.write_dist_reg(GICD_ICENABLER + (reg_idx * 4), 1 << bit_idx);

        Ok(())
    }

    /// Set interrupt priority
    pub fn set_interrupt_priority(&self, irq: u32, priority: u8) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 4) as usize;
        let byte_idx = (irq % 4) * 8;

        let reg_addr = GICD_IPRIORITYR + (reg_idx * 4);
        let current = self.read_dist_reg(reg_addr);
        let mask = !(0xFF << byte_idx);
        let new_val = (current & mask) | ((priority as u32) << byte_idx);

        self.write_dist_reg(reg_addr, new_val);

        Ok(())
    }

    /// Set interrupt target CPU
    pub fn set_interrupt_target(&self, irq: u32, cpu_mask: u8) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts || irq < SPI_BASE {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 4) as usize;
        let byte_idx = (irq % 4) * 8;

        let reg_addr = GICD_ITARGETSR + (reg_idx * 4);
        let current = self.read_dist_reg(reg_addr);
        let mask = !(0xFF << byte_idx);
        let new_val = (current & mask) | ((cpu_mask as u32) << byte_idx);

        self.write_dist_reg(reg_addr, new_val);

        Ok(())
    }

    /// Configure interrupt trigger type
    pub fn set_interrupt_trigger(
        &self,
        irq: u32,
        trigger: InterruptTrigger,
    ) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 16) as usize;
        let bit_idx = ((irq % 16) * 2) + 1; // Configuration is in bit 1 of each 2-bit field

        let reg_addr = GICD_ICFGR + (reg_idx * 4);
        let current = self.read_dist_reg(reg_addr);

        let new_val = match trigger {
            InterruptTrigger::Level => current & !(1 << bit_idx),
            InterruptTrigger::Edge => current | (1 << bit_idx),
        };

        self.write_dist_reg(reg_addr, new_val);

        Ok(())
    }

    /// Register interrupt handler
    pub fn register_handler(
        &mut self,
        irq: u32,
        handler: InterruptHandler,
    ) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let irq_idx = irq as usize;
        if irq_idx >= self.handlers.len() {
            // Extend handlers vector if needed
            while self.handlers.len() <= irq_idx {
                self.handlers.push(None).map_err(|_| DriverError::IoError)?;
            }
        }

        self.handlers[irq_idx] = Some(handler);

        // Configure interrupt with default settings
        self.set_interrupt_priority(irq, IRQ_PRIORITY_NORMAL)?;
        if irq >= SPI_BASE {
            self.set_interrupt_target(irq, 0x01)?; // Target CPU 0
        }
        self.set_interrupt_trigger(irq, InterruptTrigger::Level)?;

        Ok(())
    }

    /// Acknowledge interrupt and return interrupt number
    pub fn acknowledge_interrupt(&self) -> u32 {
        if !self.initialized {
            return SPURIOUS_INTERRUPT;
        }

        self.read_cpu_reg(GICC_IAR) & 0x3FF // Bottom 10 bits contain interrupt ID
    }

    /// End of interrupt - signal completion
    pub fn end_interrupt(&self, irq: u32) {
        if self.initialized {
            self.write_cpu_reg(GICC_EOIR, irq);
        }
    }

    /// Handle interrupt (called from interrupt vector)
    pub fn handle_interrupt(&self) {
        let irq = self.acknowledge_interrupt();

        if irq == SPURIOUS_INTERRUPT {
            return;
        }

        // Call registered handler if available
        if let Some(Some(handler)) = self.handlers.get(irq as usize) {
            handler(irq);
        }

        // Signal end of interrupt
        self.end_interrupt(irq);
    }

    /// Get pending interrupt with highest priority
    pub fn get_highest_pending(&self) -> u32 {
        if !self.initialized {
            return SPURIOUS_INTERRUPT;
        }

        self.read_cpu_reg(GICC_HPPIR) & 0x3FF
    }

    /// Check if interrupt is pending
    pub fn is_interrupt_pending(&self, irq: u32) -> bool {
        if !self.initialized || irq >= self.max_interrupts {
            return false;
        }

        let reg_idx = (irq / 32) as usize;
        let bit_idx = irq % 32;

        let pending = self.read_dist_reg(GICD_ISPENDR + (reg_idx * 4));
        (pending & (1 << bit_idx)) != 0
    }

    /// Clear pending interrupt
    pub fn clear_pending(&self, irq: u32) -> Result<(), DriverError> {
        if !self.initialized || irq >= self.max_interrupts {
            return Err(DriverError::InvalidOperation);
        }

        let reg_idx = (irq / 32) as usize;
        let bit_idx = irq % 32;

        self.write_dist_reg(GICD_ICPENDR + (reg_idx * 4), 1 << bit_idx);

        Ok(())
    }
}

impl Driver for Gic400Driver {
    fn name(&self) -> &str {
        "gic-400"
    }

    fn init(&mut self) -> Result<(), DriverError> {
        if self.initialized {
            return Ok(());
        }

        // Initialize handlers vector
        self.handlers = Vec::new();
        for _ in 0..MAX_HANDLERS {
            self.handlers.push(None).map_err(|_| DriverError::IoError)?;
        }

        // Disable distributor
        self.write_dist_reg(GICD_CTLR, 0);

        // Get maximum number of interrupts
        self.max_interrupts = self.get_max_interrupts();

        // Disable all interrupts
        let num_regs = (self.max_interrupts + 31) / 32;
        for i in 0..num_regs {
            self.write_dist_reg(GICD_ICENABLER + (i as usize * 4), 0xFFFFFFFF);
            self.write_dist_reg(GICD_ICPENDR + (i as usize * 4), 0xFFFFFFFF);
        }

        // Set default priorities
        let num_priority_regs = (self.max_interrupts + 3) / 4;
        for i in 0..num_priority_regs {
            self.write_dist_reg(
                GICD_IPRIORITYR + (i as usize * 4),
                0xA0A0A0A0, // Default priority
            );
        }

        // Set SPI targets to CPU 0
        for i in (SPI_BASE / 4)..(self.max_interrupts + 3) / 4 {
            self.write_dist_reg(
                GICD_ITARGETSR + (i as usize * 4),
                0x01010101, // Target CPU 0
            );
        }

        // Configure all interrupts as level-triggered
        let num_cfg_regs = (self.max_interrupts + 15) / 16;
        for i in 0..num_cfg_regs {
            self.write_dist_reg(GICD_ICFGR + (i as usize * 4), 0x00000000);
        }

        // Enable distributor
        self.write_dist_reg(GICD_CTLR, GICD_CTLR_ENABLE);

        // Configure CPU interface
        self.write_cpu_reg(GICC_PMR, 0xF0); // Priority mask
        self.write_cpu_reg(GICC_BPR, 0x00); // Binary point
        self.write_cpu_reg(GICC_CTLR, GICC_CTLR_ENABLE);

        self.initialized = true;

        // Print initialization info
        crate::boot::serial_println(&format_gic_info(self.max_interrupts));

        Ok(())
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        if buffer.len() < 4 {
            return Err(DriverError::BufferTooSmall);
        }

        // Return highest pending interrupt
        let pending = self.get_highest_pending();
        let bytes = pending.to_le_bytes();
        buffer[..4].copy_from_slice(&bytes);

        Ok(4)
    }

    fn write(&mut self, buffer: &[u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        if buffer.len() < 4 {
            return Err(DriverError::BufferTooSmall);
        }

        // Clear pending interrupt specified in buffer
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&buffer[..4]);
        let irq = u32::from_le_bytes(bytes);

        self.clear_pending(irq)?;

        Ok(4)
    }

    fn control(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }

        match cmd {
            0 => {
                // Get max interrupts
                Ok(self.max_interrupts as usize)
            }
            1 => {
                // Enable interrupt
                self.enable_interrupt(arg as u32)?;
                Ok(0)
            }
            2 => {
                // Disable interrupt
                self.disable_interrupt(arg as u32)?;
                Ok(0)
            }
            3 => {
                // Set interrupt priority (arg contains irq in lower 16 bits, priority in upper 16)
                let irq = (arg & 0xFFFF) as u32;
                let priority = ((arg >> 16) & 0xFF) as u8;
                self.set_interrupt_priority(irq, priority)?;
                Ok(0)
            }
            4 => {
                // Check if interrupt is pending
                let pending = self.is_interrupt_pending(arg as u32);
                Ok(if pending { 1 } else { 0 })
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

impl DeviceDriver for Gic400Driver {
    fn name(&self) -> &str {
        "gic-400"
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

        // Disable CPU interface and distributor
        self.write_cpu_reg(GICC_CTLR, 0);
        self.write_dist_reg(GICD_CTLR, 0);

        self.initialized = false;
        Ok(())
    }

    fn ioctl(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        self.control(cmd, arg)
            .map_err(|_| DriverError::HardwareError)
    }

    fn can_handle(&self, device: &DeviceInfo) -> bool {
        // Check if device is compatible with GIC-400
        if let Ok(compat_key) = heapless::String::try_from("compatible") {
            if let Some(compatible) = device.properties.get(&compat_key) {
                return compatible == "arm,gic-400" || compatible.starts_with("arm,gic");
            }
        }
        false
    }

    fn bind(&mut self, device: &DeviceInfo) -> Result<(), DriverError> {
        // Extract base addresses from device info
        if device.addresses.len() >= 2 {
            if let (Some(dist_addr), Some(cpu_addr)) =
                (device.addresses.first(), device.addresses.get(1))
            {
                if let (
                    DeviceAddress::Mmio {
                        base: dist_base,
                        size: _,
                    },
                    DeviceAddress::Mmio {
                        base: cpu_base,
                        size: _,
                    },
                ) = (dist_addr, cpu_addr)
                {
                    self.distributor_base = *dist_base as usize;
                    self.cpu_interface_base = *cpu_base as usize;
                }
            }
        }

        Driver::init(self)
    }

    fn unbind(&mut self, _device: &DeviceInfo) -> Result<(), DriverError> {
        self.shutdown()
    }
}

/// Format GIC information for display
fn format_gic_info(max_interrupts: u32) -> heapless::String<64> {
    let mut result = heapless::String::new();

    result.push_str("GIC-400: ").unwrap();
    format_number(&mut result, max_interrupts as usize).unwrap();
    result.push_str(" interrupts").unwrap();

    result
}

/// Format number into string
fn format_number(s: &mut heapless::String<64>, n: usize) -> Result<(), ()> {
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

/// Initialize GIC-400 interrupt controller
pub fn init() -> Result<(), DriverError> {
    let mut gic = Gic400Driver::new();
    Driver::init(&mut gic)?;

    // Register with device manager if available
    if let Some(manager) = crate::drivers::device_manager_mut() {
        let _ = manager.register_driver(Box::new(gic));
    }

    Ok(())
}

/// Global interrupt controller instance
static mut INTERRUPT_CONTROLLER: Option<Gic400Driver> = None;

/// Initialize system interrupt controller
pub fn init_system_interrupt_controller() -> Result<(), DriverError> {
    unsafe {
        let mut gic = Gic400Driver::new();
        Driver::init(&mut gic)?;
        INTERRUPT_CONTROLLER = Some(gic);
    }
    Ok(())
}

/// Register interrupt handler with system controller
pub fn register_system_handler(irq: u32, handler: InterruptHandler) -> Result<(), DriverError> {
    unsafe {
        if let Some(ref mut gic) = INTERRUPT_CONTROLLER {
            gic.register_handler(irq, handler)?;
            gic.enable_interrupt(irq)?;
        } else {
            return Err(DriverError::NotInitialized);
        }
    }
    Ok(())
}

/// Handle system interrupt (called from interrupt vector)
pub fn handle_system_interrupt() {
    unsafe {
        if let Some(ref gic) = INTERRUPT_CONTROLLER {
            gic.handle_interrupt();
        }
    }
}
