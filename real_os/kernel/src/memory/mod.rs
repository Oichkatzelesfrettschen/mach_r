//! Memory management subsystem

pub mod phys;
pub mod virt;
pub mod heap;

use core::alloc::{GlobalAlloc, Layout};

/// Allocate a stack of given size
pub fn alloc_stack(size: usize) -> usize {
    // Align to page boundary
    let size = (size + 4095) & !4095;
    
    // Allocate physical pages
    let mut addr = 0;
    for _ in 0..(size / 4096) {
        if let Some(page) = phys::alloc_page() {
            if addr == 0 {
                addr = page;
            }
        } else {
            panic!("Out of memory allocating stack");
        }
    }
    
    addr + size  // Return top of stack (grows down)
}

/// Free a stack
pub fn free_stack(_stack_top: usize) {
    // For now, just leak it
    // TODO: Proper stack deallocation
}