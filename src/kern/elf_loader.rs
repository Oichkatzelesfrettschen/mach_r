//! ELF Binary Loader
//!
//! Based on ELF64 specification and Mach4 loader patterns.
//!
//! This module provides functionality to load ELF64 binaries into
//! a task's address space for execution.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::kern::task::Task;
use crate::mach_vm::vm_map::{VmInherit, VmMap, VmMapId, VmProt};
use crate::mach_vm::{vm_map, PAGE_SIZE};

// ============================================================================
// ELF Constants
// ============================================================================

/// ELF magic bytes
pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// ELF class - 64-bit
pub const ELFCLASS64: u8 = 2;

/// ELF data encoding - little endian
pub const ELFDATA2LSB: u8 = 1;
/// ELF data encoding - big endian
pub const ELFDATA2MSB: u8 = 2;

/// ELF type - executable
pub const ET_EXEC: u16 = 2;
/// ELF type - shared object (position independent)
pub const ET_DYN: u16 = 3;

/// ELF machine - x86_64
pub const EM_X86_64: u16 = 62;
/// ELF machine - AArch64
pub const EM_AARCH64: u16 = 183;

/// Program header type - loadable segment
pub const PT_LOAD: u32 = 1;
/// Program header type - dynamic linking info
pub const PT_DYNAMIC: u32 = 2;
/// Program header type - interpreter path
pub const PT_INTERP: u32 = 3;
/// Program header type - note section
pub const PT_NOTE: u32 = 4;
/// Program header type - thread-local storage
pub const PT_TLS: u32 = 7;
/// Program header type - GNU stack
pub const PT_GNU_STACK: u32 = 0x6474e551;

/// Program header flags - execute
pub const PF_X: u32 = 1;
/// Program header flags - write
pub const PF_W: u32 = 2;
/// Program header flags - read
pub const PF_R: u32 = 4;

// ============================================================================
// ELF Header Structures
// ============================================================================

/// ELF64 File Header
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Elf64Header {
    /// Magic number and file class
    pub e_ident: [u8; 16],
    /// Object file type
    pub e_type: u16,
    /// Machine type
    pub e_machine: u16,
    /// Object file version
    pub e_version: u32,
    /// Entry point address
    pub e_entry: u64,
    /// Program header offset
    pub e_phoff: u64,
    /// Section header offset
    pub e_shoff: u64,
    /// Processor-specific flags
    pub e_flags: u32,
    /// ELF header size
    pub e_ehsize: u16,
    /// Program header entry size
    pub e_phentsize: u16,
    /// Number of program header entries
    pub e_phnum: u16,
    /// Section header entry size
    pub e_shentsize: u16,
    /// Number of section header entries
    pub e_shnum: u16,
    /// Section name string table index
    pub e_shstrndx: u16,
}

impl Elf64Header {
    /// Check if this is a valid ELF64 header
    pub fn is_valid(&self) -> bool {
        self.e_ident[0..4] == ELF_MAGIC && self.e_ident[4] == ELFCLASS64
    }

    /// Check if ELF is little endian
    pub fn is_little_endian(&self) -> bool {
        self.e_ident[5] == ELFDATA2LSB
    }

    /// Check if ELF is big endian
    pub fn is_big_endian(&self) -> bool {
        self.e_ident[5] == ELFDATA2MSB
    }

    /// Check if this is an executable or shared object
    pub fn is_executable(&self) -> bool {
        self.e_type == ET_EXEC || self.e_type == ET_DYN
    }

    /// Check if the machine type is supported
    pub fn is_supported_machine(&self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            self.e_machine == EM_X86_64
        }
        #[cfg(target_arch = "aarch64")]
        {
            self.e_machine == EM_AARCH64
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            false
        }
    }

    /// Get entry point address
    pub fn entry_point(&self) -> u64 {
        self.e_entry
    }
}

/// ELF64 Program Header
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Elf64ProgramHeader {
    /// Segment type
    pub p_type: u32,
    /// Segment flags
    pub p_flags: u32,
    /// Offset in file
    pub p_offset: u64,
    /// Virtual address in memory
    pub p_vaddr: u64,
    /// Physical address (usually same as vaddr)
    pub p_paddr: u64,
    /// Size in file
    pub p_filesz: u64,
    /// Size in memory
    pub p_memsz: u64,
    /// Segment alignment
    pub p_align: u64,
}

