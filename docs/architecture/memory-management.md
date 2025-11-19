# Memory Management Architecture

*In the spirit of Lions' Commentary: Understanding how Mach_R manages the machine's most precious resource*

## Introduction - Why Memory Management Matters

In a microkernel, memory management serves two masters:
1. **Isolation** - Each task must have its own protected address space
2. **Sharing** - Tasks must be able to share memory when needed (zero-copy IPC)

The Mach solution is elegant: *external pagers*. User-space servers manage memory policy while the kernel enforces protection.

## The Memory Hierarchy

```
┌──────────────────────────────────────────┐
│  Task's Virtual Address Space (64-bit)  │
│  ┌────────────────────────────────────┐ │
│  │  Stack    (grows down)             │ │
│  │             ↓                      │ │
│  │                                    │ │
│  │             ↑                      │ │
│  │  Heap     (grows up)               │ │
│  ├────────────────────────────────────┤ │
│  │  Data Segment                      │ │
│  ├────────────────────────────────────┤ │
│  │  Text Segment (code)               │ │
│  └────────────────────────────────────┘ │
└──────────────────────────────────────────┘
              ↓ Page Tables
┌──────────────────────────────────────────┐
│  Physical Memory                         │
│  ┌────────┬────────┬────────┬────────┐  │
│  │ Page 0 │ Page 1 │ Page 2 │ Page 3 │  │
│  └────────┴────────┴────────┴────────┘  │
└──────────────────────────────────────────┘
```

Understanding: A task sees a *virtual* address space (e.g., 0x0000_0000 to 0xFFFF_FFFF_FFFF_FFFF on 64-bit). The kernel maps these virtual addresses to *physical* memory pages through page tables.

## Virtual Memory Concepts

### Pages - The Unit of Memory

Everything in Mach_R operates on *pages*:

```
Page Size: 4096 bytes (4 KB) on most architectures

┌─────────────────────────────────────┐
│  Virtual Address (64-bit)           │
│  ┌───────────┬────────────────────┐ │
│  │  Page #   │  Offset (12 bits) │ │
│  └───────────┴────────────────────┘ │
│   52 bits     4096 possible offsets  │
└─────────────────────────────────────┘
```

Why pages?
- **Simplicity**: Allocate/free in fixed-size chunks
- **Protection**: Set permissions per page (read/write/execute)
- **Efficiency**: Hardware MMU works with pages

### Page Tables - The Translation Mechanism

On AArch64, we use 4-level page tables:

```
Virtual Address (48 bits used):

┌────┬────┬────┬────┬────────┐
│ L0 │ L1 │ L2 │ L3 │ Offset │
└────┴────┴────┴────┴────────┘
  9b   9b   9b   9b    12b

Each level indexes into a table:
- L0: 512 entries covering 512 GB each
- L1: 512 entries covering 1 GB each
- L2: 512 entries covering 2 MB each
- L3: 512 entries covering 4 KB each (one page)
```

The walk:
1. Start at L0 table (address in TTBR0/TTBR1 register)
2. Extract L0 index from virtual address
3. Follow pointer to L1 table
4. Extract L1 index, follow to L2 table
5. Extract L2 index, follow to L3 table
6. Extract L3 index, get physical page address
7. Add offset to get final physical address

The hardware does this automatically on every memory access.

## Memory Management in Mach_R

### Phase 1: Bootstrap Allocator (Current)

During early boot, before the full VM system initializes, we use a simple bump allocator:

```rust
/// Bump allocator for early boot.
///
/// This is the simplest possible allocator:
/// - Maintains a pointer to next free byte
/// - On allocation, returns current pointer and bumps it forward
/// - Never frees memory (acceptable during boot)
///
/// Limitations:
/// - No free() operation
/// - Wastes memory if allocation sizes vary
/// - Not suitable for general use
///
/// Used only until proper VM system is initialized.
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub fn allocate(&self, size: usize, align: usize) -> *mut u8 {
        // Align the next pointer
        let next = self.next.load(Ordering::Relaxed);
        let aligned = align_up(next, align);

        // Check if we have space
        let new_next = aligned + size;
        if new_next > self.heap_end {
            panic!("Out of memory in bump allocator");
        }

        // Update next pointer atomically
        self.next.store(new_next, Ordering::Relaxed);

        // Return allocated region
        aligned as *mut u8
    }
}
```

