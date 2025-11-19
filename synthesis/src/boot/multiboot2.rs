//! Multiboot2 information structure parser
//! Complete implementation of Multiboot2 boot protocol parsing
//!
//! This module provides type-safe parsing of the Multiboot2 information
//! structure passed by the bootloader.

use core::fmt;
use core::slice;
use core::str;

/// Multiboot2 bootloader magic number (passed in EAX/RAX)
pub const MULTIBOOT2_BOOTLOADER_MAGIC: u32 = 0x36d76289;

/// Multiboot2 information structure
#[repr(C, packed)]
pub struct Multiboot2Info {
    pub total_size: u32,
    pub reserved: u32,
}

/// Multiboot2 tag header
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Multiboot2Tag {
    pub tag_type: u32,
    pub size: u32,
}

/// Multiboot2 tag types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    End = 0,
    CommandLine = 1,
    BootLoaderName = 2,
    Module = 3,
    BasicMemInfo = 4,
    BootDev = 5,
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

impl TagType {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(TagType::End),
            1 => Some(TagType::CommandLine),
            2 => Some(TagType::BootLoaderName),
            3 => Some(TagType::Module),
            4 => Some(TagType::BasicMemInfo),
            5 => Some(TagType::BootDev),
            6 => Some(TagType::Mmap),
            7 => Some(TagType::Vbe),
            8 => Some(TagType::Framebuffer),
            9 => Some(TagType::ElfSections),
            10 => Some(TagType::Apm),
            11 => Some(TagType::Efi32),
            12 => Some(TagType::Efi64),
            13 => Some(TagType::Smbios),
            14 => Some(TagType::AcpiOld),
            15 => Some(TagType::AcpiNew),
            16 => Some(TagType::Network),
            17 => Some(TagType::EfiMmap),
            18 => Some(TagType::EfiBs),
            19 => Some(TagType::Efi32Ih),
            20 => Some(TagType::Efi64Ih),
            21 => Some(TagType::LoadBaseAddr),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TagType::End => "End",
            TagType::CommandLine => "Command Line",
            TagType::BootLoaderName => "Boot Loader Name",
            TagType::Module => "Module",
            TagType::BasicMemInfo => "Basic Memory Info",
            TagType::BootDev => "Boot Device",
            TagType::Mmap => "Memory Map",
            TagType::Vbe => "VBE Info",
            TagType::Framebuffer => "Framebuffer",
            TagType::ElfSections => "ELF Sections",
            TagType::Apm => "APM",
            TagType::Efi32 => "EFI 32-bit",
            TagType::Efi64 => "EFI 64-bit",
            TagType::Smbios => "SMBIOS",
            TagType::AcpiOld => "ACPI (old)",
            TagType::AcpiNew => "ACPI (new)",
            TagType::Network => "Network",
            TagType::EfiMmap => "EFI Memory Map",
            TagType::EfiBs => "EFI Boot Services",
            TagType::Efi32Ih => "EFI 32-bit Image Handle",
            TagType::Efi64Ih => "EFI 64-bit Image Handle",
            TagType::LoadBaseAddr => "Load Base Address",
        }
    }
}

/// String tag (for command line, bootloader name, etc.)
#[repr(C, packed)]
pub struct StringTag {
    pub tag: Multiboot2Tag,
    // Followed by null-terminated string
}

impl StringTag {
    pub fn string(&self) -> &str {
        unsafe {
            let start = (self as *const Self as *const u8).add(8);
            let len = self.tag.size as usize - 8 - 1; // -8 for header, -1 for null terminator
            let bytes = slice::from_raw_parts(start, len);
            str::from_utf8_unchecked(bytes)
        }
    }
}

/// Basic memory info tag
#[repr(C, packed)]
pub struct BasicMemInfoTag {
    pub tag: Multiboot2Tag,
    pub mem_lower: u32,  // KB of lower memory
    pub mem_upper: u32,  // KB of upper memory
}

/// Memory map entry
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MmapEntry {
    pub base_addr: u64,
    pub length: u64,
    pub mem_type: u32,
    pub reserved: u32,
}

impl MmapEntry {
    pub fn type_name(&self) -> &'static str {
        match self.mem_type {
            1 => "Available",
            2 => "Reserved",
            3 => "ACPI Reclaimable",
            4 => "ACPI NVS",
            5 => "Bad Memory",
            _ => "Unknown",
        }
    }
}

/// Memory map tag
#[repr(C, packed)]
pub struct MmapTag {
    pub tag: Multiboot2Tag,
    pub entry_size: u32,
    pub entry_version: u32,
    // Followed by memory map entries
}

impl MmapTag {
    pub fn entries(&self) -> MmapIter {
        unsafe {
            let start = (self as *const Self as *const u8).add(16);
            let count = (self.tag.size as usize - 16) / self.entry_size as usize;
            MmapIter {
                current: start as *const MmapEntry,
                remaining: count,
                entry_size: self.entry_size as usize,
            }
        }
    }
}

/// Iterator over memory map entries
pub struct MmapIter {
    current: *const MmapEntry,
    remaining: usize,
    entry_size: usize,
}

impl Iterator for MmapIter {
    type Item = MmapEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            let entry = *self.current;
            self.current = (self.current as *const u8).add(self.entry_size) as *const MmapEntry;
            self.remaining -= 1;
            Some(entry)
        }
    }
}

/// Module tag
#[repr(C, packed)]
pub struct ModuleTag {
    pub tag: Multiboot2Tag,
    pub mod_start: u32,
    pub mod_end: u32,
    // Followed by null-terminated module string
}

