//! Pure Rust bootloader for Mach_R
//! ARM64/AArch64 bootloader with UEFI support

use core::arch::global_asm;

pub mod uefi;
pub mod multiboot;
pub mod memory_map;
pub mod device_tree;
pub mod paging;
pub mod trampoline;

// Architecture-specific modules
#[cfg(target_arch = "aarch64")]
pub mod arm64 {
    pub use super::paging::*;
    pub use super::trampoline::*;
}

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

/// Boot information passed to the kernel
#[repr(C)]
#[derive(Debug)]
pub struct BootInfo {
    /// Memory map entries
    pub memory_map: &'static [MemoryMapEntry],
    /// Device tree blob pointer (if available)
    pub device_tree: Option<*const u8>,
    /// Framebuffer information (if graphics available)
    pub framebuffer: Option<FramebufferInfo>,
    /// Boot command line
    pub command_line: &'static str,
    /// Loader name
    pub loader_name: &'static str,
    /// Boot time in milliseconds since epoch
    pub boot_time: u64,
}

/// Memory map entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    /// Memory type
    pub mem_type: MemoryType,
    /// Physical start address
    pub start_addr: u64,
    /// Size in bytes
    pub size: u64,
}

/// Memory region types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    /// Available RAM
    Available,
    /// Reserved by firmware
    Reserved,
    /// ACPI reclaimable
    AcpiReclaimable,
    /// ACPI non-volatile storage
    AcpiNvs,
    /// Bad memory
    BadMemory,
    /// Bootloader reclaimable
    Bootloader,
    /// Kernel code/data
    Kernel,
    /// Device memory
    Device,
    /// Firmware reserved
    Firmware,
}

/// Framebuffer information for graphics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    /// Framebuffer physical address
    pub addr: u64,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Bytes per scanline
    pub pitch: u32,
    /// Bits per pixel
    pub bpp: u32,
    /// Memory model
    pub memory_model: u32,
    /// Red mask size
    pub red_mask_size: u8,
    /// Red mask shift
    pub red_mask_shift: u8,
    /// Green mask size
    pub green_mask_size: u8,
    /// Green mask shift
    pub green_mask_shift: u8,
    /// Blue mask size
    pub blue_mask_size: u8,
    /// Blue mask shift
    pub blue_mask_shift: u8,
}

/// Pixel format types
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    /// 32-bit RGB
    Rgb32,
    /// 32-bit BGR
    Bgr32,
    /// 24-bit RGB
    Rgb24,
    /// 16-bit RGB
    Rgb16,
}

/// Bootloader configuration
pub struct BootloaderConfig {
    /// Kernel physical load address
    pub kernel_load_addr: u64,
    /// Kernel virtual base address
    pub kernel_virtual_base: u64,
    /// Stack size for initial kernel stack
    pub stack_size: u64,
    /// Enable graphics mode
    pub enable_graphics: bool,
    /// Graphics resolution preference
    pub preferred_resolution: (u32, u32),
    /// Enable serial output
    pub enable_serial: bool,
    /// Serial port base address
    pub serial_port: u16,
}

impl Default for BootloaderConfig {
    fn default() -> Self {
        Self {
            kernel_load_addr: 0x80000,        // 512KB
            kernel_virtual_base: 0xFFFFFF8000000000, // Higher half kernel
            stack_size: 0x10000,              // 64KB stack
            enable_graphics: true,
            preferred_resolution: (1024, 768),
            enable_serial: true,
            serial_port: 0x3F8,               // COM1
        }
    }
}

/// Boot protocol interface
pub trait BootProtocol {
    /// Initialize the boot protocol
    fn init() -> Result<Self, BootError>
    where
        Self: Sized;
    
    /// Get memory map from firmware
    fn get_memory_map(&self) -> Result<&[MemoryMapEntry], BootError>;
    
    /// Exit boot services (point of no return)
    fn exit_boot_services(&mut self) -> Result<(), BootError>;
    
