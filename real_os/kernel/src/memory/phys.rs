//! Physical memory management


const PAGE_SIZE: usize = 4096;
const MAX_PAGES: usize = 1024 * 256; // 1GB of physical memory

/// Simple bitmap allocator for physical pages
struct PhysicalMemoryManager {
    bitmap: [u64; MAX_PAGES / 64],
    free_pages: usize,
    total_pages: usize,
}

static mut PMM: PhysicalMemoryManager = PhysicalMemoryManager {
    bitmap: [0; MAX_PAGES / 64],
    free_pages: 0,
    total_pages: 0,
};

impl PhysicalMemoryManager {
    /// Mark a page as used
    fn mark_used(&mut self, page_num: usize) {
        let idx = page_num / 64;
        let bit = page_num % 64;
        self.bitmap[idx] |= 1 << bit;
        self.free_pages -= 1;
    }
    
    /// Mark a page as free
    fn mark_free(&mut self, page_num: usize) {
        let idx = page_num / 64;
        let bit = page_num % 64;
        self.bitmap[idx] &= !(1 << bit);
        self.free_pages += 1;
    }
    
    /// Find and allocate a free page
    fn alloc_page(&mut self) -> Option<usize> {
        for (idx, &bits) in self.bitmap.iter().enumerate() {
            if bits != u64::MAX {
                // Found a free page in this bitmap entry
                for bit in 0..64 {
                    if bits & (1 << bit) == 0 {
                        let page_num = idx * 64 + bit;
                        if page_num < self.total_pages {
                            self.mark_used(page_num);
                            return Some(page_num * PAGE_SIZE);
                        }
                    }
                }
            }
        }
        None
    }
    
    /// Free a page
    fn free_page(&mut self, addr: usize) {
        let page_num = addr / PAGE_SIZE;
        if page_num < self.total_pages {
            self.mark_free(page_num);
        }
    }
}

pub fn init() {
    unsafe {
        // Assume we have 256MB of RAM starting at 0x40000000 (QEMU default)
        let _ram_start = 0x40000000;
        let ram_size = 256 * 1024 * 1024;
        
        PMM.total_pages = ram_size / PAGE_SIZE;
        PMM.free_pages = PMM.total_pages;
        
        // Mark kernel pages as used (first 16MB)
        let kernel_pages = (16 * 1024 * 1024) / PAGE_SIZE;
        for i in 0..kernel_pages {
            PMM.mark_used(i);
        }
    }
}

pub fn alloc_page() -> Option<usize> {
    unsafe { PMM.alloc_page() }
}

pub fn free_page(addr: usize) {
    unsafe { PMM.free_page(addr) }
}