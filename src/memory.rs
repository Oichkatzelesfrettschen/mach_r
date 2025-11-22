//! Memory management for Mach_R
//!
//! Provides basic memory allocation and virtual memory management.
//! This is a simplified version - a real implementation would include
//! external pagers, copy-on-write, and more sophisticated VM operations.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use spin::Mutex;
// Types are re-exported below

/// Simple bump allocator for early kernel bootstrap
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
}

impl BumpAllocator {
    /// Create a new bump allocator
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
        }
    }
    
    /// Initialize with heap bounds
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
    
    /// Allocate memory
    pub fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let alloc_start = align_up(self.next, layout.align());
        let alloc_end = alloc_start + layout.size();
        
        if alloc_end > self.heap_end {
            // Out of memory
            return null_mut();
        }
        
        self.next = alloc_end;
        alloc_start as *mut u8
    }
}

/// Align address upward to alignment
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Global allocator instance
pub struct GlobalAllocator {
    allocator: Mutex<BumpAllocator>,
}

impl GlobalAllocator {
    /// Create a new global allocator
    pub const fn new() -> Self {
        GlobalAllocator {
            allocator: Mutex::new(BumpAllocator::new()),
        }
    }
    
    /// Initialize the allocator
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.allocator.lock().init(heap_start, heap_size);
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocator.lock().allocate(layout)
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't support deallocation
        // A real implementation would use a more sophisticated allocator
    }
}

/// The global allocator instance
#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

// For tests, use the system allocator
#[cfg(test)]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

/// Initialize memory management
pub fn init() {
    // In a real kernel, we would:
    // 1. Detect available memory from bootloader
    // 2. Set up page tables
    // 3. Initialize the heap
    
    unsafe {
        #[cfg(not(test))]
        {
            let heap_start = 0x200000; // 2MB mark
            let heap_size = 0x100000;  // 1MB heap
            ALLOCATOR.init(heap_start, heap_size);
        }
        
        #[cfg(test)]
        {
            // For tests, use a static buffer
            static mut HEAP: [u8; 65536] = [0; 65536];
            let heap_start = HEAP.as_ptr() as usize;
            let heap_size = HEAP.len();
            ALLOCATOR.init(heap_start, heap_size);
        }
    }
}

/// Virtual memory operations (placeholder)
pub mod vm {
    /// VM map structure (simplified)
    pub struct VmMap {
        /// Start address of the VM map
        pub start: usize,
        /// End address of the VM map
        pub end: usize,
    }
    
    impl VmMap {
        /// Create a new VM map
        pub fn new(start: usize, size: usize) -> Self {
            VmMap {
                start,
                end: start + size,
            }
        }
    }
}

/// Page frame manager for physical memory
pub struct PageManager {
    free_pages: Mutex<alloc::vec::Vec<crate::paging::PhysicalAddress>>,
}

impl PageManager {
    /// Create a new page manager
    pub const fn new() -> Self {
        Self {
            free_pages: Mutex::new(alloc::vec::Vec::new()),
        }
    }
    
    /// Allocate a physical page
    pub fn allocate_page(&self) -> Result<crate::paging::PhysicalAddress, ()> {
        let mut pages = self.free_pages.lock();
        if let Some(page) = pages.pop() {
            Ok(page)
        } else {
            // Allocate a new page from the heap area
            Ok(crate::paging::PhysicalAddress(0x300000)) // Use fixed address for now
        }
    }
    
    /// Deallocate a physical page
    pub fn deallocate_page(&self, page: crate::paging::PhysicalAddress) {
        let mut pages = self.free_pages.lock();
        pages.push(page);
    }
    
    /// Add a free page to the manager
    pub fn add_free_page(&self, page: crate::paging::PhysicalAddress) {
        let mut pages = self.free_pages.lock();
        pages.push(page);
    }
}

static PAGE_MANAGER: PageManager = PageManager::new();

/// Get the global page manager instance
pub fn page_manager() -> &'static PageManager {
    &PAGE_MANAGER
}

// Re-export types for convenience
pub use crate::paging::{VirtualAddress, PhysicalAddress};

/// Allocate a stack of given size
pub fn alloc_stack(size: usize) -> usize {
    // Align to page boundary
    let size = (size + crate::paging::PAGE_SIZE - 1) & !(crate::paging::PAGE_SIZE - 1);
    
    // Allocate physical pages
    let mut addr = 0;
    for _ in 0..(size / crate::paging::PAGE_SIZE) {
        if let Ok(page) = page_manager().allocate_page() {
            if addr == 0 {
                addr = page.0;
            }
        } else {
            panic!("Out of memory allocating stack");
        }
    }
    
    addr + size  // Return top of stack (grows down)
}

/// Free a stack
pub fn free_stack(stack_top: usize, size: usize) {
    // Align to page boundary
    let size = (size + crate::paging::PAGE_SIZE - 1) & !(crate::paging::PAGE_SIZE - 1);
    let start_addr = stack_top - size;

    for i in 0..(size / crate::paging::PAGE_SIZE) {
        page_manager().deallocate_page(crate::paging::PhysicalAddress(start_addr + i * crate::paging::PAGE_SIZE));
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 4), 8);
    }
    
    #[test]
    fn test_bump_allocator() {
        let mut allocator = BumpAllocator::new();
        unsafe {
            allocator.init(0x1000, 0x1000);
        }
        
        let layout = Layout::from_size_align(16, 8).unwrap();
        let ptr = allocator.allocate(layout);
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize, 0x1000);
    }
}