    /// Set up graphics mode
    fn setup_graphics(&mut self, config: &BootloaderConfig) -> Result<FramebufferInfo, BootError>;
    
    /// Get device tree (ARM64 specific)
    fn get_device_tree(&self) -> Result<Option<*const u8>, BootError>;
    
    /// Allocate memory for kernel
    fn allocate_kernel_memory(&mut self, size: u64) -> Result<u64, BootError>;
    
    /// Load kernel from storage
    fn load_kernel(&mut self, kernel_data: &[u8], load_addr: u64) -> Result<(), BootError>;
}

/// Boot error types
#[derive(Debug)]
pub enum BootError {
    /// UEFI-specific error
    UefiError(&'static str),
    /// Memory allocation failed
    OutOfMemory,
    /// Invalid kernel format
    InvalidKernel,
    /// Graphics setup failed
    GraphicsError,
    /// Device tree parsing failed
    DeviceTreeError,
    /// I/O error
    IoError,
}

/// Main bootloader entry point
pub fn boot_kernel(config: BootloaderConfig) -> ! {
    // Initialize serial output for debugging
    if config.enable_serial {
        serial_init(config.serial_port);
        serial_println("Mach_R Pure Rust Bootloader v0.1.0");
        
        #[cfg(target_arch = "aarch64")]
        serial_println("Target: ARM64/AArch64");
        #[cfg(target_arch = "x86_64")]
        serial_println("Target: x86_64");
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        serial_println("Target: Unknown");
    }
    
    // Architecture-specific initialization
    arch_init();
    
    // Get memory map from UEFI or use default
    let memory_map = get_memory_map();
    serial_println("Memory map obtained");
    
    // Set up graphics if requested
    let framebuffer = if config.enable_graphics {
        setup_graphics(&config)
    } else {
        None
    };
    
    // Get device tree (ARM64 specific)
    let device_tree = get_device_tree();
    
    // Architecture-specific kernel handoff preparation
    let (page_table_addr, stack_top) = prepare_kernel_handoff(&config, &memory_map);
    
    // Create boot info structure as static
    static mut BOOT_INFO_STORAGE: Option<BootInfo> = None;
    
    unsafe {
        BOOT_INFO_STORAGE = Some(BootInfo {
            memory_map: &memory_map,
            device_tree,
            framebuffer,
            command_line: get_command_line(),
            loader_name: "Mach_R Pure Rust Bootloader",
            boot_time: 0, // TODO: Get actual time
        });
        
        let boot_info = BOOT_INFO_STORAGE.as_ref().unwrap();
        
        serial_println("Jumping to kernel...");
        
        // Architecture-specific kernel jump
        arch_jump_to_kernel(
            config.kernel_load_addr,
            stack_top,
            boot_info,
            page_table_addr,
        );
    }
}

/// Architecture-specific initialization
fn arch_init() {
    #[cfg(target_arch = "aarch64")]
    {
        trampoline::ensure_el1();
        serial_println("ARM64: Running in EL1");
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        match x86_64::init_x86_64() {
            Ok(_) => serial_println("x86_64: Initialization complete"),
            Err(e) => {
                serial_println("x86_64: Initialization failed");
                panic_halt();
            }
        }
    }
}

/// Get memory map from firmware or use default
fn get_memory_map() -> &'static [MemoryMapEntry] {
    use memory_map::DEFAULT_ARM64_MEMORY_MAP;
    // TODO: Add x86_64 specific memory map
    DEFAULT_ARM64_MEMORY_MAP
}

