//! Console output driver

use core::fmt;

// QEMU virt machine UART base
const UART_BASE: usize = 0x0900_0000;

struct Uart {
    base: usize,
}

impl Uart {
    const fn new(base: usize) -> Self {
        Self { base }
    }
    
    unsafe fn putc(&self, c: u8) {
        let ptr = self.base as *mut u8;
        ptr.write_volatile(c);
    }
    
    fn puts(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                unsafe { self.putc(b'\r'); }
            }
            unsafe { self.putc(byte); }
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.puts(s);
        Ok(())
    }
}

static UART: Uart = Uart::new(UART_BASE);

pub fn init() {
    // UART is memory-mapped, no init needed for QEMU
}

pub fn print(s: &str) {
    UART.puts(s);
}

pub fn println(s: &str) {
    print(s);
    print("\n");
}

// Formatting support
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    let mut uart = Uart::new(UART_BASE);
    let _ = uart.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}