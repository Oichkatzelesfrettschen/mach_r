# Roadmap: From Current State to Bootable Shell on QEMU x86_64

*A realistic, detailed plan for getting Mach_R to boot with a working shell*

## Current State (2025-01-19)

**What Works:**
- ✅ Port/IPC system (library only)
- ✅ Message passing (library only)
- ✅ Task structures (library only)
- ✅ MIG tool (complete)

**What Doesn't Work:**
- ❌ Doesn't boot
- ❌ No memory management
- ❌ No scheduler
- ❌ No user mode
- ❌ No programs
- ❌ No shell

## Goal: Bootable System

```
┌──────────────────────────────────────┐
│  QEMU x86_64                         │
│  ┌────────────────────────────────┐  │
│  │  mach_r.qcow2                  │  │
│  │  ┌──────────────────────────┐  │  │
│  │  │  Bootloader (GRUB)       │  │  │
│  │  ├──────────────────────────┤  │  │
│  │  │  Mach_R Kernel           │  │  │
│  │  ├──────────────────────────┤  │  │
│  │  │  Init ramdisk (initrd)   │  │  │
│  │  │    - Shell binary        │  │  │
│  │  │    - Core utilities      │  │  │
│  │  └──────────────────────────┘  │  │
│  └────────────────────────────────┘  │
│                                      │
│  $ qemu-system-x86_64 \              │
│      -drive file=mach_r.qcow2 \      │
│      -m 256M                         │
│                                      │
│  [Boot messages...]                  │
│  Mach_R shell> █                     │
└──────────────────────────────────────┘
```

## The Path: 10 Phases

### Phase 1: Boot Infrastructure (1-2 weeks)

**Goal:** Kernel boots on x86_64 and prints to console

#### Tasks:

1. **x86_64 Boot Sequence**
   ```
   src/arch/x86_64/boot.S
   ```
   - Multiboot2 header
   - 32-bit → 64-bit long mode transition
   - Set up initial GDT
   - Set up initial IDT
   - Jump to Rust entry point

   **Lines of Code:** ~200 (assembly)
   **Difficulty:** Medium-High
   **Time:** 2-3 days

2. **Early Console Output**
   ```
   src/arch/x86_64/vga.rs
   src/arch/x86_64/serial.rs
   ```
   - VGA text mode (0xB8000)
   - Serial port output (COM1)
   - println! macro for kernel

   **Lines of Code:** ~150
   **Difficulty:** Low
   **Time:** 1 day

3. **Multiboot2 Parsing**
   ```
   src/boot/multiboot2.rs
   ```
   - Parse memory map
   - Parse module information
   - Parse framebuffer info

   **Lines of Code:** ~300
   **Difficulty:** Medium
   **Time:** 2 days

**Milestone 1:** `make qemu-x86` boots and prints "Mach_R kernel starting..."

---

### Phase 2: Memory Management (2-3 weeks)

**Goal:** Kernel can allocate/free memory

#### Tasks:

1. **Physical Page Allocator**
   ```
   src/memory/physical.rs
   ```
   - Bitmap allocator for physical pages
   - Parse multiboot memory map
   - Mark kernel/initrd memory as used
   - alloc_page() / free_page()

   **Lines of Code:** ~400
   **Difficulty:** Medium
   **Time:** 4-5 days

2. **Virtual Memory Manager**
   ```
   src/memory/virtual.rs
   src/arch/x86_64/paging.rs
   ```
   - 4-level page table management
   - map_page() / unmap_page()
   - Higher-half kernel mapping
   - Identity mapping for early boot
   - TLB flush functions

   **Lines of Code:** ~600
   **Difficulty:** High
   **Time:** 1 week

3. **Heap Allocator**
   ```
   src/memory/heap.rs
   ```
   - Replace bump allocator
   - Buddy allocator or slab allocator
   - GlobalAlloc implementation
   - Box/Vec now work

   **Lines of Code:** ~400
   **Difficulty:** Medium
   **Time:** 3-4 days

**Milestone 2:** Kernel can allocate Vec, Box, HashMap

---

### Phase 3: Task & Thread Support (2-3 weeks)

