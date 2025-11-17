//! Multiboot2 specification support
//! For x86_64 systems (legacy support)

use crate::boot::{BootProtocol, BootError, MemoryMapEntry, MemoryType, FramebufferInfo, BootloaderConfig};

/// Multiboot2 header magic number
pub const MULTIBOOT2_HEADER_MAGIC: u32 = 0xe85250d6;

/// Multiboot2 bootloader magic number
pub const MULTIBOOT2_BOOTLOADER_MAGIC: u32 = 0x36d76289;

/// Multiboot2 header
#[repr(C, packed)]
pub struct Multiboot2Header {
    pub magic: u32,
    pub architecture: u32,
    pub header_length: u32,
    pub checksum: u32,
}

/// Multiboot2 information structure
#[repr(C, packed)]
pub struct Multiboot2Info {
    pub total_size: u32,
    pub reserved: u32,
}

/// Multiboot2 tag header
#[repr(C, packed)]
pub struct Multiboot2Tag {
    pub tag_type: u32,
    pub size: u32,
}

/// Multiboot2 tag types
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Multiboot2TagType {
    End = 0,
    CommandLine = 1,
    BootLoaderName = 2,
    Module = 3,
    BasicMemInfo = 4,
    Bootdev = 5,
    Mmap = 6,
    Vbe = 7,
    Framebuffer = 8,
    ElfSections = 9,
    Apm = 10,
    Efi32 = 11,
    Efi64 = 12,
    Smbios = 13,
    AcpiOld = 14,
    AcpiNew = 15,
    Network = 16,
    EfiMmap = 17,
    EfiBs = 18,
    Efi32Ih = 19,
    Efi64Ih = 20,
    LoadBaseAddr = 21,
}

/// Multiboot2 memory map entry
#[repr(C, packed)]
pub struct Multiboot2MmapEntry {
    pub base_addr: u64,
    pub length: u64,
    pub memory_type: u32,
    pub reserved: u32,
}

/// Multiboot2 memory map tag
#[repr(C, packed)]
pub struct Multiboot2MmapTag {
    pub tag: Multiboot2Tag,
    pub entry_size: u32,
    pub entry_version: u32,
}

/// Multiboot2 framebuffer tag
#[repr(C, packed)]
pub struct Multiboot2FramebufferTag {
    pub tag: Multiboot2Tag,
    pub framebuffer_addr: u64,
    pub framebuffer_pitch: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_bpp: u8,
    pub framebuffer_type: u8,
    pub reserved: u16,
}

/// Multiboot2 framebuffer types
#[repr(u8)]
pub enum Multiboot2FramebufferType {
    Indexed = 0,
    Rgb = 1,
    EgaText = 2,
}

/// Multiboot2 protocol implementation
pub struct Multiboot2Protocol {
    info: *const Multiboot2Info,
}

impl Multiboot2Protocol {
    /// Initialize from multiboot2 information pointer
    pub unsafe fn from_info(info_ptr: *const Multiboot2Info) -> Result<Self, BootError> {
        if info_ptr.is_null() {
            return Err(BootError::UefiError("Null multiboot2 info"));
        }
        
        Ok(Self {
            info: info_ptr,
        })
    }
    
    /// Find a tag by type
    fn find_tag(&self, tag_type: Multiboot2TagType) -> Option<*const Multiboot2Tag> {
        unsafe {
            let info = &*self.info;
            let mut current = (self.info as *const u8).add(8); // Skip total_size and reserved
            let end = (self.info as *const u8).add(info.total_size as usize);
            
            while current < end {
                let tag = current as *const Multiboot2Tag;
                let tag_ref = &*tag;
                
                if tag_ref.tag_type == tag_type as u32 {
                    return Some(tag);
                }
                
                if tag_ref.tag_type == Multiboot2TagType::End as u32 {
                    break;
                }
                
                // Move to next tag (align to 8 bytes)
                let size = (tag_ref.size + 7) & !7;
                current = current.add(size as usize);
            }
            
            None
        }
    }
    
    /// Get memory map from multiboot2
    fn get_multiboot_memory_map(&self) -> Result<&[MemoryMapEntry], BootError> {
        // TODO: Parse multiboot2 memory map
        // This would find the memory map tag and convert entries
        Err(BootError::UefiError("Multiboot2 memory map not implemented"))
    }
    