impl Elf64ProgramHeader {
    /// Check if this is a loadable segment
    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }

    /// Convert ELF flags to VM protection
    pub fn to_vm_prot(&self) -> VmProt {
        let mut prot = VmProt::empty();
        if (self.p_flags & PF_R) != 0 {
            prot |= VmProt::READ;
        }
        if (self.p_flags & PF_W) != 0 {
            prot |= VmProt::WRITE;
        }
        if (self.p_flags & PF_X) != 0 {
            prot |= VmProt::EXECUTE;
        }
        prot
    }
}

// ============================================================================
// ELF Loader Errors
// ============================================================================

/// ELF loading error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    /// Invalid ELF magic number
    InvalidMagic,
    /// Not a 64-bit ELF
    Not64Bit,
    /// Unsupported machine type
    UnsupportedMachine,
    /// Not an executable
    NotExecutable,
    /// Invalid program header
    InvalidProgramHeader,
    /// Memory mapping failed
    MappingFailed,
    /// Binary too large
    TooLarge,
    /// Invalid endianness for platform
    InvalidEndian,
}

/// ELF loading result
pub type ElfResult<T> = Result<T, ElfError>;

// ============================================================================
// Loaded Binary Info
// ============================================================================

/// Information about a loaded ELF binary
#[derive(Debug, Clone)]
pub struct LoadedBinary {
    /// Entry point address
    pub entry_point: u64,
    /// Base address where binary was loaded
    pub base_address: u64,
    /// End of loaded segments
    pub end_address: u64,
    /// Stack pointer (if stack was allocated)
    pub stack_pointer: Option<u64>,
    /// Number of segments loaded
    pub segments_loaded: usize,
    /// Interpreter path (if dynamic)
    pub interpreter: Option<String>,
}

// ============================================================================
// ELF Loader
// ============================================================================

/// ELF64 Binary Loader
pub struct ElfLoader;

impl ElfLoader {
    /// Parse ELF header from binary data
    pub fn parse_header(data: &[u8]) -> ElfResult<Elf64Header> {
        if data.len() < core::mem::size_of::<Elf64Header>() {
            return Err(ElfError::TooLarge);
        }

        // Safety: We've verified the length
        let header = unsafe { *(data.as_ptr() as *const Elf64Header) };

        // Validate header
        if !header.is_valid() {
            return Err(ElfError::InvalidMagic);
        }

        // Check machine type
        if !header.is_supported_machine() {
            return Err(ElfError::UnsupportedMachine);
        }

        // Verify executable type
        if !header.is_executable() {
            return Err(ElfError::NotExecutable);
        }

        // Check endianness matches platform
        #[cfg(target_endian = "little")]
        if !header.is_little_endian() {
            return Err(ElfError::InvalidEndian);
        }
        #[cfg(target_endian = "big")]
        if !header.is_big_endian() {
            return Err(ElfError::InvalidEndian);
        }

        Ok(header)
    }

    /// Parse program headers from binary data
    pub fn parse_program_headers(
        data: &[u8],
        header: &Elf64Header,
    ) -> ElfResult<Vec<Elf64ProgramHeader>> {
        let phoff = header.e_phoff as usize;
        let phentsize = header.e_phentsize as usize;
        let phnum = header.e_phnum as usize;

        if phoff + phnum * phentsize > data.len() {
            return Err(ElfError::InvalidProgramHeader);
        }

        let mut phdrs = Vec::with_capacity(phnum);

        for i in 0..phnum {
            let offset = phoff + i * phentsize;
            let phdr = unsafe { *(data.as_ptr().add(offset) as *const Elf64ProgramHeader) };
            phdrs.push(phdr);
        }

        Ok(phdrs)
    }