**Goal:** Kernel can create and schedule threads

#### Tasks:

1. **Thread Scheduler**
   ```
   src/scheduler.rs
   ```
   - Priority run queues (32 levels)
   - Round-robin within priority
   - Thread state machine
   - yield() / schedule()

   **Lines of Code:** ~500
   **Difficulty:** Medium-High
   **Time:** 1 week

2. **Context Switching**
   ```
   src/arch/x86_64/context.rs
   src/arch/x86_64/switch.S
   ```
   - Save/restore all registers
   - Switch stack pointer
   - Switch page tables
   - FPU/SSE state handling

   **Lines of Code:** ~300 (assembly + Rust)
   **Difficulty:** High
   **Time:** 4-5 days

3. **Timer Interrupt**
   ```
   src/arch/x86_64/timer.rs
   src/arch/x86_64/interrupt.rs
   ```
   - Program PIT or APIC timer
   - Timer interrupt handler
   - Preemptive scheduling
   - Time slice expiration

   **Lines of Code:** ~400
   **Difficulty:** Medium-High
   **Time:** 3-4 days

**Milestone 3:** Two kernel threads run concurrently, switch every 10ms

---

### Phase 4: IPC Enhancement (1 week)

**Goal:** Complete IPC for userland communication

#### Tasks:

1. **Complete Message Passing**
   ```
   src/message.rs
   src/port.rs
   ```
   - Out-of-line data support
   - Port right transfer in messages
   - Blocking/non-blocking receive
   - Timeouts

   **Lines of Code:** ~300
   **Difficulty:** Medium
   **Time:** 3-4 days

2. **Port Namespace**
   ```
   src/task.rs
   ```
   - Per-task port tables
   - Name → port mapping
   - Port right management
   - Proper cleanup on task exit

   **Lines of Code:** ~250
   **Difficulty:** Medium
   **Time:** 2-3 days

**Milestone 4:** Tasks can send messages with port rights

---

### Phase 5: System Calls (1-2 weeks)

**Goal:** Userland can call kernel services

#### Tasks:

1. **Syscall Infrastructure**
   ```
   src/arch/x86_64/syscall.rs
   src/syscall/mod.rs
   ```
   - SYSCALL/SYSRET support
   - Syscall table
   - Argument passing (registers)
   - Return value handling

   **Lines of Code:** ~300
   **Difficulty:** Medium-High
   **Time:** 4-5 days

2. **Basic Syscalls**
   ```
   src/syscall/handlers.rs
   ```
   - sys_write (console output)
   - sys_read (console input)
   - sys_exit (terminate task)
   - sys_mach_msg (IPC)
   - sys_thread_create
   - sys_vm_allocate

   **Lines of Code:** ~400
   **Difficulty:** Medium
   **Time:** 3-4 days

**Milestone 5:** Syscall from userland prints to console

---

### Phase 6: User Mode (1-2 weeks)

**Goal:** Run code in ring 3 (user mode)

#### Tasks:

1. **User Mode Entry**
   ```
   src/arch/x86_64/usermode.rs
   ```
   - Set up user segments (GDT)
   - TSS for kernel stack
   - IRET to ring 3
   - Syscall entry from user

   **Lines of Code:** ~300
   **Difficulty:** Medium-High
   **Time:** 4-5 days

2. **ELF Loader**
   ```
   src/loader/elf.rs
   ```
   - Parse ELF64 headers
   - Load program segments
   - Set up initial stack
   - Jump to entry point

   **Lines of Code:** ~500
   **Difficulty:** Medium
   **Time:** 4-5 days

**Milestone 6:** Load and run "hello world" ELF binary

---

### Phase 7: Initial RAM Disk (1 week)

**Goal:** Bundle programs into kernel image

#### Tasks:

1. **Initrd Format**
   ```
   tools/mkfs_initrd.rs
   src/fs/initrd.rs
   ```
   - Simple tar-like format
   - Header + file entries
   - File data inline
   - No directories (flat namespace)

   **Lines of Code:** ~400
   **Difficulty:** Low-Medium
   **Time:** 3 days