Why a bump allocator?
- **Simple**: Can't get much simpler than this
- **Fast**: Just an atomic increment
- **Sufficient**: During boot we don't free memory

Limitation: We can't reclaim memory. Once allocated, it's gone. This is acceptable for boot but not for a running system.

### Phase 2: Page Frame Allocator (Planned)

Once we have enough infrastructure, we'll implement a proper page allocator:

```rust
/// Physical page frame allocator.
///
/// Manages physical memory at page granularity (4 KB).
/// Uses a bitmap to track which pages are free/allocated.
///
/// Design:
/// - Bitmap: 1 bit per page (1 = allocated, 0 = free)
/// - For 4 GB RAM: 4GB / 4KB = 1M pages = 128 KB bitmap
/// - Fast allocation: Find first zero bit, set it
/// - Fast deallocation: Clear the bit
///
pub struct PageAllocator {
    /// Bitmap of page states (1 = allocated, 0 = free)
    bitmap: &'static mut [u8],

    /// Start of allocatable physical memory
    start_addr: PhysAddr,

    /// Total number of pages
    num_pages: usize,
}

impl PageAllocator {
    /// Allocates a single physical page.
    ///
    /// Returns: Physical address of the page, or None if no pages available.
    pub fn allocate_page(&mut self) -> Option<PhysAddr> {
        // Find first free page (bitmap bit = 0)
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            if *byte != 0xFF {  // Not all bits set
                // Find first zero bit in this byte
                for bit in 0..8 {
                    if (*byte & (1 << bit)) == 0 {
                        // Mark as allocated
                        *byte |= 1 << bit;

                        // Calculate physical address
                        let page_num = byte_idx * 8 + bit;
                        let addr = self.start_addr + (page_num * PAGE_SIZE);

                        return Some(addr);
                    }
                }
            }
        }

        None  // No free pages
    }

    /// Frees a physical page.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - The page was previously allocated
    /// - No references to this page exist
    pub unsafe fn free_page(&mut self, addr: PhysAddr) {
        let page_num = (addr - self.start_addr) / PAGE_SIZE;
        let byte_idx = page_num / 8;
        let bit = page_num % 8;

        // Clear the bit
        self.bitmap[byte_idx] &= !(1 << bit);
    }
}
```

Why a bitmap?
- **Space efficient**: 1 bit per page
- **Fast**: Bit operations are cheap
- **Simple**: Easy to understand and debug

### Phase 3: Virtual Memory Manager (Planned)

The VM manager maps virtual addresses to physical pages:

```rust
/// Virtual memory region.
///
/// Represents a contiguous range of virtual addresses
/// with uniform protection attributes.
///
/// Example: Task's stack might be one region:
/// - Start: 0x0000_7FFF_F000_0000
/// - End:   0x0000_7FFF_FFFF_FFFF  (16 MB)
/// - Protection: Read + Write
/// - Backed by: Anonymous memory (pager: default)
///
pub struct VmRegion {
    /// Virtual start address (page-aligned)
    start: VirtAddr,

    /// Size in bytes (multiple of PAGE_SIZE)
    size: usize,

    /// Protection bits
    protection: Protection,

    /// Object backing this region
    memory_object: Arc<dyn MemoryObject>,

    /// Offset into the memory object
    offset: usize,
}

bitflags! {
    pub struct Protection: u8 {
        const READ    = 0b0001;
        const WRITE   = 0b0010;
        const EXECUTE = 0b0100;
    }
}

/// Task's virtual memory map.
///
/// Contains all virtual memory regions for a task.
/// Organized as a sorted list for efficient lookup.
///
pub struct VmMap {
    /// List of regions (sorted by start address, non-overlapping)
    regions: Vec<VmRegion>,

    /// Root page table physical address
    page_table: PhysAddr,
}

impl VmMap {
    /// Handles a page fault at the given virtual address.
    ///
    /// Called by the CPU when:
    /// - Accessing unmapped memory
    /// - Protection violation (e.g., write to read-only page)
    ///
    /// Steps:
    /// 1. Find the region containing this address
    /// 2. Check if access is allowed (protection)
    /// 3. Ask the region's pager for the page data
    /// 4. Map the physical page into page tables
    /// 5. Resume execution
    ///
    pub async fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        access_type: AccessType,
    ) -> Result<(), VmError> {
        // Find region
        let region = self.find_region(addr)
            .ok_or(VmError::InvalidAddress)?;

        // Check protection
        if !region.protection.allows(access_type) {
            return Err(VmError::ProtectionViolation);
        }

        // Calculate offset into memory object
        let offset_in_region = addr - region.start;
        let object_offset = region.offset + offset_in_region;

        // Ask pager for page data
        let page = region.memory_object
            .get_page(object_offset)
            .await?;

        // Map the page
        self.map_page(addr, page.phys_addr, region.protection)?;

        Ok(())
    }
}
```

