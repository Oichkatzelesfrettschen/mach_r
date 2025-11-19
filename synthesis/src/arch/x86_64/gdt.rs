//! x86_64 Global Descriptor Table

use core::arch::asm;

/// GDT entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

impl GdtEntry {
    const fn null() -> Self {
        GdtEntry {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }

    const fn new(base: u32, limit: u32, access: u8, granularity: u8) -> Self {
        GdtEntry {
            limit_low: (limit & 0xFFFF) as u16,
            base_low: (base & 0xFFFF) as u16,
            base_middle: ((base >> 16) & 0xFF) as u8,
            access,
            granularity: (granularity & 0xF0) | (((limit >> 16) & 0x0F) as u8),
            base_high: ((base >> 24) & 0xFF) as u8,
        }
    }
}

/// GDT pointer structure
#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

/// Global Descriptor Table
#[repr(align(16))]
struct Gdt {
    entries: [GdtEntry; 5],
}

static mut GDT: Gdt = Gdt {
    entries: [
        // Null descriptor
        GdtEntry::null(),
        // Kernel code segment (0x08)
        GdtEntry::new(0, 0xFFFFF, 0x9A, 0xA0),
        // Kernel data segment (0x10)
        GdtEntry::new(0, 0xFFFFF, 0x92, 0xC0),
        // User code segment (0x18)
        GdtEntry::new(0, 0xFFFFF, 0xFA, 0xA0),
        // User data segment (0x20)
        GdtEntry::new(0, 0xFFFFF, 0xF2, 0xC0),
    ],
};

/// Initialize GDT
pub fn init() {
    unsafe {
        let gdt_ptr = GdtPointer {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: &GDT as *const _ as u64,
        };

        // Load GDT
        asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));

        // Reload segment registers
        asm!(
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            out("rax") _,
            options(nostack)
        );
    }
}