2. **Initrd Driver**
   ```
   src/fs/initrd.rs
   ```
   - Parse initrd from multiboot module
   - File lookup by name
   - Read file contents
   - List files

   **Lines of Code:** ~300
   **Difficulty:** Low
   **Time:** 2 days

**Milestone 7:** Load initrd, list files, read contents

---

### Phase 8: Shell & Utilities (2-3 weeks)

**Goal:** Interactive shell running in userland

#### Tasks:

1. **Minimal libc**
   ```
   userland/libc/
   ```
   - printf/sprintf
   - strlen/strcmp/strcpy
   - malloc/free (from syscall)
   - Syscall wrappers

   **Lines of Code:** ~800
   **Difficulty:** Medium
   **Time:** 1 week

2. **Shell**
   ```
   userland/shell/main.c
   ```
   - Read line from input
   - Parse command line
   - Fork + exec (or spawn)
   - Built-ins: cd, exit, help
   - External commands

   **Lines of Code:** ~600
   **Difficulty:** Medium
   **Time:** 1 week

3. **Core Utilities**
   ```
   userland/utils/ls.c
   userland/utils/cat.c
   userland/utils/echo.c
   ```
   - ls: List files
   - cat: Display file contents
   - echo: Print arguments

   **Lines of Code:** ~300
   **Difficulty:** Low
   **Time:** 2-3 days

**Milestone 8:** Shell runs, can execute ls/cat/echo

---

### Phase 9: Disk Image Creation (3-4 days)

**Goal:** Create bootable disk image

#### Tasks:

1. **Build System**
   ```
   Makefile or build.rs
   ```
   - Compile kernel
   - Compile userland programs
   - Create initrd with programs
   - Combine into bootable image

   **Lines of Code:** ~200 (Makefile)
   **Difficulty:** Low-Medium
   **Time:** 2 days

2. **Bootable Image**
   ```
   scripts/create_image.sh
   ```
   ```bash
   # Create raw disk image
   dd if=/dev/zero of=mach_r.img bs=1M count=128

   # Create partition table
   parted mach_r.img mklabel msdos
   parted mach_r.img mkpart primary 1MiB 100%

   # Format partition
   mkfs.ext2 disk.img

   # Mount and install GRUB
   sudo mount -o loop mach_r.img /mnt
   sudo grub-install --boot-directory=/mnt/boot /dev/loop0

   # Copy kernel and initrd
   sudo cp kernel.bin /mnt/boot/
   sudo cp initrd.img /mnt/boot/

   # Create GRUB config
   cat > /mnt/boot/grub/grub.cfg <<EOF
   menuentry "Mach_R" {
       multiboot2 /boot/kernel.bin
       module2 /boot/initrd.img
       boot
   }
   EOF

   sudo umount /mnt
   ```

   **Difficulty:** Low
   **Time:** 1 day

3. **Convert to QCOW2**
   ```bash
   qemu-img convert -f raw -O qcow2 mach_r.img mach_r.qcow2
   ```

   **Difficulty:** Trivial
   **Time:** 5 minutes

**Milestone 9:** mach_r.qcow2 boots in QEMU

---

### Phase 10: Testing & Polish (1 week)

**Goal:** Stable, demonstrable system

#### Tasks:

1. **Fix Boot Issues**
   - Kernel panics
   - Triple faults
   - Memory corruption
   - Race conditions

   **Time:** 3-4 days

2. **Documentation**
   - Update README with boot instructions
   - Add architecture diagrams
   - Document syscalls
   - Write user guide

   **Time:** 2-3 days

**Milestone 10:** Reliable boot, stable shell, demo-ready

---

## Timeline Summary

| Phase | Component | Time Estimate | Difficulty |
|-------|-----------|--------------|------------|
| 1 | Boot Infrastructure | 1-2 weeks | Medium-High |
| 2 | Memory Management | 2-3 weeks | High |
| 3 | Task & Thread | 2-3 weeks | High |
| 4 | IPC Enhancement | 1 week | Medium |
| 5 | System Calls | 1-2 weeks | Medium-High |
| 6 | User Mode | 1-2 weeks | Medium-High |
| 7 | Initial RAM Disk | 1 week | Low-Medium |
| 8 | Shell & Utilities | 2-3 weeks | Medium |
| 9 | Disk Image | 3-4 days | Low-Medium |
| 10 | Testing & Polish | 1 week | Medium |