## External Pagers - Mach's Innovation

The key insight: *Separate mechanism from policy*.

The **kernel** provides the mechanism:
- Page tables
- Protection enforcement
- Page fault handling

**User-space pagers** provide the policy:
- Where does page data come from? (file, swap, zero-fill, etc.)
- When to page data out?
- How to handle copy-on-write?

```
┌──────────────────────────────────────┐
│  Task A                              │
│  ┌────────────────────────────────┐  │
│  │  Access: addr = 0x1000         │  │
│  └────────────┬───────────────────┘  │
└───────────────┼──────────────────────┘
                │ Page Fault!
                ▼
┌──────────────────────────────────────┐
│  Mach_R Kernel                       │
│  ┌────────────────────────────────┐  │
│  │  1. Catch fault                │  │
│  │  2. Find region for 0x1000     │  │
│  │  3. Send IPC to pager          │  │
│  └────────────┬───────────────────┘  │
└───────────────┼──────────────────────┘
                │ IPC Message
                ▼
┌──────────────────────────────────────┐
│  File System Pager (user-space)     │
│  ┌────────────────────────────────┐  │
│  │  1. Receive page request       │  │
│  │  2. Read from file on disk     │  │
│  │  3. Reply with page data       │  │
│  └────────────┬───────────────────┘  │
└───────────────┼──────────────────────┘
                │ IPC Reply
                ▼
┌──────────────────────────────────────┐
│  Mach_R Kernel                       │
│  ┌────────────────────────────────┐  │
│  │  1. Receive page data          │  │
│  │  2. Map page in task's tables  │  │
│  │  3. Resume task execution      │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

### Memory Objects - The Pager Interface

```rust
/// Abstract interface to page data.
///
/// Implementors:
/// - DefaultPager: Anonymous memory (heap/stack)
/// - FilePager: Memory-mapped files
/// - SwapPager: Swap space management
///
#[async_trait]
pub trait MemoryObject: Send + Sync {
    /// Provides a page of data.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset into this memory object (page-aligned)
    ///
    /// # Returns
    ///
    /// Physical page containing the data, or an error.
    ///
    /// # Implementation Note
    ///
    /// This is async to allow pagers to perform I/O.
    /// For example, FilePager may need to read from disk.
    async fn get_page(&self, offset: usize) -> Result<Page, PagerError>;

    /// Requests to page out data.
    ///
    /// Kernel asks pager to save the page and free physical memory.
    /// Pager can refuse if it can't save the data (e.g., disk full).
    async fn page_out(&self, offset: usize, page: Page) -> Result<(), PagerError>;

    /// Notification that page is no longer needed.
    ///
    /// Allows pager to release resources.
    async fn page_in_completed(&self, offset: usize);
}
```

Example: Default Pager (zero-fill memory)

```rust
/// Provides zero-filled pages for anonymous memory.
///
/// Used for: heap, stack, anonymous mmap()
///
pub struct DefaultPager {
    /// Allocator for physical pages
    page_allocator: Arc<Mutex<PageAllocator>>,
}

#[async_trait]
impl MemoryObject for DefaultPager {
    async fn get_page(&self, _offset: usize) -> Result<Page, PagerError> {
        // Allocate a physical page
        let mut allocator = self.page_allocator.lock().await;
        let phys_addr = allocator.allocate_page()
            .ok_or(PagerError::OutOfMemory)?;

        // Zero-fill it (security: don't leak previous data)
        unsafe {
            let ptr = phys_addr as *mut u8;
            core::ptr::write_bytes(ptr, 0, PAGE_SIZE);
        }

        Ok(Page {
            phys_addr,
            state: PageState::Present,
        })
    }

    async fn page_out(&self, _offset: usize, page: Page) -> Result<(), PagerError> {
        // Anonymous pages can be discarded when paged out
        // (they're zero-fill, so just free the page)

        let mut allocator = self.page_allocator.lock().await;
        unsafe {
            allocator.free_page(page.phys_addr);
        }

        Ok(())
    }

