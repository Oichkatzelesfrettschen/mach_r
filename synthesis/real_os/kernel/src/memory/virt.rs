//! Virtual memory management

/// Page table structure
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    pub fn new() -> Self {
        Self {
            entries: [PageTableEntry(0); 512],
        }
    }
}

#[derive(Copy, Clone)]
#[repr(transparent)]
struct PageTableEntry(u64);

pub fn init() {
    // TODO: Set up page tables
    // For now, we rely on identity mapping from bootloader
}