**Total Time: 14-20 weeks (3.5 - 5 months)**

**Total Code: ~7,000-10,000 lines**

## Critical Path

The *minimum viable path* (fastest to bootable shell):

1. **Boot** (2 weeks)
2. **Memory** (3 weeks)
3. **Scheduler** (3 weeks)
4. **Syscalls** (2 weeks)
5. **User mode** (2 weeks)
6. **Initrd** (1 week)
7. **Shell** (2 weeks)
8. **Disk image** (4 days)

**Minimum: ~15 weeks (4 months)**

## Parallel Tracks

Work can be parallelized:

**Track 1: Kernel**
- Boot → Memory → Threads → Syscalls

**Track 2: Userland**
- libc → Shell → Utilities (can start early with stubs)

**Track 3: Tools**
- initrd creator
- Disk image builder

With 2-3 developers, timeline could compress to ~10-12 weeks.

## High-Risk Areas

### 1. Context Switching
- **Risk:** Hard to debug, subtle bugs
- **Mitigation:** Test extensively, single-step with GDB
- **Fallback:** Use simpler cooperative scheduling first

### 2. Memory Management
- **Risk:** Corruption, leaks, panics
- **Mitigation:** Extensive testing, sanitizers
- **Fallback:** Simple allocators before complex ones

### 3. User Mode Transition
- **Risk:** Triple faults, crashes
- **Mitigation:** Test in stages (kernel threads first)
- **Fallback:** Stay in kernel mode longer

### 4. QEMU Specifics
- **Risk:** Works in QEMU, fails on real hardware
- **Mitigation:** Test on real hardware early
- **Alternative:** Focus on QEMU-only for initial demo

## Detailed File Structure

```
mach_r/
├── src/
│   ├── arch/
│   │   └── x86_64/
│   │       ├── boot.S                # New - boot assembly
│   │       ├── context.rs            # New - context switching
│   │       ├── gdt.rs                # New - GDT setup
│   │       ├── idt.rs                # New - IDT setup
│   │       ├── interrupt.rs          # New - interrupt handling
│   │       ├── paging.rs             # New - page tables
│   │       ├── serial.rs             # New - serial console
│   │       ├── switch.S              # New - context switch asm
│   │       ├── syscall.rs            # New - syscall entry
│   │       ├── timer.rs              # New - timer setup
│   │       ├── usermode.rs           # New - ring 3 transition
│   │       └── vga.rs                # New - VGA text mode
│   ├── boot/
│   │   └── multiboot2.rs             # New - multiboot parsing
│   ├── fs/
│   │   └── initrd.rs                 # New - initrd filesystem
│   ├── loader/
│   │   └── elf.rs                    # New - ELF loader
│   ├── memory/
│   │   ├── heap.rs                   # New - heap allocator
│   │   ├── physical.rs               # New - page allocator
│   │   └── virtual.rs                # New - VM manager
│   ├── scheduler.rs                  # Enhance - add implementation
│   ├── syscall/
│   │   ├── mod.rs                    # New - syscall table
│   │   └── handlers.rs               # New - syscall implementations
│   └── main.rs                       # Modify - kernel entry point
│
├── userland/
│   ├── libc/
│   │   ├── stdio.c                   # New - printf, etc.
│   │   ├── string.c                  # New - str functions
│   │   ├── syscalls.c                # New - syscall wrappers
│   │   └── malloc.c                  # New - userland allocator
│   ├── shell/
│   │   └── main.c                    # New - shell implementation
│   └── utils/
│       ├── ls.c                      # New - list files
│       ├── cat.c                     # New - display files
│       └── echo.c                    # New - echo arguments
│
├── tools/
│   ├── mkfs_initrd.rs                # New - create initrd
│   └── create_image.sh               # New - build disk image
│
└── grub.cfg                          # New - GRUB configuration
```

## Step-by-Step First Task

