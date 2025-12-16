//! Memory map management and utilities
//! Physical memory layout detection and management

use crate::boot::{MemoryMapEntry, MemoryType};

/// Memory map manager
pub struct MemoryMapManager {
    entries: &'static [MemoryMapEntry],
    total_memory: u64,
    usable_memory: u64,
}

impl MemoryMapManager {
    /// Create a new memory map manager
    pub fn new(entries: &'static [MemoryMapEntry]) -> Self {
        let mut total = 0;
        let mut usable = 0;

        for entry in entries {
            total += entry.size;
            if entry.mem_type == MemoryType::Available {
                usable += entry.size;
            }
        }

        Self {
            entries,
            total_memory: total,
            usable_memory: usable,
        }
    }

    /// Get total physical memory size
    pub fn total_memory(&self) -> u64 {
        self.total_memory
    }

    /// Get usable physical memory size
    pub fn usable_memory(&self) -> u64 {
        self.usable_memory
    }

    /// Get memory entries
    pub fn entries(&self) -> &[MemoryMapEntry] {
        self.entries
    }

    /// Find largest available memory region
    pub fn find_largest_available_region(&self) -> Option<&MemoryMapEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.mem_type == MemoryType::Available)
            .max_by_key(|entry| entry.size)
    }

    /// Find available memory region of at least specified size
    pub fn find_available_region(&self, min_size: u64, alignment: u64) -> Option<u64> {
        for entry in self.entries {
            if entry.mem_type != MemoryType::Available {
                continue;
            }

            // Align start address
            let aligned_start = (entry.start_addr + alignment - 1) & !(alignment - 1);

            // Check if aligned region fits
            if aligned_start >= entry.start_addr + entry.size {
                continue;
            }

            let available_size = entry.start_addr + entry.size - aligned_start;
            if available_size >= min_size {
                return Some(aligned_start);
            }
        }

        None
    }

    /// Check if address range is available
    pub fn is_range_available(&self, start_addr: u64, size: u64) -> bool {
        let end_addr = start_addr + size;

        for entry in self.entries {
            if entry.mem_type != MemoryType::Available {
                continue;
            }

            let entry_end = entry.start_addr + entry.size;

            // Check if range is completely within this available region
            if start_addr >= entry.start_addr && end_addr <= entry_end {
                return true;
            }
        }

        false
    }

    /// Get memory type for a given address
    pub fn get_memory_type(&self, addr: u64) -> MemoryType {
        for entry in self.entries {
            if addr >= entry.start_addr && addr < entry.start_addr + entry.size {
                return entry.mem_type;
            }
        }

        MemoryType::Reserved // Default to reserved if not found
    }

    /// Print memory map for debugging
    pub fn print_memory_map(&self) {
        crate::boot::serial_println("Memory Map:");
        crate::boot::serial_println("============");

        for (i, entry) in self.entries.iter().enumerate() {
            let type_str = match entry.mem_type {
                MemoryType::Available => "Available",
                MemoryType::Reserved => "Reserved",
                MemoryType::AcpiReclaimable => "ACPI Reclaimable",
                MemoryType::AcpiNvs => "ACPI NVS",
                MemoryType::BadMemory => "Bad Memory",
                MemoryType::Bootloader => "Bootloader",
                MemoryType::Kernel => "Kernel",
                MemoryType::Device => "Device",
                MemoryType::Firmware => "Firmware",
            };

            // Format addresses and sizes in a simple way
            crate::boot::serial_println(&format_memory_entry(i, entry, type_str));
        }

        crate::boot::serial_println("");
        crate::boot::serial_println(&format_memory_summary(
            self.total_memory,
            self.usable_memory,
        ));
    }

    /// Validate memory map consistency
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.entries.is_empty() {
            return Err("Empty memory map");
        }

        // Check for overlapping regions
        for i in 0..self.entries.len() {
            for j in i + 1..self.entries.len() {
                let entry1 = &self.entries[i];
                let entry2 = &self.entries[j];

                let end1 = entry1.start_addr + entry1.size;
                let end2 = entry2.start_addr + entry2.size;

                // Check for overlap
                if (entry1.start_addr < end2) && (entry2.start_addr < end1) {
                    return Err("Overlapping memory regions detected");
                }
            }
        }

        Ok(())
    }

    /// Get memory statistics
    pub fn get_stats(&self) -> MemoryStats {
        let mut stats = MemoryStats::default();

        for entry in self.entries {
            match entry.mem_type {
                MemoryType::Available => stats.available += entry.size,
                MemoryType::Reserved => stats.reserved += entry.size,
                MemoryType::AcpiReclaimable => stats.acpi_reclaimable += entry.size,
                MemoryType::AcpiNvs => stats.acpi_nvs += entry.size,
                MemoryType::BadMemory => stats.bad_memory += entry.size,
                MemoryType::Bootloader => stats.bootloader += entry.size,
                MemoryType::Kernel => stats.kernel += entry.size,
                MemoryType::Device => stats.device += entry.size,
                MemoryType::Firmware => stats.reserved += entry.size, // Firmware is part of reserved
            }
        }

        stats.total = self.total_memory;
        stats
    }
}

