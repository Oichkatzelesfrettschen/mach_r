//! Kernel heap allocator

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

const HEAP_SIZE: usize = 1024 * 1024; // 1MB heap
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut HEAP_POS: usize = 0;

struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        
        // Align the position
        let aligned_pos = (HEAP_POS + align - 1) & !(align - 1);
        
        if aligned_pos + size > HEAP_SIZE {
            return ptr::null_mut();
        }
        
        let ptr = HEAP.as_mut_ptr().add(aligned_pos);
        HEAP_POS = aligned_pos + size;
        ptr
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Simple bump allocator - no deallocation
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;

pub fn init() {
    // Heap is ready to use
}