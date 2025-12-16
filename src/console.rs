//! Console output for Mach_R kernel
//!
//! Provides basic text output functionality for kernel debugging and status messages.
//! In a real implementation, this would interface with UART, VGA, or framebuffer.

use core::fmt::{self, Write};
use spin::Mutex;

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
}

/// Console writer interface
pub struct Console {
    // In a real kernel, this would contain hardware-specific state
    // For now, we'll use a simple buffer for testing
    #[cfg(test)]
    buffer: heapless::String<1024>,
}

impl Console {
    /// Create a new console instance
    pub const fn new() -> Self {
        Console {
            #[cfg(test)]
            buffer: heapless::String::new(),
        }
    }

    /// Write a byte to the console
    pub fn write_byte(&mut self, byte: u8) {
        #[cfg(test)]
        {
            // In test mode, append to buffer
            if byte.is_ascii() {
                let _ = self.buffer.push(byte as char);
            }
        }

        #[cfg(not(test))]
        {
            // In kernel mode, write to UART hardware
            unsafe {
                Uart::new(UART_BASE).putc(byte);
            }
        }
    }

    /// Write a string to the console
    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                // Handle newlines for serial output
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }

    /// Clear the console
    pub fn clear(&mut self) {
        #[cfg(test)]
        self.buffer.clear();

        #[cfg(not(test))]
        {
            // Clear screen implementation (platform-specific, not implemented for serial)
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// Global console instance
static CONSOLE: Mutex<Console> = Mutex::new(Console::new());

/// Initialize the console subsystem
pub fn init() {
    // Platform-specific initialization
    // Set up UART, VGA, or framebuffer
    CONSOLE.lock().clear();
}

/// Print formatted text to console
pub fn print(args: fmt::Arguments) {
    CONSOLE.lock().write_fmt(args).unwrap();
}

/// Print macro for kernel use
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(format_args!($($arg)*));
    };
}

/// Print with newline macro
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n");
    };
    ($($arg:tt)*) => {
        $crate::console::print(format_args!("{}\n", format_args!($($arg)*)))
    };
}

// Macros are already exported via #[macro_export]
// No need to re-export them

/// Process keyboard input (called from interrupt handler)
pub fn process_keyboard(scancode: u8) {
    // Simple scancode to ASCII conversion (US layout)
    let ascii = match scancode {
        0x1E => b'a',
        0x30 => b'b',
        0x2E => b'c',
        0x20 => b'd',
        0x12 => b'e',
        0x21 => b'f',
        0x22 => b'g',
        0x23 => b'h',
        0x17 => b'i',
        0x24 => b'j',
        0x25 => b'k',
        0x26 => b'l',
        0x32 => b'm',
        0x31 => b'n',
        0x18 => b'o',
        0x19 => b'p',
        0x10 => b'q',
        0x13 => b'r',
        0x1F => b's',
        0x14 => b't',
        0x16 => b'u',
        0x2F => b'v',
        0x11 => b'w',
        0x2D => b'x',
        0x15 => b'y',
        0x2C => b'z',
        0x39 => b' ',  // Space
        0x1C => b'\n', // Enter
        0x0E => 0x08,  // Backspace
        _ => 0,
    };

    if ascii != 0 {
        // In real implementation, would add to keyboard buffer
        // For now, just echo to console
        CONSOLE.lock().write_byte(ascii);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_write() {
        let mut console = Console::new();
        console.write_str("Hello, Mach_R!");
        assert!(console.buffer.contains("Hello, Mach_R!"));
    }

    #[test]
    fn test_console_formatting() {
        let mut console = Console::new();
        write!(&mut console, "Test {}", 42).unwrap();
        assert!(console.buffer.contains("Test 42"));
    }
}