    /// Load ELF binary into a VM map
    ///
    /// Returns information about the loaded binary including entry point.
    pub fn load_into_map(data: &[u8], map: &Arc<VmMap>) -> ElfResult<LoadedBinary> {
        // Parse header
        let header = Self::parse_header(data)?;

        // Parse program headers
        let phdrs = Self::parse_program_headers(data, &header)?;

        let mut base_address = u64::MAX;
        let mut end_address = 0u64;
        let mut segments_loaded = 0;
        let mut interpreter = None;

        // Process each program header
        for phdr in &phdrs {
            match phdr.p_type {
                PT_LOAD => {
                    // Load segment into memory
                    Self::load_segment(data, phdr, map)?;

                    // Track address range
                    if phdr.p_vaddr < base_address {
                        base_address = phdr.p_vaddr;
                    }
                    let seg_end = phdr.p_vaddr + phdr.p_memsz;
                    if seg_end > end_address {
                        end_address = seg_end;
                    }

                    segments_loaded += 1;
                }
                PT_INTERP => {
                    // Extract interpreter path
                    let offset = phdr.p_offset as usize;
                    let size = phdr.p_filesz as usize;
                    if offset + size <= data.len() {
                        let path_bytes = &data[offset..offset + size];
                        // Remove null terminator if present
                        let path_len = path_bytes.iter().position(|&b| b == 0).unwrap_or(size);
                        if let Ok(path) = core::str::from_utf8(&path_bytes[..path_len]) {
                            interpreter = Some(String::from(path));
                        }
                    }
                }
                _ => {
                    // Ignore other segment types for now
                }
            }
        }

        // Allocate user stack
        let stack_size = 0x100000u64; // 1MB stack
        let stack_top = 0x7FFF_FFFF_F000u64;
        let stack_bottom = stack_top - stack_size;

        let stack_allocated = map
            .enter(
                stack_bottom,
                stack_top,
                None, // Anonymous memory
                0,
                VmProt::READ | VmProt::WRITE,
                VmProt::ALL,
                VmInherit::Copy,
            )
            .is_ok();

        let stack_pointer = if stack_allocated {
            Some(stack_top - 8) // Align to 8 bytes, leave space for return address
        } else {
            None
        };

        Ok(LoadedBinary {
            entry_point: header.entry_point(),
            base_address,
            end_address,
            stack_pointer,
            segments_loaded,
            interpreter,
        })
    }

    /// Load a single segment into the VM map
    fn load_segment(
        _data: &[u8],
        phdr: &Elf64ProgramHeader,
        map: &Arc<VmMap>,
    ) -> ElfResult<()> {
        if phdr.p_memsz == 0 {
            return Ok(()); // Nothing to load
        }

        // Calculate page-aligned addresses
        let vaddr = phdr.p_vaddr;
        let page_offset = vaddr & (PAGE_SIZE as u64 - 1);
        let page_start = vaddr - page_offset;
        let total_size = ((phdr.p_memsz + page_offset + PAGE_SIZE as u64 - 1)
            / PAGE_SIZE as u64)
            * PAGE_SIZE as u64;

        // Get protection
        let prot = phdr.to_vm_prot();

        // Enter mapping in VM map
        map.enter(
            page_start,
            page_start + total_size,
            None, // Anonymous memory, will be faulted in
            0,
            prot,
            VmProt::ALL,
            VmInherit::Copy,
        )
        .map_err(|_| ElfError::MappingFailed)?;

        // In a real implementation, we would copy the segment data
        // into the allocated pages. For now, we just allocate the space.
        //
        // The actual data copy would be:
        // 1. Get physical pages backing the VM region
        // 2. Copy data from file offset to the pages
        // 3. Zero fill any BSS (memsz > filesz)

        // Note: File data would be copied from:
        //   data[phdr.p_offset..phdr.p_offset + phdr.p_filesz]
        // To virtual address:
        //   phdr.p_vaddr
        // And zero-fill from:
        //   phdr.p_vaddr + phdr.p_filesz to phdr.p_vaddr + phdr.p_memsz

        Ok(())
    }

    /// Load ELF binary into a task
    pub fn load_into_task(data: &[u8], task: &Arc<Task>) -> ElfResult<LoadedBinary> {
        // Get or create task's VM map
        let map_id = task.get_map_id().ok_or(ElfError::MappingFailed)?;
        let map = vm_map::lookup(map_id).ok_or(ElfError::MappingFailed)?;

        Self::load_into_map(data, &map)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validate ELF binary without loading
pub fn validate_elf(data: &[u8]) -> ElfResult<Elf64Header> {
    ElfLoader::parse_header(data)
}

/// Load ELF binary and return entry point
pub fn load_elf(data: &[u8], task: &Arc<Task>) -> ElfResult<LoadedBinary> {
    ElfLoader::load_into_task(data, task)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_magic() {
        assert_eq!(ELF_MAGIC, [0x7f, b'E', b'L', b'F']);
    }

    #[test]
    fn test_invalid_elf() {
        let bad_data = [0u8; 64];
        assert_eq!(
            ElfLoader::parse_header(&bad_data),
            Err(ElfError::InvalidMagic)
        );
    }

    #[test]
    fn test_program_header_flags() {
        let phdr = Elf64ProgramHeader {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: 0,
            p_vaddr: 0x400000,
            p_paddr: 0x400000,
            p_filesz: 0x1000,
            p_memsz: 0x1000,
            p_align: 0x1000,
        };

        let prot = phdr.to_vm_prot();
        assert!(prot.contains(VmProt::READ));
        assert!(prot.contains(VmProt::EXECUTE));
        assert!(!prot.contains(VmProt::WRITE));
    }
}