impl ModuleTag {
    pub fn name(&self) -> &str {
        unsafe {
            let start = (self as *const Self as *const u8).add(16);
            let len = self.tag.size as usize - 16 - 1; // -16 for header, -1 for null
            let bytes = slice::from_raw_parts(start, len);
            str::from_utf8_unchecked(bytes)
        }
    }
}

/// Framebuffer tag
#[repr(C, packed)]
pub struct FramebufferTag {
    pub tag: Multiboot2Tag,
    pub framebuffer_addr: u64,
    pub framebuffer_pitch: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_bpp: u8,
    pub framebuffer_type: u8,
    pub reserved: u16,
    // Followed by color info
}

impl FramebufferTag {
    pub fn size_bytes(&self) -> u64 {
        self.framebuffer_pitch as u64 * self.framebuffer_height as u64
    }

    pub fn type_name(&self) -> &'static str {
        match self.framebuffer_type {
            0 => "Indexed",
            1 => "RGB",
            2 => "EGA Text",
            _ => "Unknown",
        }
    }
}

/// ELF sections tag
#[repr(C, packed)]
pub struct ElfSectionsTag {
    pub tag: Multiboot2Tag,
    pub num: u32,
    pub entsize: u32,
    pub shndx: u32,
    // Followed by ELF section headers
}

/// Load base address tag
#[repr(C, packed)]
pub struct LoadBaseAddrTag {
    pub tag: Multiboot2Tag,
    pub load_base_addr: u32,
}

/// Multiboot2 information parser
pub struct Multiboot2InfoParser {
    info_addr: u64,
}

impl Multiboot2InfoParser {
    /// Create a new parser from the multiboot info address
    ///
    /// # Safety
    /// The caller must ensure that info_addr points to a valid Multiboot2 info structure
    pub unsafe fn new(info_addr: u64) -> Option<Self> {
        if info_addr == 0 {
            return None;
        }

        // Verify alignment (must be 8-byte aligned)
        if info_addr & 0x7 != 0 {
            return None;
        }

        Some(Self { info_addr })
    }

    /// Get the total size of the multiboot info structure
    pub fn total_size(&self) -> u32 {
        unsafe {
            let info = self.info_addr as *const Multiboot2Info;
            (*info).total_size
        }
    }

    /// Iterate over all tags
    pub fn tags(&self) -> TagIter {
        unsafe {
            let start = self.info_addr + 8; // Skip total_size and reserved
            let end = self.info_addr + self.total_size() as u64;
            TagIter {
                current: start,
                end,
            }
        }
    }

    /// Find a specific tag by type
    pub fn find_tag(&self, tag_type: TagType) -> Option<*const Multiboot2Tag> {
        for tag in self.tags() {
            if tag.tag_type == tag_type as u32 {
                return Some(tag as *const Multiboot2Tag);
            }
        }
        None
    }

    /// Get command line string
    pub fn command_line(&self) -> Option<&str> {
        self.find_tag(TagType::CommandLine).map(|tag| unsafe {
            let string_tag = tag as *const StringTag;
            (*string_tag).string()
        })
    }

    /// Get bootloader name
    pub fn bootloader_name(&self) -> Option<&str> {
        self.find_tag(TagType::BootLoaderName).map(|tag| unsafe {
            let string_tag = tag as *const StringTag;
            (*string_tag).string()
        })
    }

    /// Get basic memory info
    pub fn basic_memory_info(&self) -> Option<(u32, u32)> {
        self.find_tag(TagType::BasicMemInfo).map(|tag| unsafe {
            let mem_tag = tag as *const BasicMemInfoTag;
            ((*mem_tag).mem_lower, (*mem_tag).mem_upper)
        })
    }

    /// Get memory map
    pub fn memory_map(&self) -> Option<MmapIter> {
        self.find_tag(TagType::Mmap).map(|tag| unsafe {
            let mmap_tag = tag as *const MmapTag;
            (*mmap_tag).entries()
        })
    }

    /// Get framebuffer info
    pub fn framebuffer(&self) -> Option<&FramebufferTag> {
        self.find_tag(TagType::Framebuffer).map(|tag| unsafe {
            &*(tag as *const FramebufferTag)
        })
    }

    /// Get ELF sections
    pub fn elf_sections(&self) -> Option<&ElfSectionsTag> {
        self.find_tag(TagType::ElfSections).map(|tag| unsafe {
            &*(tag as *const ElfSectionsTag)
        })
    }

    /// Get load base address
    pub fn load_base_addr(&self) -> Option<u32> {
        self.find_tag(TagType::LoadBaseAddr).map(|tag| unsafe {
            let addr_tag = tag as *const LoadBaseAddrTag;
            (*addr_tag).load_base_addr
        })
    }
}

/// Iterator over Multiboot2 tags
pub struct TagIter {
    current: u64,
    end: u64,
}

impl Iterator for TagIter {
    type Item = Multiboot2Tag;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }

        unsafe {
            let tag_ptr = self.current as *const Multiboot2Tag;
            let tag = *tag_ptr;

            // Check for end tag
            if tag.tag_type == TagType::End as u32 {
                return None;
            }

            // Move to next tag (8-byte aligned)
            let size = (tag.size + 7) & !7;
            self.current += size as u64;

            Some(tag)
        }
    }
}

/// Verify Multiboot2 magic number
pub fn verify_magic(magic: u32) -> bool {
    magic == MULTIBOOT2_BOOTLOADER_MAGIC
}

/// Format a memory size in human-readable form
pub fn format_memory_size(kb: u32) -> (u32, &'static str) {
    if kb >= 1024 * 1024 {
        ((kb / (1024 * 1024)), "GB")
    } else if kb >= 1024 {
        ((kb / 1024), "MB")
    } else {
        (kb, "KB")
    }
}