### Start Here: Boot on x86_64

**File:** `src/arch/x86_64/boot.S`

```asm
.section .multiboot
.align 8
multiboot2_header_start:
    .long 0xe85250d6                  // Magic number
    .long 0                           // Architecture (i386)
    .long multiboot2_header_end - multiboot2_header_start
    .long -(0xe85250d6 + 0 + (multiboot2_header_end - multiboot2_header_start))

    // End tag
    .word 0
    .word 0
    .long 8
multiboot2_header_end:

.section .bss
.align 4096
boot_page_table_l4:
    .skip 4096
boot_page_table_l3:
    .skip 4096
boot_page_table_l2:
    .skip 4096
stack_bottom:
    .skip 16384
stack_top:

.section .text
.global _start
.code32
_start:
    // Save multiboot info
    mov %ebx, %edi

    // Set up page tables
    mov $boot_page_table_l3, %eax
    or $0b11, %eax
    mov %eax, boot_page_table_l4

    // Identity map first 1GB
    mov $boot_page_table_l2, %eax
    or $0b11, %eax
    mov %eax, boot_page_table_l3

    mov $0, %ecx
.map_l2:
    mov $0x200000, %eax
    mul %ecx
    or $0b10000011, %eax
    mov %eax, boot_page_table_l2(,%ecx,8)
    inc %ecx
    cmp $512, %ecx
    jne .map_l2

    // Enable PAE
    mov %cr4, %eax
    or $(1 << 5), %eax
    mov %eax, %cr4

    // Set long mode bit
    mov $0xC0000080, %ecx
    rdmsr
    or $(1 << 8), %eax
    wrmsr

    // Load page table
    mov $boot_page_table_l4, %eax
    mov %eax, %cr3

    // Enable paging
    mov %cr0, %eax
    or $(1 << 31), %eax
    mov %eax, %cr0

    // Load 64-bit GDT
    lgdt gdt64_pointer

    // Jump to 64-bit code
    ljmp $0x08, $long_mode_start

.code64
long_mode_start:
    // Set up segment registers
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss

    // Set up stack
    mov $stack_top, %rsp

    // Call Rust entry
    call kernel_main

    // Halt
.halt:
    hlt
    jmp .halt

.section .rodata
gdt64:
    .quad 0
    .quad 0x00AF9A000000FFFF  // Code segment
    .quad 0x00AF92000000FFFF  // Data segment
gdt64_pointer:
    .word gdt64_pointer - gdt64 - 1
    .quad gdt64
```

**Next:** Implement `kernel_main()` in Rust to print "Hello from Mach_R!"

## Suggested Development Order

1. ✅ **Week 1:** Boot + VGA console
2. ✅ **Week 2-4:** Memory management
3. ✅ **Week 5-7:** Scheduler + threading
4. ✅ **Week 8:** IPC completion
5. ✅ **Week 9-10:** Syscalls
6. ✅ **Week 11-12:** User mode
7. ✅ **Week 13:** Initrd
8. ✅ **Week 14-16:** Shell + utilities
9. ✅ **Week 17:** Disk image + testing

## Success Criteria

You've succeeded when:

```bash
$ qemu-system-x86_64 -drive file=mach_r.qcow2,format=qcow2 -m 256M -serial stdio

[Booting...]
Mach_R kernel v0.1.0
Memory: 256 MB
Loading initrd...
Starting init...

Mach_R shell v0.1
Type 'help' for commands

shell> ls
shell
ls
cat
echo

shell> cat /proc/meminfo
Total: 256 MB
Used: 12 MB
Free: 244 MB

shell> echo Hello, Mach_R!
Hello, Mach_R!

shell> █
```

## Reality Check

**This is a lot of work.** 3-5 months of focused development.

**But it's achievable.** Each phase builds on the previous one. The path is clear.

**And it's worthwhile.** At the end, you'll have a working microkernel OS demonstrating Mach concepts in memory-safe Rust.

**Next step:** Start with `src/arch/x86_64/boot.S` and get "Hello World" printing from the kernel.

---

**Status:** Detailed roadmap complete
**Next:** Begin Phase 1 implementation
