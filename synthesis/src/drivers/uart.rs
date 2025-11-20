//! ARM PL011 UART driver implementation
//! Pure Rust driver for ARM PrimCell UART (PL011)

use super::{Driver, DriverError, PollStatus, DeviceDriver, DeviceInfo, DeviceAddress};
use alloc::boxed::Box;
use core::ptr::{read_volatile, write_volatile};

/// PL011 UART register offsets
const UART_DR: usize = 0x00;      // Data Register
#[allow(dead_code)]
const UART_RSR: usize = 0x04;     // Receive Status Register
const UART_FR: usize = 0x18;      // Flag Register
#[allow(dead_code)]
const UART_ILPR: usize = 0x20;    // IrDA Low-Power Counter Register
const UART_IBRD: usize = 0x24;    // Integer Baud Rate Divisor
const UART_FBRD: usize = 0x28;    // Fractional Baud Rate Divisor
const UART_LCR_H: usize = 0x2C;   // Line Control Register
const UART_CR: usize = 0x30;      // Control Register
#[allow(dead_code)]
const UART_IFLS: usize = 0x34;    // Interrupt FIFO Level Select Register
const UART_IMSC: usize = 0x38;    // Interrupt Mask Set/Clear Register
#[allow(dead_code)]
const UART_RIS: usize = 0x3C;     // Raw Interrupt Status Register
#[allow(dead_code)]
const UART_MIS: usize = 0x40;     // Masked Interrupt Status Register
const UART_ICR: usize = 0x44;     // Interrupt Clear Register

/// Flag Register bits
const FR_RXFE: u32 = 1 << 4;      // Receive FIFO Empty
const FR_TXFF: u32 = 1 << 5;      // Transmit FIFO Full
#[allow(dead_code)]
const FR_RXFF: u32 = 1 << 6;      // Receive FIFO Full
const FR_TXFE: u32 = 1 << 7;      // Transmit FIFO Empty
const FR_BUSY: u32 = 1 << 3;      // UART Busy

/// Control Register bits
const CR_UARTEN: u32 = 1 << 0;    // UART Enable
#[allow(dead_code)]
const CR_LBE: u32 = 1 << 7;       // Loopback Enable
const CR_TXE: u32 = 1 << 8;       // Transmit Enable
const CR_RXE: u32 = 1 << 9;       // Receive Enable

/// Line Control Register bits
const LCR_H_FEN: u32 = 1 << 4;    // FIFO Enable
const LCR_H_WLEN_8: u32 = 3 << 5; // 8-bit word length

/// PL011 UART driver
pub struct Pl011Driver {
    base_addr: usize,
    initialized: bool,
    baud_rate: u32,
    clock_freq: u32,
}

impl Pl011Driver {
    /// Create new PL011 driver
    pub fn new() -> Self {
        Self {
            base_addr: 0x09000000, // Default base address for ARM Virt
            initialized: false,
            baud_rate: 115200,
            clock_freq: 24_000_000, // 24MHz clock
        }
    }
    
    /// Create PL011 driver with custom base address
    pub fn new_with_base(base_addr: usize) -> Self {
        Self {
            base_addr,
            initialized: false,
            baud_rate: 115200,
            clock_freq: 24_000_000,
        }
    }
    
    /// Set baud rate
    pub fn set_baud_rate(&mut self, baud_rate: u32) {
        self.baud_rate = baud_rate;
        if self.initialized {
            self.configure_baud_rate();
        }
    }
    
    /// Set clock frequency
    pub fn set_clock_freq(&mut self, freq: u32) {
        self.clock_freq = freq;
        if self.initialized {
            self.configure_baud_rate();
        }
    }
    
    /// Read register
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe {
            read_volatile((self.base_addr + offset) as *const u32)
        }
    }
    
    /// Write register
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe {
            write_volatile((self.base_addr + offset) as *mut u32, value);
        }
    }
    
    /// Configure baud rate
    fn configure_baud_rate(&self) {
        // Calculate baud rate divisor
        // UART_CLK / (16 * baud_rate) = divisor
        let divisor = (self.clock_freq + (8 * self.baud_rate)) / (16 * self.baud_rate);
        let ibrd = divisor;
        let fbrd = ((64 * self.clock_freq + (8 * 16 * self.baud_rate)) / (16 * self.baud_rate)) - (64 * ibrd);
        
        self.write_reg(UART_IBRD, ibrd);
        self.write_reg(UART_FBRD, fbrd & 0x3F);
    }
    
    /// Wait for transmit FIFO to have space
    fn wait_tx_ready(&self) {
        while (self.read_reg(UART_FR) & FR_TXFF) != 0 {
            core::hint::spin_loop();
        }
    }
    
    /// Check if receive FIFO has data
    fn has_rx_data(&self) -> bool {
        (self.read_reg(UART_FR) & FR_RXFE) == 0
    }
    
    /// Send a single byte
    pub fn send_byte(&self, byte: u8) {
        if !self.initialized {
            return;
        }
        
        self.wait_tx_ready();
        self.write_reg(UART_DR, byte as u32);
    }
    
    /// Receive a single byte
    pub fn recv_byte(&self) -> Option<u8> {
        if !self.initialized || !self.has_rx_data() {
            return None;
        }
        
        let data = self.read_reg(UART_DR);
        Some((data & 0xFF) as u8)
    }
    
    /// Send string
    pub fn send_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send_byte(b'\r'); // Add CR for line endings
            }
            self.send_byte(byte);
        }
    }
    
    /// Flush transmit FIFO
    pub fn flush(&self) {
        if !self.initialized {
            return;
        }
        
        // Wait for FIFO to be empty and UART not busy
        while (self.read_reg(UART_FR) & (FR_TXFE | FR_BUSY)) != FR_TXFE {
            core::hint::spin_loop();
        }
    }
}

