//! Serial port driver for Mach_R
//!
//! Supports both ARM64 PL011 UART and x86_64 16550 UART

use core::fmt;
use spin::Mutex;
use alloc::boxed::Box;

/// Serial port abstraction
pub trait SerialPort: Send {
    /// Initialize the serial port
    fn init(&mut self);
    
    /// Write a byte to the serial port
    fn write_byte(&mut self, byte: u8);
    
    /// Read a byte from the serial port
    fn read_byte(&mut self) -> Option<u8>;
    
    /// Check if data is available
    fn has_data(&self) -> bool;
}

// ARM64 PL011 UART implementation
#[cfg(target_arch = "aarch64")]
pub mod pl011 {
    use super::*;
    
    /// PL011 UART registers
    #[repr(C)]
    struct Pl011Regs {
        dr: u32,      // 0x00 - Data register
        rsr: u32,     // 0x04 - Receive status register
        _pad0: [u32; 4],
        fr: u32,      // 0x18 - Flag register  
        _pad1: u32,
        ilpr: u32,    // 0x20 - IrDA low-power counter
        ibrd: u32,    // 0x24 - Integer baud rate
        fbrd: u32,    // 0x28 - Fractional baud rate
        lcr_h: u32,   // 0x2C - Line control
        cr: u32,      // 0x30 - Control register
        ifls: u32,    // 0x34 - Interrupt FIFO level
        imsc: u32,    // 0x38 - Interrupt mask
        ris: u32,     // 0x3C - Raw interrupt status
        mis: u32,     // 0x40 - Masked interrupt status
        icr: u32,     // 0x44 - Interrupt clear
        dmacr: u32,   // 0x48 - DMA control
    }
    
    /// PL011 UART driver
    pub struct Pl011Uart {
        base_addr: usize,
    }
    
    impl Pl011Uart {
        /// Create a new PL011 UART at the given base address
        pub const fn new(base_addr: usize) -> Self {
            Pl011Uart { base_addr }
        }
        
        fn regs(&self) -> &'static mut Pl011Regs {
            unsafe { &mut *(self.base_addr as *mut Pl011Regs) }
        }
    }
    
    impl SerialPort for Pl011Uart {
        fn init(&mut self) {
            let regs = self.regs();
            
            // Disable UART
            regs.cr = 0;
            
            // Set baud rate to 115200
            // Assuming 48MHz clock
            regs.ibrd = 26;  // 48000000 / (16 * 115200) = 26.042
            regs.fbrd = 3;   // 0.042 * 64 = 2.688 ≈ 3
            
            // Enable FIFO, 8-bit data, no parity, 1 stop bit
            regs.lcr_h = (1 << 4) | (3 << 5);
            
            // Enable UART, TX, and RX
            regs.cr = (1 << 0) | (1 << 8) | (1 << 9);
        }
        
        fn write_byte(&mut self, byte: u8) {
            let regs = self.regs();
            
            // Wait until TX FIFO not full
            while (regs.fr & (1 << 5)) != 0 {
                core::hint::spin_loop();
            }
            
            // Write byte
            regs.dr = byte as u32;
        }
        
        fn read_byte(&mut self) -> Option<u8> {
            let regs = self.regs();
            
            // Check if RX FIFO empty
            if (regs.fr & (1 << 4)) != 0 {
                None
            } else {
                Some((regs.dr & 0xFF) as u8)
            }
        }
        
        fn has_data(&self) -> bool {
            let regs = self.regs();
            (regs.fr & (1 << 4)) == 0
        }
    }
}

// x86_64 16550 UART implementation
#[cfg(target_arch = "x86_64")]
pub mod uart16550 {
    use super::*;
    
    /// 16550 UART I/O ports
    const UART_DATA: u16 = 0;
    const UART_IER: u16 = 1;     // Interrupt enable
    const UART_FCR: u16 = 2;     // FIFO control
    const UART_LCR: u16 = 3;     // Line control
    const UART_MCR: u16 = 4;     // Modem control
    const UART_LSR: u16 = 5;     // Line status
    #[allow(dead_code)]
    const UART_MSR: u16 = 6;     // Modem status
    const UART_DLL: u16 = 0;     // Divisor latch low (when DLAB=1)
    const UART_DLH: u16 = 1;     // Divisor latch high (when DLAB=1)
    
    /// 16550 UART driver
    pub struct Uart16550 {
        base_port: u16,
    }
    
    impl Uart16550 {
        /// Create a new 16550 UART at the given I/O port
        pub const fn new(base_port: u16) -> Self {
            Uart16550 { base_port }
        }
        
        fn read_reg(&self, reg: u16) -> u8 {
            crate::arch::x86_64::X86_64::inb(self.base_port + reg)
        }
        
        fn write_reg(&self, reg: u16, value: u8) {
            crate::arch::x86_64::X86_64::outb(self.base_port + reg, value);
        }
    }
    
    impl SerialPort for Uart16550 {
        fn init(&mut self) {
            // Disable interrupts
            self.write_reg(UART_IER, 0x00);
            
            // Enable DLAB (set baud rate)
            self.write_reg(UART_LCR, 0x80);
            
            // Set divisor to 1 (115200 baud)
            self.write_reg(UART_DLL, 0x01);
            self.write_reg(UART_DLH, 0x00);
            
            // 8 bits, no parity, 1 stop bit
            self.write_reg(UART_LCR, 0x03);
            
            // Enable FIFO, clear them, 14-byte threshold
            self.write_reg(UART_FCR, 0xC7);
            
            // Enable RTS/DSR
            self.write_reg(UART_MCR, 0x0B);
            
            // Enable interrupts
            self.write_reg(UART_IER, 0x01);
        }
        
        fn write_byte(&mut self, byte: u8) {
            // Wait for transmit buffer to be empty
            while (self.read_reg(UART_LSR) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            
            self.write_reg(UART_DATA, byte);
        }
        
        fn read_byte(&mut self) -> Option<u8> {
            if self.has_data() {
                Some(self.read_reg(UART_DATA))
            } else {
                None
            }
        }
        
        fn has_data(&self) -> bool {
            (self.read_reg(UART_LSR) & 0x01) != 0
        }
    }
}

/// Global serial port instance
static SERIAL: Mutex<Option<Box<dyn SerialPort>>> = Mutex::new(None);

/// Initialize the serial driver
pub fn init() {
    let mut serial = SERIAL.lock();
    
    #[cfg(target_arch = "aarch64")]
    {
        // QEMU virt machine UART at 0x09000000
        let mut uart = Box::new(pl011::Pl011Uart::new(0x0900_0000));
        uart.init();
        *serial = Some(uart as Box<dyn SerialPort>);
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        // COM1 at I/O port 0x3F8
        let mut uart = Box::new(uart16550::Uart16550::new(0x3F8));
        uart.init();
        *serial = Some(uart as Box<dyn SerialPort>);
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        // No serial support for other architectures yet
        *serial = None;
    }
}

/// Write a byte to the serial port
pub fn write_byte(byte: u8) {
    if let Some(ref mut port) = *SERIAL.lock() {
        port.write_byte(byte);
    }
}

/// Read a byte from the serial port
pub fn read_byte() -> Option<u8> {
    if let Some(ref mut port) = *SERIAL.lock() {
        port.read_byte()
    } else {
        None
    }
}

/// Serial writer for fmt::Write
pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            write_byte(byte);
        }
        Ok(())
    }
}

/// Print to serial console
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*))
    };
}

/// Print with newline to serial console
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    SerialWriter.write_fmt(args).unwrap();
}