/// Memory statistics
#[derive(Default)]
pub struct MemoryStats {
    pub total: u64,
    pub available: u64,
    pub reserved: u64,
    pub acpi_reclaimable: u64,
    pub acpi_nvs: u64,
    pub bad_memory: u64,
    pub bootloader: u64,
    pub kernel: u64,
    pub device: u64,
}

/// Format memory entry for display
fn format_memory_entry(
    index: usize,
    entry: &MemoryMapEntry,
    type_str: &str,
) -> heapless::String<128> {
    let mut result = heapless::String::new();

    // Simple formatting without format! macro
    result.push_str(&format_number(index)).unwrap();
    result.push_str(": 0x").unwrap();
    result.push_str(&format_hex(entry.start_addr)).unwrap();
    result.push_str(" - 0x").unwrap();
    result
        .push_str(&format_hex(entry.start_addr + entry.size - 1))
        .unwrap();
    result.push_str(" (").unwrap();
    result.push_str(&format_size(entry.size)).unwrap();
    result.push_str(") ").unwrap();
    result.push_str(type_str).unwrap();

    result
}

/// Format memory summary
fn format_memory_summary(total: u64, usable: u64) -> heapless::String<128> {
    let mut result = heapless::String::new();

    result.push_str("Total: ").unwrap();
    result.push_str(&format_size(total)).unwrap();
    result.push_str(", Usable: ").unwrap();
    result.push_str(&format_size(usable)).unwrap();

    result
}

/// Simple number formatting
fn format_number(n: usize) -> heapless::String<16> {
    let mut result = heapless::String::new();
    let mut num = n;

    if num == 0 {
        result.push('0').unwrap();
        return result;
    }

    let mut digits = [0u8; 16];
    let mut count = 0;

    while num > 0 && count < digits.len() {
        digits[count] = (num % 10) as u8 + b'0';
        num /= 10;
        count += 1;
    }

    for i in (0..count).rev() {
        result.push(digits[i] as char).unwrap();
    }

    result
}

/// Simple hex formatting (lower 32 bits)
fn format_hex(n: u64) -> heapless::String<16> {
    let mut result = heapless::String::new();
    let num = n as u32; // Simplified to 32-bit for display

    let hex_chars = b"0123456789abcdef";

    for i in (0..8).rev() {
        let nibble = ((num >> (i * 4)) & 0xf) as usize;
        result.push(hex_chars[nibble] as char).unwrap();
    }

    result
}

/// Simple size formatting
fn format_size(size: u64) -> heapless::String<16> {
    let mut result = heapless::String::new();

    if size >= 1024 * 1024 * 1024 {
        let gb = (size / (1024 * 1024 * 1024)) as usize;
        result.push_str(&format_number(gb)).unwrap();
        result.push_str("GB").unwrap();
    } else if size >= 1024 * 1024 {
        let mb = (size / (1024 * 1024)) as usize;
        result.push_str(&format_number(mb)).unwrap();
        result.push_str("MB").unwrap();
    } else if size >= 1024 {
        let kb = (size / 1024) as usize;
        result.push_str(&format_number(kb)).unwrap();
        result.push_str("KB").unwrap();
    } else {
        result.push_str(&format_number(size as usize)).unwrap();
        result.push_str("B").unwrap();
    }

    result
}

/// Default memory layout for ARM64 systems
pub const DEFAULT_ARM64_MEMORY_MAP: &[MemoryMapEntry] = &[
    // Low memory - reserved for firmware
    MemoryMapEntry {
        mem_type: MemoryType::Reserved,
        start_addr: 0x00000000,
        size: 0x00080000, // 512KB
    },
    // Kernel load area
    MemoryMapEntry {
        mem_type: MemoryType::Kernel,
        start_addr: 0x00080000,
        size: 0x00080000, // 512KB for kernel
    },
    // Main RAM - available for use
    MemoryMapEntry {
        mem_type: MemoryType::Available,
        start_addr: 0x00100000,
        size: 0x3FF00000, // ~1GB - 1MB
    },
    // Device memory region
    MemoryMapEntry {
        mem_type: MemoryType::Device,
        start_addr: 0x40000000,
        size: 0x40000000, // 1GB device space
    },
];

/// Create a default memory map manager for testing
pub fn create_default_memory_map() -> MemoryMapManager {
    MemoryMapManager::new(DEFAULT_ARM64_MEMORY_MAP)
}