impl Driver for Pl011Driver {
    fn name(&self) -> &str {
        "pl011-uart"
    }
    
    fn init(&mut self) -> Result<(), DriverError> {
        if self.initialized {
            return Ok(());
        }
        
        // Disable UART
        self.write_reg(UART_CR, 0);
        
        // Clear any pending interrupts
        self.write_reg(UART_ICR, 0x7FF);
        
        // Configure baud rate
        self.configure_baud_rate();
        
        // Configure line control: 8N1, FIFO enabled
        self.write_reg(UART_LCR_H, LCR_H_WLEN_8 | LCR_H_FEN);
        
        // Configure interrupts (disable all for polling mode)
        self.write_reg(UART_IMSC, 0);
        
        // Enable UART, TX, and RX
        self.write_reg(UART_CR, CR_UARTEN | CR_TXE | CR_RXE);
        
        self.initialized = true;
        
        // Test the UART
        self.send_str("PL011 UART initialized\n");
        
        Ok(())
    }
    
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        let mut bytes_read = 0;
        
        for i in 0..buffer.len() {
            if let Some(byte) = self.recv_byte() {
                buffer[i] = byte;
                bytes_read += 1;
            } else {
                break;
            }
        }
        
        Ok(bytes_read)
    }
    
    fn write(&mut self, buffer: &[u8]) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        for &byte in buffer {
            self.send_byte(byte);
        }
        
        Ok(buffer.len())
    }
    
    fn control(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        if !self.initialized {
            return Err(DriverError::NotInitialized);
        }
        
        match cmd {
            0 => {
                // Flush
                self.flush();
                Ok(0)
            }
            1 => {
                // Set baud rate
                self.set_baud_rate(arg as u32);
                Ok(0)
            }
            2 => {
                // Set clock frequency
                self.set_clock_freq(arg as u32);
                Ok(0)
            }
            _ => Err(DriverError::NotSupported),
        }
    }
    
    fn poll(&self) -> PollStatus {
        if !self.initialized {
            return PollStatus {
                can_read: false,
                can_write: false,
                has_error: false,
            };
        }
        
        let fr = self.read_reg(UART_FR);
        
        PollStatus {
            can_read: (fr & FR_RXFE) == 0,
            can_write: (fr & FR_TXFF) == 0,
            has_error: false, // TODO: Check error flags
        }
    }
}

impl DeviceDriver for Pl011Driver {
    fn name(&self) -> &str {
        "pl011-uart"
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
        
        // Flush and disable UART
        self.flush();
        self.write_reg(UART_CR, 0);
        self.initialized = false;
        
        Ok(())
    }
    
    fn ioctl(&mut self, cmd: u32, arg: usize) -> Result<usize, DriverError> {
        self.control(cmd, arg)
            .map_err(|_| DriverError::HardwareError)
    }
    
    fn can_handle(&self, device: &DeviceInfo) -> bool {
        // Check if device is compatible with PL011
        if let Ok(compat_key) = heapless::String::try_from("compatible") {
            if let Some(compatible) = device.properties.get(&compat_key) {
                return compatible == "arm,pl011" || compatible.starts_with("arm,pl011");
            }
        }
        false
    }
    
    fn bind(&mut self, device: &DeviceInfo) -> Result<(), DriverError> {
        // Extract base address from device info
        if let Some(addr) = device.addresses.get(0) {
            if let DeviceAddress::Mmio { base, size: _ } = addr {
                self.base_addr = *base as usize;
            }
        }
        
        Driver::init(self)
    }
    
    fn unbind(&mut self, _device: &DeviceInfo) -> Result<(), DriverError> {
        self.shutdown()
    }
}

/// Initialize PL011 UART driver
pub fn init() -> Result<(), DriverError> {
    let mut uart = Pl011Driver::new();
    Driver::init(&mut uart)?;
    
    // Register with device manager if available
    if let Some(manager) = crate::drivers::device_manager_mut() {
        let _ = manager.register_driver(Box::new(uart));
    }
    
    Ok(())
}

/// Send formatted string to UART
#[macro_export]
macro_rules! uart_print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut uart = crate::drivers::uart::Pl011Driver::new();
        if uart.init().is_ok() {
            uart.send_str(&alloc::format!($($arg)*));
        }
    }};
}

/// Send formatted string with newline to UART
#[macro_export]
macro_rules! uart_println {
    () => (uart_print!("\n"));
    ($($arg:tt)*) => (uart_print!("{}\n", core::format_args!($($arg)*)));
}

/// Global UART instance for console output
static mut CONSOLE_UART: Option<Pl011Driver> = None;

/// Initialize console UART
pub fn init_console() -> Result<(), DriverError> {
    unsafe {
        let mut uart = Pl011Driver::new();
        Driver::init(&mut uart)?;
        CONSOLE_UART = Some(uart);
    }
    Ok(())
}

/// Write to console UART
pub fn console_write(s: &str) {
    unsafe {
        if let Some(ref uart) = CONSOLE_UART {
            uart.send_str(s);
        }
    }
}

/// Write byte to console UART
pub fn console_write_byte(byte: u8) {
    unsafe {
        if let Some(ref uart) = CONSOLE_UART {
            uart.send_byte(byte);
        }
    }
}