    async fn page_in_completed(&self, _offset: usize) {
        // Nothing to do
    }
}
```

## Copy-on-Write - Efficient Forking

When a task forks, we don't copy all memory immediately. Instead:

1. **Mark pages read-only** in both parent and child
2. **Share the physical pages**
3. **On write**: Copy the page, update page tables

```
Before Fork:
┌──────────────┐
│   Parent     │
│  ┌────────┐  │
│  │ Page A │  │ ──────┐
│  │  R/W   │  │       │
│  └────────┘  │       ▼
└──────────────┘   ┌────────┐
                   │ Phys   │
                   │ Page 1 │
                   └────────┘

After Fork (before write):
┌──────────────┐
│   Parent     │       Both point to
│  ┌────────┐  │       same physical page,
│  │ Page A │  │──┐    but marked read-only
│  │  R/O   │  │  │
│  └────────┘  │  │
└──────────────┘  │    ┌────────┐
                  ├───▶│ Phys   │
┌──────────────┐  │    │ Page 1 │
│    Child     │  │    └────────┘
│  ┌────────┐  │  │
│  │ Page A │  │──┘
│  │  R/O   │  │
│  └────────┘  │
└──────────────┘

After Write by Child:
┌──────────────┐
│   Parent     │
│  ┌────────┐  │       ┌────────┐
│  │ Page A │  │──────▶│ Phys   │
│  │  R/O   │  │       │ Page 1 │
│  └────────┘  │       └────────┘
└──────────────┘

┌──────────────┐
│    Child     │       ┌────────┐
│  ┌────────┐  │──────▶│ Phys   │
│  │ Page A │  │       │ Page 2 │ (copy)
│  │  R/W   │  │       └────────┘
│  └────────┘  │
└──────────────┘
```

Implementation:

```rust
/// Handles copy-on-write fault.
///
/// Called when a task writes to a COW page.
///
fn handle_cow_fault(&mut self, addr: VirtAddr) -> Result<(), VmError> {
    // Find the page table entry
    let pte = self.page_table.get_entry(addr)?;

    // Verify this is a COW fault
    if !pte.is_cow() {
        return Err(VmError::NotCow);
    }

    // Get reference count of physical page
    let ref_count = PAGE_REF_COUNTS.get(pte.phys_addr);

    if ref_count == 1 {
        // We're the only one using this page
        // Just make it writable again
        pte.set_writable(true);
        pte.clear_cow();
    } else {
        // Others are using this page
        // Must copy it

        // Allocate new page
        let new_page = PAGE_ALLOCATOR.allocate_page()
            .ok_or(VmError::OutOfMemory)?;

        // Copy data
        unsafe {
            core::ptr::copy_nonoverlapping(
                pte.phys_addr as *const u8,
                new_page as *mut u8,
                PAGE_SIZE,
            );
        }

        // Update page table to point to our copy
        pte.set_phys_addr(new_page);
        pte.set_writable(true);
        pte.clear_cow();

        // Decrement ref count on original
        PAGE_REF_COUNTS.decrement(pte.phys_addr);
    }

    // Flush TLB
    flush_tlb(addr);

    Ok(())
}
```

## Memory Protection

Each page has protection attributes:

```rust
// In page table entry:
┌─────────────────────────────────────┐
│  Physical Address    │ UXN│XN│AP│  │
├──────────────────────┼────┼──┼──┼──┤
│  40 bits            │ 1b │1b│2b│..│
└─────────────────────────────────────┘

UXN = User Execute Never
XN  = Execute Never
AP  = Access Permission (00=R/W, 01=R/W, 10=R/O, 11=R/O)
```

The CPU checks these bits on every memory access. Violations trigger page faults.

## Summary

Mach_R's memory management builds in layers:

1. **Hardware**: Page tables, MMU, TLB
2. **Physical allocator**: Manages physical RAM
3. **Virtual memory**: Maps virtual to physical
4. **External pagers**: Provide page data
5. **Copy-on-write**: Efficient sharing

The beauty: Each layer is simple. Complexity emerges from composition, not from complicated individual pieces.

---

**See Also:**
- [Task & Threading](task-threading.md) - How tasks use memory
- [IPC System](ipc-system.md) - Zero-copy message passing
- [Overview](overview.md) - High-level architecture
