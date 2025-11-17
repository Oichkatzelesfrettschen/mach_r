# ROADMAP: FROM FAKE KERNEL TO REAL OS

## Current State: "Hello World with Tests"
- Boots ✓
- Prints ✓  
- Fake tests ✓
- No input ✗
- No programs ✗
- No shell ✗

## Week 1: Make It Respond

### Monday: IDT Setup
```c
// idt.c - Need to write
struct idt_entry {
    uint16_t base_low;
    uint16_t selector;
    uint8_t zero;
    uint8_t flags;
    uint16_t base_high;
} __attribute__((packed));

// interrupt.S - Need to write  
isr0: push $0; push $0; jmp isr_common
isr1: push $0; push $1; jmp isr_common
...
```
- [ ] Define IDT entries (256)
- [ ] Write 32 ISR stubs
- [ ] Write IRQ handlers (16)
- [ ] Load IDT with LIDT

### Tuesday: Timer
```c
// timer.c - Need to write
void init_timer(uint32_t frequency) {
    uint32_t divisor = 1193180 / frequency;
    outb(0x43, 0x36);
    outb(0x40, divisor & 0xFF);
    outb(0x40, divisor >> 8);
}
```
- [ ] Initialize PIT
- [ ] Handle IRQ0
- [ ] Add tick counter
- [ ] Test with on-screen counter

### Wednesday: Keyboard
```c
// keyboard.c - Need to write
void keyboard_handler() {
    uint8_t scancode = inb(0x60);
    char c = scancode_to_ascii[scancode];
    buffer_add(c);
}
```
- [ ] Handle IRQ1
- [ ] Scancode translation
- [ ] Circular buffer
- [ ] getchar() function

### Thursday-Friday: Memory Manager
```c
// pmm.c - Need to write
uint32_t* page_bitmap;
void init_pmm(multiboot_info_t* mbi) {
    // Parse memory map
    // Set up bitmap
    // Mark kernel pages used
}
```
- [ ] Parse multiboot memory map
- [ ] Bitmap allocator
- [ ] page_alloc()/page_free()
- [ ] Test with allocation loop

## Week 2: Make It Manage

### Monday-Tuesday: Virtual Memory
```c
// vmm.c - Need to write
void map_page(uint32_t virt, uint32_t phys, uint32_t flags) {
    uint32_t pd_index = virt >> 22;
    uint32_t pt_index = (virt >> 12) & 0x3FF;
    // Map in page tables
}
```
- [ ] Page directory management
- [ ] Page table allocation
- [ ] map/unmap functions
- [ ] Page fault handler

### Wednesday-Thursday: Processes
```c
// process.c - Need to write
struct process {
    uint32_t pid;
    uint32_t esp, ebp, eip;
    uint32_t page_directory;
    enum state { RUNNING, READY, BLOCKED };
};
```
- [ ] PCB structure
- [ ] Context switch in assembly
- [ ] Process queue
- [ ] Simple round-robin

### Friday: User Mode
```c
// usermode.c - Need to write
void enter_usermode(uint32_t entry) {
    set_kernel_stack(tss.esp0);
    asm volatile(
        "mov $0x23, %%ax\n"
        "mov %%ax, %%ds\n"
        // ... switch to ring 3
    );
}
```
- [ ] GDT user segments
- [ ] TSS setup
- [ ] Ring 3 transition
- [ ] Test with user program

## Week 3: Make It Serve

### Monday-Tuesday: System Calls
```c
// syscall.c - Need to write
void syscall_handler(registers_t* regs) {
    switch(regs->eax) {
        case SYS_WRITE:
            sys_write(regs->ebx, regs->ecx, regs->edx);
            break;
        // ...
    }
}
```
- [ ] INT 0x80 handler
- [ ] Syscall table
- [ ] Basic syscalls (read, write, exit)
- [ ] Test from userspace

### Wednesday-Friday: Simple Filesystem
```c
// fs.c - Need to write
struct inode {
    uint32_t size;
    uint32_t blocks[12];
    uint32_t indirect;
};
```
- [ ] Initrd format
- [ ] Read files
- [ ] Directory traversal
- [ ] Execute programs

## Week 4: Make It Usable

### Monday-Wednesday: Shell
```c
// shell.c - User program!
int main() {
    char line[256];
    while(1) {
        write(1, "$ ", 2);
        read(0, line, 256);
        execute_command(line);
    }
}
```
- [ ] Command parsing
- [ ] Fork/exec
- [ ] Built-ins (cd, exit)
- [ ] Path search

### Thursday-Friday: Basic Utils
```c
// ls.c, cat.c, echo.c - User programs
```
- [ ] ls - list directory
- [ ] cat - show file
- [ ] echo - print args
- [ ] ps - show processes

## Critical Missing Pieces Right Now

### 1. Can't Get Input
- No keyboard driver
- No interrupt handling
- No input buffer

### 2. Can't Run Programs  
- No ELF loader
- No exec() syscall
- No user mode

### 3. Can't Manage Memory
- No page allocation
- No virtual memory
- No heap

### 4. Can't Switch Tasks
- No context saving
- No scheduler
- No preemption

## Simplest Path to Shell (2 weeks)

### Week 1
1. IDT + keyboard → Can type
2. Memory manager → Can allocate
3. User mode → Can run programs

### Week 2  
1. Syscalls → Programs can I/O
2. Exec → Can load programs
3. Shell → Can run commands

## Code Needed (Realistic)

```
kernel/
├── interrupts/
│   ├── idt.c        (200 lines)
│   ├── isr.S        (300 lines)
│   └── irq.c        (150 lines)
├── drivers/
│   ├── keyboard.c   (150 lines)
│   ├── timer.c      (100 lines)
│   └── vga.c        (existing)
├── memory/
│   ├── pmm.c        (200 lines)
│   ├── vmm.c        (400 lines)
│   └── heap.c       (200 lines)
├── process/
│   ├── process.c    (300 lines)
│   ├── scheduler.c  (200 lines)
│   └── switch.S     (100 lines)
├── syscall/
│   ├── syscall.c    (200 lines)
│   └── handlers.c   (300 lines)
├── fs/
│   ├── vfs.c        (300 lines)
│   └── initrd.c     (200 lines)
└── main.c           (existing)

userland/
├── shell/
│   └── shell.c      (400 lines)
├── utils/
│   ├── ls.c         (100 lines)
│   ├── cat.c        (50 lines)
│   └── echo.c       (30 lines)
└── libc/
    └── syscalls.c   (100 lines)

Total: ~4000 lines for minimal interactive OS
```

## The Brutal Truth

Right now we have:
- 872 lines that boot and print

To get a shell we need:
- 4000+ more lines minimum
- 2-4 weeks of solid coding
- Lots of debugging

But it's doable. Start with IDT tomorrow.