/// Set up graphics based on configuration and architecture
fn setup_graphics(config: &BootloaderConfig) -> Option<FramebufferInfo> {
    #[cfg(target_arch = "aarch64")]
    {
        Some(FramebufferInfo {
            addr: 0xB0000000, // ARM64 common framebuffer address
            width: config.preferred_resolution.0,
            height: config.preferred_resolution.1,
            pitch: config.preferred_resolution.0 * 4,
            bpp: 32,
            memory_model: 1, // RGB
            red_mask_size: 8,
            red_mask_shift: 16,
            green_mask_size: 8,
            green_mask_shift: 8,
            blue_mask_size: 8,
            blue_mask_shift: 0,
        })
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        Some(FramebufferInfo {
            addr: 0xE0000000, // x86_64 common framebuffer address
            width: config.preferred_resolution.0,
            height: config.preferred_resolution.1,
            pitch: config.preferred_resolution.0 * 4,
            bpp: 32,
            memory_model: 1, // RGB
            red_mask_size: 8,
            red_mask_shift: 16,
            green_mask_size: 8,
            green_mask_shift: 8,
            blue_mask_size: 8,
            blue_mask_shift: 0,
        })
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    None
}

/// Get device tree (ARM64 specific)
fn get_device_tree() -> Option<*const u8> {
    #[cfg(target_arch = "aarch64")]
    {
        // TODO: Implement device tree detection for ARM64
        None
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    None
}

/// Get kernel command line
fn get_command_line() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "root=/dev/mmcblk0p1 console=ttyAMA0,115200"
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        "root=/dev/sda1 console=ttyS0,115200"
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "root=/dev/sda1"
    }
}

/// Prepare for kernel handoff (architecture-specific)
fn prepare_kernel_handoff(
    config: &BootloaderConfig,
    memory_map: &[MemoryMapEntry],
) -> (u64, u64) {
    #[cfg(target_arch = "aarch64")]
    {
        trampoline::prepare_kernel_handoff();
        serial_println("ARM64: Prepared for kernel handoff");
        
        let page_table_addr = setup_page_tables_arm64(memory_map, config.kernel_load_addr);
        let stack_top = config.kernel_virtual_base + 0x100000; // 1MB into higher half
        
        serial_println("ARM64: Page tables set up");
        (page_table_addr, stack_top)
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        serial_println("x86_64: Preparing for kernel handoff");
        
        let page_table_addr = setup_page_tables_x86_64(memory_map, config.kernel_load_addr);
        let stack_top = config.kernel_virtual_base + 0x100000; // 1MB into higher half
        
        serial_println("x86_64: Page tables set up");
        (page_table_addr, stack_top)
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        serial_println("Generic: Basic handoff preparation");
        (0, config.kernel_load_addr + 0x100000)
    }
}

/// Architecture-specific kernel jump
fn arch_jump_to_kernel(
    kernel_entry: u64,
    stack_top: u64,
    boot_info: &'static BootInfo,
    page_table_addr: u64,
) -> ! {
    #[cfg(target_arch = "aarch64")]
    {
        trampoline::execute_trampoline(kernel_entry, stack_top, boot_info, page_table_addr);
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        x86_64::execute_trampoline_x86_64(kernel_entry, stack_top, boot_info);
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        // Generic fallback
        unsafe {
            let kernel_entry: extern "C" fn(&'static BootInfo) -> ! = 
                core::mem::transmute(kernel_entry);
            kernel_entry(boot_info);
        }
    }
}

/// Set up ARM64 page tables for higher half kernel
#[cfg(target_arch = "aarch64")]
fn setup_page_tables_arm64(memory_map: &[MemoryMapEntry], kernel_load_addr: u64) -> u64 {
    use paging::*;
    
    // Allocate root page table (this should be done with proper memory allocator)
    static mut ROOT_PAGE_TABLE: PageTable = PageTable::new();
    static mut ALLOCATED_TABLES: [PageTable; 64] = [PageTable::new(); 64];
    static mut TABLE_INDEX: usize = 0;
    
    unsafe {
        let root_table = &mut ROOT_PAGE_TABLE;
        let mut page_manager = PageTableManager::new(root_table);
        
        // Simple table allocator
        let allocate_table = || -> Option<&'static mut PageTable> {
            if TABLE_INDEX < ALLOCATED_TABLES.len() {
                let table = &mut ALLOCATED_TABLES[TABLE_INDEX];
                TABLE_INDEX += 1;
                Some(table)
            } else {
                None
            }
        };
        
        // Identity map first 4GB for bootloader
        let _ = page_manager.identity_map_low_memory(0x100000000, allocate_table);
        
        // Map kernel to higher half
        let kernel_size = 0x1000000; // 16MB kernel space
        let _ = page_manager.map_kernel_higher_half(kernel_load_addr, kernel_size, allocate_table);
        
        // Map device memory (UART, etc.)
        let _ = page_manager.map_device(
            HIGHER_HALF_OFFSET + 0x09000000, // UART virtual address
            0x09000000,                       // UART physical address  
            0x1000,                          // 4KB
            allocate_table,
        );
        
        page_manager.root_address()
    }
}