    /// Get framebuffer information
    fn get_multiboot_framebuffer(&self) -> Result<FramebufferInfo, BootError> {
        let tag_ptr = self.find_tag(Multiboot2TagType::Framebuffer)
            .ok_or(BootError::GraphicsError)?;
            
        unsafe {
            let fb_tag = tag_ptr as *const Multiboot2FramebufferTag;
            let fb = &*fb_tag;
            
            Ok(FramebufferInfo {
                addr: fb.framebuffer_addr,
                width: fb.framebuffer_width,
                height: fb.framebuffer_height,
                bpp: fb.framebuffer_bpp as u32,
                pitch: fb.framebuffer_pitch,
                memory_model: 1, // RGB
                red_mask_size: 8,
                red_mask_shift: 16,
                green_mask_size: 8,
                green_mask_shift: 8,
                blue_mask_size: 8,
                blue_mask_shift: 0,
            })
        }
    }
}

impl BootProtocol for Multiboot2Protocol {
    fn init() -> Result<Self, BootError> {
        // Multiboot2 protocol is initialized externally by bootloader
        Err(BootError::UefiError("Multiboot2 requires external initialization"))
    }
    
    fn get_memory_map(&self) -> Result<&[MemoryMapEntry], BootError> {
        self.get_multiboot_memory_map()
    }
    
    fn exit_boot_services(&mut self) -> Result<(), BootError> {
        // Multiboot2 doesn't have boot services to exit
        Ok(())
    }
    
    fn setup_graphics(&mut self, config: &BootloaderConfig) -> Result<FramebufferInfo, BootError> {
        if !config.enable_graphics {
            return Err(BootError::GraphicsError);
        }
        
        self.get_multiboot_framebuffer()
    }
    
    fn get_device_tree(&self) -> Result<Option<*const u8>, BootError> {
        // x86_64 doesn't typically use device trees
        Ok(None)
    }
    
    fn allocate_kernel_memory(&mut self, size: u64) -> Result<u64, BootError> {
        // Memory allocation not available in multiboot2
        Err(BootError::OutOfMemory)
    }
    
    fn load_kernel(&mut self, kernel_data: &[u8], load_addr: u64) -> Result<(), BootError> {
        // Kernel loading handled by multiboot2 bootloader
        Ok(())
    }
}

/// Convert multiboot2 memory type to our memory type
fn convert_multiboot_memory_type(mb_type: u32) -> MemoryType {
    match mb_type {
        1 => MemoryType::Available,        // Available
        3 => MemoryType::AcpiReclaimable, // ACPI reclaimable
        4 => MemoryType::AcpiNvs,         // ACPI NVS
        5 => MemoryType::BadMemory,       // Bad memory
        _ => MemoryType::Reserved,        // Reserved or unknown
    }
}

/// Generate multiboot2 header for kernel binary
pub const fn generate_multiboot2_header() -> [u8; 32] {
    let magic = MULTIBOOT2_HEADER_MAGIC;
    let arch = 0; // i386 (also works for x86_64)
    let length = 32;
    let checksum = 0u32.wrapping_sub(magic).wrapping_sub(arch).wrapping_sub(length);
    
    let mut header = [0u8; 32];
    
    // Magic
    header[0] = (magic & 0xff) as u8;
    header[1] = ((magic >> 8) & 0xff) as u8;
    header[2] = ((magic >> 16) & 0xff) as u8;
    header[3] = ((magic >> 24) & 0xff) as u8;
    
    // Architecture
    header[4] = (arch & 0xff) as u8;
    header[5] = ((arch >> 8) & 0xff) as u8;
    header[6] = ((arch >> 16) & 0xff) as u8;
    header[7] = ((arch >> 24) & 0xff) as u8;
    
    // Header length
    header[8] = (length & 0xff) as u8;
    header[9] = ((length >> 8) & 0xff) as u8;
    header[10] = ((length >> 16) & 0xff) as u8;
    header[11] = ((length >> 24) & 0xff) as u8;
    
    // Checksum
    header[12] = (checksum & 0xff) as u8;
    header[13] = ((checksum >> 8) & 0xff) as u8;
    header[14] = ((checksum >> 16) & 0xff) as u8;
    header[15] = ((checksum >> 24) & 0xff) as u8;
    
    // End tag
    header[16] = 0; // type = 0 (end)
    header[17] = 0;
    header[18] = 0;
    header[19] = 0;
    header[20] = 8; // size = 8
    header[21] = 0;
    header[22] = 0;
    header[23] = 0;
    
    // Padding to 32 bytes
    header
}