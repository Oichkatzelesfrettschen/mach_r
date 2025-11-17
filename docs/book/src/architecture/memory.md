# Memory Management in Mach_R

Mach's memory management system is one of its most powerful and defining features. It abstracts physical memory away from tasks, providing a clean, object-oriented model of virtual memory.

## Core Concepts

- **Tasks and Address Spaces**: Each task has its own virtual address space, providing complete isolation from other tasks. A task's address space is simply a list of mappings from virtual address ranges to memory objects.

- **Memory Objects**: A memory object is a kernel abstraction representing a source of data, like a file, a device, or pure anonymous memory. These objects are managed by user-space servers called **pagers**.

- **Pagers (Memory Managers)**: Pagers are user-space tasks responsible for providing the data that backs a memory object. When a task tries to access a page of memory that is not currently in physical RAM (a page fault), the kernel sends a message to the pager responsible for that memory region, requesting the data for that page. This allows for incredible flexibility: filesystems, network-backed memory, and custom swapping strategies can all be implemented as user-space pagers.

- **Copy-on-Write (COW)**: COW is a fundamental optimization. When a task's address space is duplicated (e.g., during a `fork()` operation), the kernel does not immediately copy all the memory. Instead, it marks the memory pages as read-only in both tasks. If either task attempts to write to a page, the kernel traps the write, allocates a new page of physical memory, copies the original data, and then maps the new page into the writing task's address space. This makes operations like process creation extremely fast.

## Implementation in Mach_R

### Memory Layout (AArch64 Example)

The virtual address space will be partitioned to separate user and kernel memory.

```
0x0000_0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF : User Space (for applications and servers)
0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF : Kernel Space (higher-half kernel)
```

### Key Data Structures

- **Physical Page Allocator**: A buddy system or similar algorithm will be used to manage physical RAM frames.

- **`AddressSpace`**: A per-task structure (likely a balanced binary tree or similar) that manages the list of `VmMapping`s.

- **`VmMapping`**: A mapping from a virtual address range to a region of a `MemoryObject`.

- **`MemoryObject`**: A trait-based abstraction for a source of memory. Implementations could include anonymous memory, device memory, or pager-backed memory.

```rust
// Conceptual structure for a Task's memory
struct AddressSpace {
    mappings: BTreeMap<VirtualAddress, VmMapping>,
    page_table_root: PhysicalAddress,
}

struct VmMapping {
    protection: ProtectionFlags, // Read, Write, Execute
    object: Arc<dyn MemoryObject>,
    offset_in_object: u64,
}

trait MemoryObject {
    // Method to handle a page fault on this object
    fn handle_fault(&self, offset: u64) -> Result<PhysicalAddress, FaultError>;
}
```

### The Page Fault Path

1.  A thread accesses a virtual address that is not currently mapped in the CPU's MMU.
2.  The CPU triggers a page fault exception, trapping into the kernel.
3.  The kernel's exception handler looks up the faulting address in the current task's `AddressSpace`.
4.  It finds the corresponding `MemoryObject` and calls its `handle_fault` method.
5.  The `MemoryObject` (or its backing pager via IPC) provides the data, which is placed into a physical page frame.
6.  The kernel updates the CPU's page tables to map the virtual address to the new physical page.
7.  The kernel returns from the exception, and the instruction is re-executed, this time successfully.