/// Set up x86_64 page tables for higher half kernel
#[cfg(target_arch = "x86_64")]
fn setup_page_tables_x86_64(memory_map: &[MemoryMapEntry], kernel_load_addr: u64) -> u64 {
    use x86_64::*;
    
    // Allocate PML4 (root page table)
    static mut PML4: PageTable = PageTable::new();
    
    unsafe {
        let pml4 = &mut PML4;
        let mut mem_manager = X86_64MemoryManager::new(pml4);
        
        // Identity map first 4GB for bootloader
        for addr in (0u64..0x100000000).step_by(0x200000) {
            // Use 2MB pages for simplicity
            let _ = mem_manager.map_page(
                addr,
                addr,
                PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE,
            );
        }
        
        // Map kernel to higher half
        let kernel_virt_addr = KERNEL_VIRTUAL_BASE;
        for offset in (0u64..0x1000000).step_by(0x1000) {
            let _ = mem_manager.map_page(
                kernel_virt_addr + offset,
                kernel_load_addr + offset,
                PAGE_PRESENT | PAGE_WRITABLE,
            );
        }
        
        mem_manager.pml4_address()
    }
}

/// Generic page table setup for unsupported architectures
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn setup_page_tables_generic(_memory_map: &[MemoryMapEntry], _kernel_load_addr: u64) -> u64 {
    // Return identity mapping (no paging)
    0
}

/// Initialize serial port for debug output (ARM64 PL011 UART)
fn serial_init(port: u16) {
    unsafe {
        init_pl011_uart();
    }
}

/// Print string to serial port
pub fn serial_println(s: &str) {
    unsafe {
        uart_print_string(s.as_ptr(), s.len());
    }
}

// External assembly functions
extern "C" {
    /// Set up ARM64 page tables and enable MMU
    fn setup_arm64_paging(virtual_base: u64, physical_base: u64);
    
    /// Initialize PL011 UART for debug output
    fn init_pl011_uart();
    
    // Other externs declared here; uart_print_string is declared conditionally below
    
    /// Jump to kernel entry point (defined in assembly)
    fn jump_to_kernel_asm(entry_point: u64, stack_pointer: u64, boot_info: u64) -> !;
}

// Declare uart_print_string only for non-test builds; tests provide a stub
#[cfg(not(test))]
extern "C" { fn uart_print_string(s: *const u8, len: usize); }

// Provide a dummy UART print for tests to satisfy linking
#[cfg(test)]
#[no_mangle]
pub extern "C" fn uart_print_string(_s: *const u8, _len: usize) { }

/// Jump to kernel entry point
unsafe fn jump_to_kernel(entry_point: u64, stack_pointer: u64, boot_info: u64) -> ! {
    jump_to_kernel_asm(entry_point, stack_pointer, boot_info)
}

/// Halt system on panic
fn panic_halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// UEFI boot main function called from UEFI protocol
pub fn boot_main(image_handle: usize, system_table: usize) -> ! {
    let config = BootloaderConfig::default();
    boot_kernel(config)
}

/// Boot sector for UEFI (stub)
#[no_mangle]
pub extern "C" fn efi_main() -> ! {
    let config = BootloaderConfig::default();
    boot_kernel(config)
}

// Assembly stubs for low-level operations
#[cfg(target_arch = "aarch64")]
global_asm!(
    r#"
.text
.global _start
_start:
    /* Boot entry point - UEFI calls this */
    /* Set up minimal environment and jump to Rust */
    
    /* Ensure we're running on the boot processor */
    mrs x0, mpidr_el1
    and x0, x0, #0xFF
    cbnz x0, 1f
    
    /* Set up initial stack (use high memory) */
    mov x0, #0x90000
    mov sp, x0
    
    /* Jump to Rust entry point */
    bl efi_main

1:  /* halt_secondary */
    wfe
    b 1b
    
.global setup_arm64_paging
setup_arm64_paging:
    /* x0 = virtual_base, x1 = physical_base */
    /* TODO: Implement ARM64 page table setup */
    /* For now, identity map everything (MMU disabled mode) */
    
    /* Disable MMU if enabled */
    mrs x2, sctlr_el1
    bic x2, x2, #1
    msr sctlr_el1, x2
    isb
    
    ret
    
.global init_pl011_uart
init_pl011_uart:
    /* Initialize PL011 UART at base address 0x09000000 */
    mov x0, #0x09000000
    
    /* Disable UART */
    str wzr, [x0, #0x30]  /* UARTCR = 0 */
    
    /* Clear any pending errors */
    str wzr, [x0, #0x04]  /* UARTDR error clear */
    
    /* Set baud rate (115200 @ 24MHz clock) */
    mov w1, #13           /* IBRD */
    str w1, [x0, #0x24]   /* UARTIBRD */
    mov w1, #1            /* FBRD */
    str w1, [x0, #0x28]   /* UARTFBRD */
    
    /* Set line control: 8N1 */
    mov w1, #0x70         /* 8-bit, no parity, 1 stop bit, FIFOs enabled */
    str w1, [x0, #0x2C]   /* UARTLCR_H */
    
    /* Enable UART: TXE, RXE, UARTEN */
    mov w1, #0x301
    str w1, [x0, #0x30]   /* UARTCR */
    
    ret
    
.global uart_print_string
uart_print_string:
    /* x0 = string pointer, x1 = length */
    cbz x1, 3f
    mov x2, #0x09000000   /* UART base */
    
2:  /* uart_loop */
    /* Wait for TX FIFO not full */
1:  /* uart_wait */
    ldr w3, [x2, #0x18]   /* UARTFR */
    tst w3, #0x20         /* TXFF bit */
    b.ne 1b
    
    /* Send character */
    ldrb w3, [x0], #1
    str w3, [x2, #0x00]   /* UARTDR */
    
    subs x1, x1, #1
    b.ne 2b
    
3:  /* uart_done */
    ret
    
.global jump_to_kernel_asm
jump_to_kernel_asm:
    /* x0 = entry point, x1 = stack pointer, x2 = boot info pointer */
    
    /* Set up stack pointer */
    mov sp, x1
    
    /* Set up registers for kernel entry */
    mov x0, x2        /* First argument: boot info pointer */
    mov x1, xzr       /* Second argument: reserved (0) */
    mov x2, xzr       /* Third argument: reserved (0) */
    mov x3, xzr       /* Fourth argument: reserved (0) */
    
    /* Clear other registers */
    mov x4, xzr
    mov x5, xzr
    mov x6, xzr
    mov x7, xzr
    
    /* Jump to kernel entry point */
    br x0
    
    /* Should never reach here */
1:  /* kernel_return */
    b 1b
    "#
);

// x86_64 assembly stubs - minimal implementation
#[cfg(target_arch = "x86_64")]
global_asm!(
    r#"
.text
.global _start
_start:
    /* Simple x86_64 entry */
    mov rsp, 0x90000
    call efi_main
1:  hlt
    jmp 1b

.global setup_x86_64_paging
setup_x86_64_paging:
    ret
    
.global jump_to_kernel_asm
jump_to_kernel_asm:
    ret
    "#
);
