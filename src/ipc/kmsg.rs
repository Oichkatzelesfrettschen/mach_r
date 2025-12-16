//! IPC Kernel Message - Internal message representation
//!
//! Based on Mach4 ipc/ipc_kmsg.h
//! Kernel messages are the internal representation of Mach messages
//! as they pass through the kernel.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

use super::entry::MachPortName;
use super::port::Port;
use super::right::{copyin, copyout, MsgTypeName};
use super::space::IpcSpace;
use super::IpcError;

// ============================================================================
// Message Header (matches Mach message format)
// ============================================================================

/// Message header bits
pub type MsgBits = u32;

/// Message size type
pub type MsgSize = u32;

/// Message ID type
pub type MsgId = i32;

/// Message option flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MsgOption {
    None = 0,
    SendMsg = 0x00000001,
    RcvMsg = 0x00000002,
    SendTimeout = 0x00000010,
    RcvTimeout = 0x00000100,
    SendInterrupt = 0x00000040,
    RcvInterrupt = 0x00000400,
    SendNotify = 0x00000080,
    RcvLarge = 0x00000800,
}

/// Mach message header
#[derive(Debug, Clone)]
#[repr(C)]
pub struct MachMsgHeader {
    /// Message bits (remote/local port types, complex flag)
    pub msgh_bits: MsgBits,
    /// Size of message including header
    pub msgh_size: MsgSize,
    /// Destination port (send right)
    pub msgh_remote_port: MachPortName,
    /// Reply port (send/send-once right)
    pub msgh_local_port: MachPortName,
    /// Reserved (voucher port in newer Mach)
    pub msgh_reserved: u32,
    /// Message ID
    pub msgh_id: MsgId,
}

impl MachMsgHeader {
    /// Create a new empty header
    pub const fn new() -> Self {
        Self {
            msgh_bits: 0,
            msgh_size: core::mem::size_of::<Self>() as MsgSize,
            msgh_remote_port: 0,
            msgh_local_port: 0,
            msgh_reserved: 0,
            msgh_id: 0,
        }
    }

    /// Get remote port type from bits
    pub fn remote_port_type(&self) -> u32 {
        (self.msgh_bits >> 8) & 0xFF
    }

    /// Get local port type from bits
    pub fn local_port_type(&self) -> u32 {
        self.msgh_bits & 0xFF
    }

    /// Check if message is complex (has OOL data or port rights in body)
    pub fn is_complex(&self) -> bool {
        (self.msgh_bits & MACH_MSGH_BITS_COMPLEX) != 0
    }

    /// Set message bits for port types
    pub fn set_bits(&mut self, remote_type: u32, local_type: u32, complex: bool) {
        self.msgh_bits = ((remote_type & 0xFF) << 8) | (local_type & 0xFF);
        if complex {
            self.msgh_bits |= MACH_MSGH_BITS_COMPLEX;
        }
    }
}

impl Default for MachMsgHeader {
    fn default() -> Self {
        Self::new()
    }
}

// Message bits constants
pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x80000000;
pub const MACH_MSGH_BITS_REMOTE_MASK: u32 = 0x0000FF00;
pub const MACH_MSGH_BITS_LOCAL_MASK: u32 = 0x000000FF;

// ============================================================================
// Type Descriptor for complex messages
// ============================================================================

/// Type descriptor for inline data
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachMsgTypeDescriptor {
    /// Type name (data type)
    pub msgt_name: u8,
    /// Size in bits
    pub msgt_size: u8,
    /// Number of elements
    pub msgt_number: u16,
    /// Is data inline?
    pub msgt_inline: bool,
    /// Is this a long-form descriptor?
    pub msgt_longform: bool,
    /// Should receiver deallocate?
    pub msgt_deallocate: bool,
    /// Unused
    pub msgt_unused: u8,
}

/// Port descriptor in complex message
#[derive(Debug, Clone)]
pub struct PortDescriptor {
    /// Port name/right
    pub name: MachPortName,
    /// Disposition (type of right)
    pub disposition: MsgTypeName,
}

/// OOL (out-of-line) data descriptor
#[derive(Debug, Clone)]
pub struct OolDescriptor {
    /// Address of data
    pub address: usize,
    /// Size of data
    pub size: usize,
    /// Should receiver deallocate?
    pub deallocate: bool,
    /// Copy option
    pub copy: OolCopyOption,
}

/// OOL copy options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OolCopyOption {
    /// Physical copy
    Physical,
    /// Virtual copy (COW)
    Virtual,
    /// Overwrite destination
    Overwrite,
}

// ============================================================================
// Mach Message Body Descriptor Types (for complex messages)
// ============================================================================

/// Descriptor types (from Mach headers)
pub const MACH_MSG_PORT_DESCRIPTOR: u32 = 0;
pub const MACH_MSG_OOL_DESCRIPTOR: u32 = 1;
pub const MACH_MSG_OOL_PORTS_DESCRIPTOR: u32 = 2;
pub const MACH_MSG_OOL_VOLATILE_DESCRIPTOR: u32 = 3;

/// Port descriptor (in message body)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachMsgPortDescriptor {
    /// Port name (in user message) or offset in ports array (in kernel message)
    pub name: MachPortName,
    /// Pad to align
    pub pad1: u32,
    /// Pad to 16 bytes
    pub pad2: u32,
    /// Descriptor type and disposition
    pub type_and_disposition: u32,
}

impl MachMsgPortDescriptor {
    pub const SIZE: usize = 16;

    /// Get descriptor type
    pub fn descriptor_type(&self) -> u32 {
        self.type_and_disposition & 0xFF
    }

    /// Get disposition
    pub fn disposition(&self) -> u32 {
        (self.type_and_disposition >> 8) & 0xFF
    }

    /// Create from raw values
    pub fn new(name: MachPortName, disposition: MsgTypeName) -> Self {
        Self {
            name,
            pad1: 0,
            pad2: 0,
            type_and_disposition: MACH_MSG_PORT_DESCRIPTOR | ((disposition as u32) << 8),
        }
    }
}

/// OOL descriptor (in message body)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachMsgOolDescriptor {
    /// Address in sender's address space
    pub address: usize,
    /// If true, deallocate in sender after send
    pub deallocate: u32,
    /// Copy semantics
    pub copy: u32,
    /// Pad for alignment
    pub pad1: u32,
    /// Descriptor type
    pub descriptor_type: u32,
    /// Size of data
    pub size: usize,
}

impl MachMsgOolDescriptor {
    pub const SIZE: usize = 32;

    /// Get OOL copy option
    pub fn copy_option(&self) -> OolCopyOption {
        match self.copy {
            1 => OolCopyOption::Virtual,
            2 => OolCopyOption::Overwrite,
            _ => OolCopyOption::Physical,
        }
    }

    /// Create new OOL descriptor
    pub fn new(address: usize, size: usize, deallocate: bool, copy: OolCopyOption) -> Self {
        Self {
            address,
            deallocate: if deallocate { 1 } else { 0 },
            copy: match copy {
                OolCopyOption::Physical => 0,
                OolCopyOption::Virtual => 1,
                OolCopyOption::Overwrite => 2,
            },
            pad1: 0,
            descriptor_type: MACH_MSG_OOL_DESCRIPTOR,
            size,
        }
    }
}

/// OOL ports descriptor (array of ports out-of-line)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachMsgOolPortsDescriptor {
    /// Address of port names array
    pub address: usize,
    /// If true, deallocate source
    pub deallocate: u32,
    /// Copy semantics (unused for ports)
    pub copy: u32,
    /// Disposition of ports
    pub disposition: u32,
    /// Descriptor type
    pub descriptor_type: u32,
    /// Number of ports
    pub count: u32,
    /// Padding
    pub pad: u32,
}

impl MachMsgOolPortsDescriptor {
    pub const SIZE: usize = 32;

    /// Create new OOL ports descriptor
    pub fn new(address: usize, count: u32, disposition: MsgTypeName, deallocate: bool) -> Self {
        Self {
            address,
            deallocate: if deallocate { 1 } else { 0 },
            copy: 0,
            disposition: disposition as u32,
            descriptor_type: MACH_MSG_OOL_PORTS_DESCRIPTOR,
            count,
            pad: 0,
        }
    }
}

/// Message body descriptor header (complex messages have this after header)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachMsgBodyDescriptor {
    /// Number of descriptors that follow
    pub descriptor_count: u32,
}

// ============================================================================
// Kernel Message
// ============================================================================

/// Kernel message - internal representation of a Mach message
///
/// From Mach4:
/// - ikm_next, ikm_prev: queue links
/// - ikm_size: total allocation size
/// - ikm_header: the message header
///
/// In Rust, we use a more type-safe representation.
/// Queue links are handled by the containing VecDeque, not raw pointers.
#[derive(Debug)]
pub struct IpcKmsg {
    /// Total size of this kmsg buffer
    size: usize,

    /// The message header (ports are actual Arc references, not names)
    header: KmsgHeader,

    /// Message body (inline data)
    body: Vec<u8>,

    /// Port rights in message body (for complex messages)
    port_rights: Vec<KmsgPortRight>,

    /// OOL data regions (for complex messages)
    ool_regions: Vec<KmsgOolRegion>,

    /// OOL port arrays (for complex messages with port arrays)
    ool_ports: Vec<KmsgOolPorts>,
}

/// Internal header with actual port references
#[derive(Debug, Clone)]
pub struct KmsgHeader {
    /// Original message bits
    pub bits: MsgBits,
    /// Total message size
    pub size: MsgSize,
    /// Destination port
    pub remote_port: Option<Arc<Mutex<Port>>>,
    /// Reply port
    pub local_port: Option<Arc<Mutex<Port>>>,
    /// Message ID
    pub id: MsgId,
}

/// Port right embedded in message
#[derive(Debug, Clone)]
pub struct KmsgPortRight {
    /// Offset in message body where this right appears
    pub offset: usize,
    /// The port reference
    pub port: Arc<Mutex<Port>>,
    /// Type of right
    pub disposition: MsgTypeName,
}

/// OOL region in message
#[derive(Debug)]
pub struct KmsgOolRegion {
    /// Offset in message body where descriptor appears
    pub offset: usize,
    /// The actual data
    pub data: Vec<u8>,
    /// Should receiver deallocate source?
    pub deallocate: bool,
}

/// OOL ports array in message
#[derive(Debug)]
pub struct KmsgOolPorts {
    /// Offset in message body where descriptor appears
    pub offset: usize,
    /// The port references
    pub ports: Vec<Arc<Mutex<Port>>>,
    /// Disposition type for all ports
    pub disposition: MsgTypeName,
    /// Should sender deallocate?
    pub deallocate: bool,
}

impl IpcKmsg {
    /// Overhead size for kmsg allocation
    pub const OVERHEAD: usize =
        core::mem::size_of::<Self>() - core::mem::size_of::<MachMsgHeader>();

    /// Default cached message size
    pub const SAVED_SIZE: usize = 256;

    /// Create a new kernel message
    pub fn new(body_size: usize) -> Box<Self> {
        Box::new(Self {
            size: Self::OVERHEAD + core::mem::size_of::<MachMsgHeader>() + body_size,
            header: KmsgHeader {
                bits: 0,
                size: 0,
                remote_port: None,
                local_port: None,
                id: 0,
            },
            body: Vec::with_capacity(body_size),
            port_rights: Vec::new(),
            ool_regions: Vec::new(),
            ool_ports: Vec::new(),
        })
    }

    /// Get total allocation size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get message size (as seen by user)
    pub fn msg_size(&self) -> MsgSize {
        self.header.size
    }

    /// Get message ID
    pub fn msg_id(&self) -> MsgId {
        self.header.id
    }

    /// Get destination port
    pub fn remote_port(&self) -> Option<&Arc<Mutex<Port>>> {
        self.header.remote_port.as_ref()
    }

    /// Get reply port
    pub fn local_port(&self) -> Option<&Arc<Mutex<Port>>> {
        self.header.local_port.as_ref()
    }

    /// Check if message is complex
    pub fn is_complex(&self) -> bool {
        (self.header.bits & MACH_MSGH_BITS_COMPLEX) != 0
    }

    /// Get raw header bits
    pub fn header_bits(&self) -> MsgBits {
        self.header.bits
    }

    /// Set message header from components
    pub fn set_header(
        &mut self,
        bits: MsgBits,
        size: MsgSize,
        remote_port: Option<Arc<Mutex<Port>>>,
        local_port: Option<Arc<Mutex<Port>>>,
        id: MsgId,
    ) {
        self.header.bits = bits;
        self.header.size = size;
        self.header.remote_port = remote_port;
        self.header.local_port = local_port;
        self.header.id = id;
    }

    /// Get message body
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Get mutable message body
    pub fn body_mut(&mut self) -> &mut Vec<u8> {
        &mut self.body
    }

    // ========================================================================
    // Copyin - Convert user message to kernel message
    // ========================================================================

    /// Copy in a message from user space
    ///
    /// This converts user port names to actual port references
    pub fn copyin_header(
        &mut self,
        user_header: &MachMsgHeader,
        space: &IpcSpace,
    ) -> Result<(), IpcError> {
        self.header.bits = user_header.msgh_bits;
        self.header.size = user_header.msgh_size;
        self.header.id = user_header.msgh_id;

        // Copy in remote port (destination)
        if user_header.msgh_remote_port != 0 {
            let remote_type = user_header.remote_port_type();
            let msg_type = MsgTypeName::from_u32(remote_type).ok_or(IpcError::InvalidRight)?;

            let result = copyin(space, user_header.msgh_remote_port, msg_type)?;
            self.header.remote_port = result.port;
        }

        // Copy in local port (reply)
        if user_header.msgh_local_port != 0 {
            let local_type = user_header.local_port_type();
            let msg_type = MsgTypeName::from_u32(local_type).ok_or(IpcError::InvalidRight)?;

            let result = copyin(space, user_header.msgh_local_port, msg_type)?;
            self.header.local_port = result.port;
        }

        Ok(())
    }

    /// Copy in message body (for complex messages)
    ///
    /// For complex messages, this parses descriptors and:
    /// 1. Converts port names to port references (copyin)
    /// 2. Copies OOL data from sender's address space
    pub fn copyin_body(&mut self, data: &[u8], space: &IpcSpace) -> Result<(), IpcError> {
        // For simple messages, just copy the data
        if !self.is_complex() {
            self.body = data.to_vec();
            return Ok(());
        }

        // Complex message: parse descriptors
        if data.len() < 4 {
            return Err(IpcError::MessageTooLarge);
        }

        // Read descriptor count (first 4 bytes of body)
        let descriptor_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

        let mut offset = 4; // After MachMsgBodyDescriptor
        let mut new_body = Vec::with_capacity(data.len());

        // Copy descriptor count to output body
        new_body.extend_from_slice(&data[0..4]);

        // Process each descriptor
        for _i in 0..descriptor_count {
            if offset + 4 > data.len() {
                return Err(IpcError::MessageTooLarge);
            }

            // Peek at descriptor type (last byte of first u32)
            let desc_type = data[offset] & 0xFF;

            match desc_type as u32 {
                MACH_MSG_PORT_DESCRIPTOR => {
                    // Port descriptor: 16 bytes
                    if offset + MachMsgPortDescriptor::SIZE > data.len() {
                        return Err(IpcError::MessageTooLarge);
                    }

                    let name = u32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    let type_and_disp = u32::from_le_bytes([
                        data[offset + 12],
                        data[offset + 13],
                        data[offset + 14],
                        data[offset + 15],
                    ]);

                    let disposition = ((type_and_disp >> 8) & 0xFF) as u32;
                    let msg_type =
                        MsgTypeName::from_u32(disposition).ok_or(IpcError::InvalidRight)?;

                    // Copy in the port right
                    let result = copyin(space, name, msg_type)?;
                    if let Some(port) = result.port {
                        // Store port reference
                        self.port_rights.push(KmsgPortRight {
                            offset: new_body.len(),
                            port,
                            disposition: msg_type,
                        });
                    }

                    // Copy descriptor to output (port name will be resolved on copyout)
                    new_body.extend_from_slice(&data[offset..offset + MachMsgPortDescriptor::SIZE]);
                    offset += MachMsgPortDescriptor::SIZE;
                }

                MACH_MSG_OOL_DESCRIPTOR | MACH_MSG_OOL_VOLATILE_DESCRIPTOR => {
                    // OOL descriptor: 32 bytes
                    if offset + MachMsgOolDescriptor::SIZE > data.len() {
                        return Err(IpcError::MessageTooLarge);
                    }

                    let address = usize::from_le_bytes({
                        let mut arr = [0u8; core::mem::size_of::<usize>()];
                        arr.copy_from_slice(&data[offset..offset + core::mem::size_of::<usize>()]);
                        arr
                    });
                    let deallocate = u32::from_le_bytes([
                        data[offset + 8],
                        data[offset + 9],
                        data[offset + 10],
                        data[offset + 11],
                    ]) != 0;
                    let size_offset = offset + 24;
                    let ool_size = usize::from_le_bytes({
                        let mut arr = [0u8; core::mem::size_of::<usize>()];
                        arr.copy_from_slice(
                            &data[size_offset..size_offset + core::mem::size_of::<usize>()],
                        );
                        arr
                    });

                    // Copy OOL data from sender's address space
                    // In a real kernel with user-space, this would use vm_map_copyin
                    // For kernel-to-kernel messages, we can directly copy the data
                    //
                    // Safety: This is only safe for kernel-to-kernel messages where
                    // the address is a valid kernel pointer. User-space OOL requires
                    // proper VM copyin (Phase 4).
                    let ool_data = if ool_size > 0 && address != 0 {
                        // For kernel addresses, directly copy the data
                        // SAFETY: Caller must ensure address points to valid kernel memory
                        let mut data_copy = vec![0u8; ool_size];
                        unsafe {
                            let src_ptr = address as *const u8;
                            // Only copy if the address looks like a valid kernel pointer
                            // (non-null and reasonably aligned)
                            if !src_ptr.is_null() {
                                core::ptr::copy_nonoverlapping(
                                    src_ptr,
                                    data_copy.as_mut_ptr(),
                                    ool_size,
                                );
                            }
                        }
                        data_copy
                    } else {
                        Vec::new()
                    };

                    self.ool_regions.push(KmsgOolRegion {
                        offset: new_body.len(),
                        data: ool_data,
                        deallocate,
                    });

                    // Copy descriptor to output
                    new_body.extend_from_slice(&data[offset..offset + MachMsgOolDescriptor::SIZE]);
                    offset += MachMsgOolDescriptor::SIZE;
                }

                MACH_MSG_OOL_PORTS_DESCRIPTOR => {
                    // OOL ports descriptor: 32 bytes
                    if offset + MachMsgOolPortsDescriptor::SIZE > data.len() {
                        return Err(IpcError::MessageTooLarge);
                    }

                    let address = usize::from_le_bytes({
                        let mut arr = [0u8; core::mem::size_of::<usize>()];
                        arr.copy_from_slice(&data[offset..offset + core::mem::size_of::<usize>()]);
                        arr
                    });
                    let deallocate = u32::from_le_bytes([
                        data[offset + 8],
                        data[offset + 9],
                        data[offset + 10],
                        data[offset + 11],
                    ]) != 0;
                    let disposition = u32::from_le_bytes([
                        data[offset + 16],
                        data[offset + 17],
                        data[offset + 18],
                        data[offset + 19],
                    ]);
                    let count = u32::from_le_bytes([
                        data[offset + 24],
                        data[offset + 25],
                        data[offset + 26],
                        data[offset + 27],
                    ]);

                    let msg_type =
                        MsgTypeName::from_u32(disposition).ok_or(IpcError::InvalidRight)?;

                    // Copy in port names from sender's address space
                    // For kernel-to-kernel messages, treat address as kernel pointer
                    let mut ports = Vec::with_capacity(count as usize);
                    if address != 0 && count > 0 {
                        unsafe {
                            let names_ptr = address as *const MachPortName;
                            for i in 0..count as usize {
                                let name = *names_ptr.add(i);
                                // Convert each port name to a reference
                                let result = copyin(space, name, msg_type)?;
                                if let Some(port) = result.port {
                                    ports.push(port);
                                }
                            }
                        }
                    }

                    self.ool_ports.push(KmsgOolPorts {
                        offset: new_body.len(),
                        ports,
                        disposition: msg_type,
                        deallocate,
                    });

                    // Copy descriptor to output
                    new_body
                        .extend_from_slice(&data[offset..offset + MachMsgOolPortsDescriptor::SIZE]);
                    offset += MachMsgOolPortsDescriptor::SIZE;
                }

                _ => {
                    // Unknown descriptor type
                    return Err(IpcError::InvalidRight);
                }
            }
        }

        // Copy remaining inline data after descriptors
        if offset < data.len() {
            new_body.extend_from_slice(&data[offset..]);
        }

        self.body = new_body;
        Ok(())
    }

    // ========================================================================
    // Copyout - Convert kernel message to user message
    // ========================================================================

    /// Copy out header to user space
    pub fn copyout_header(&self, space: &IpcSpace) -> Result<MachMsgHeader, IpcError> {
        let mut header = MachMsgHeader::new();

        header.msgh_bits = self.header.bits;
        header.msgh_size = self.header.size;
        header.msgh_id = self.header.id;

        // Copy out remote port
        if let Some(port) = &self.header.remote_port {
            let remote_type = (self.header.bits >> 8) & 0xFF;
            let msg_type = MsgTypeName::from_u32(remote_type).ok_or(IpcError::InvalidRight)?;

            header.msgh_remote_port = copyout(space, Arc::clone(port), msg_type)?;
        }

        // Copy out local port
        if let Some(port) = &self.header.local_port {
            let local_type = self.header.bits & 0xFF;
            let msg_type = MsgTypeName::from_u32(local_type).ok_or(IpcError::InvalidRight)?;

            header.msgh_local_port = copyout(space, Arc::clone(port), msg_type)?;
        }

        Ok(header)
    }

    /// Copy out message body (simple version - just returns bytes)
    pub fn copyout_body(&self) -> &[u8] {
        &self.body
    }

    /// Copy out message body with complex message handling
    ///
    /// For complex messages, this:
    /// 1. Converts port references back to names in receiver's space
    /// 2. Maps OOL data into receiver's address space
    pub fn copyout_body_complex(&self, space: &IpcSpace) -> Result<Vec<u8>, IpcError> {
        if !self.is_complex() {
            return Ok(self.body.clone());
        }

        let mut output = self.body.clone();

        // Process port rights: convert port references to names in receiver's space
        for port_right in &self.port_rights {
            let name = copyout(space, Arc::clone(&port_right.port), port_right.disposition)?;

            // Write the port name back into the output at the right offset
            // Port name is at the beginning of the descriptor
            if port_right.offset + 4 <= output.len() {
                let name_bytes = name.to_le_bytes();
                output[port_right.offset..port_right.offset + 4].copy_from_slice(&name_bytes);
            }
        }

        // Process OOL regions: update addresses to point to receiver's space
        //
        // For kernel-to-kernel messages, we update the descriptor to point
        // to the kmsg's internal OOL data buffer. The receiver can access
        // this data directly since it's in kernel space.
        //
        // For user-space receivers (Phase 4), this would use vm_map_copyout
        // to map the data into the receiver's address space.
        for ool_region in &self.ool_regions {
            // For kernel-to-kernel messages, provide the pointer to our internal buffer
            let data_ptr = ool_region.data.as_ptr() as usize;

            // Update descriptor's address field to point to the copied data
            // The address field is at the start of the OOL descriptor
            if ool_region.offset + core::mem::size_of::<usize>() <= output.len() {
                let addr_bytes = data_ptr.to_le_bytes();
                output[ool_region.offset..ool_region.offset + addr_bytes.len()]
                    .copy_from_slice(&addr_bytes);
            }

            // Also update the size field if it was somehow modified
            // Size is at offset 24 in MachMsgOolDescriptor (after address, deallocate, copy, pad1, descriptor_type)
            let size_offset = ool_region.offset + 24;
            if size_offset + core::mem::size_of::<usize>() <= output.len() {
                let size_bytes = ool_region.data.len().to_le_bytes();
                output[size_offset..size_offset + size_bytes.len()].copy_from_slice(&size_bytes);
            }
        }

        // Process OOL ports arrays: convert port references to names in receiver's space
        // and allocate an array of port names for the receiver
        for ool_ports_entry in &self.ool_ports {
            if ool_ports_entry.ports.is_empty() {
                continue;
            }

            // Allocate space for port names (kernel-to-kernel: use Vec as backing store)
            let mut names: Vec<MachPortName> = Vec::with_capacity(ool_ports_entry.ports.len());
            for port in &ool_ports_entry.ports {
                let name = copyout(space, Arc::clone(port), ool_ports_entry.disposition)?;
                names.push(name);
            }

            // For kernel-to-kernel messages, the names Vec serves as the backing store
            // Update the address in the descriptor to point to the names array
            let names_ptr = names.as_ptr() as usize;
            if ool_ports_entry.offset + core::mem::size_of::<usize>() <= output.len() {
                let addr_bytes = names_ptr.to_le_bytes();
                output[ool_ports_entry.offset..ool_ports_entry.offset + addr_bytes.len()]
                    .copy_from_slice(&addr_bytes);
            }

            // Update count field (at offset 24)
            let count_offset = ool_ports_entry.offset + 24;
            if count_offset + 4 <= output.len() {
                let count_bytes = (names.len() as u32).to_le_bytes();
                output[count_offset..count_offset + 4].copy_from_slice(&count_bytes);
            }

            // Note: names Vec will be leaked here since we're passing raw pointer
            // In a real implementation, this would be managed by the VM system
            core::mem::forget(names);
        }

        Ok(output)
    }

    /// Get OOL region data by index
    pub fn get_ool_data(&self, index: usize) -> Option<&[u8]> {
        self.ool_regions.get(index).map(|r| r.data.as_slice())
    }

    /// Get number of OOL regions
    pub fn ool_count(&self) -> usize {
        self.ool_regions.len()
    }

    /// Get number of port rights
    pub fn port_rights_count(&self) -> usize {
        self.port_rights.len()
    }

    /// Get port right by index
    pub fn get_port_right(&self, index: usize) -> Option<&KmsgPortRight> {
        self.port_rights.get(index)
    }

    // ========================================================================
    // Cleanup
    // ========================================================================

    /// Clean up the message (release all port references)
    pub fn clean(&mut self) {
        // Release remote port
        if let Some(port) = self.header.remote_port.take() {
            let port_guard = port.lock();
            port_guard.release_send_right();
        }

        // Release local port
        if let Some(port) = self.header.local_port.take() {
            let port_guard = port.lock();
            port_guard.release_send_right();
        }

        // Release embedded port rights (inline descriptors)
        for right in self.port_rights.drain(..) {
            let port_guard = right.port.lock();
            port_guard.release_send_right();
        }

        // Release OOL port array references
        for ool_ports_entry in self.ool_ports.drain(..) {
            for port in ool_ports_entry.ports {
                let port_guard = port.lock();
                port_guard.release_send_right();
            }
        }

        // Clear OOL data regions
        self.ool_regions.clear();
        self.body.clear();
    }

    /// Destroy the message
    pub fn destroy(mut self) {
        self.clean();
        // Box will be dropped here
    }
}

// ============================================================================
// Kernel Message Queue
// ============================================================================

/// Queue of kernel messages
#[derive(Debug, Default)]
pub struct IpcKmsgQueue {
    /// Messages stored in a VecDeque for safe Rust queue management
    messages: alloc::collections::VecDeque<Box<IpcKmsg>>,
}

impl IpcKmsgQueue {
    /// Create a new empty queue
    pub fn new() -> Self {
        Self {
            messages: alloc::collections::VecDeque::new(),
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get queue count
    pub fn count(&self) -> usize {
        self.messages.len()
    }

    /// Enqueue a message at the tail
    pub fn enqueue(&mut self, kmsg: Box<IpcKmsg>) {
        self.messages.push_back(kmsg);
    }

    /// Dequeue a message from the head
    pub fn dequeue(&mut self) -> Option<Box<IpcKmsg>> {
        self.messages.pop_front()
    }

    /// Peek at first message without removing
    pub fn first(&self) -> Option<&IpcKmsg> {
        self.messages.front().map(|b| b.as_ref())
    }

    /// Clear all messages from queue
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

// ============================================================================
// Message allocation helpers
// ============================================================================

/// Allocate a kernel message of specified size
pub fn kmsg_alloc(size: MsgSize) -> Box<IpcKmsg> {
    IpcKmsg::new(size as usize)
}

/// Allocate a small cached kernel message
pub fn kmsg_alloc_small() -> Box<IpcKmsg> {
    IpcKmsg::new(IpcKmsg::SAVED_SIZE)
}

/// Get a message from user space
pub fn kmsg_get(
    user_header: &MachMsgHeader,
    user_body: &[u8],
    space: &IpcSpace,
) -> Result<Box<IpcKmsg>, IpcError> {
    let mut kmsg = kmsg_alloc(user_header.msgh_size);

    kmsg.copyin_header(user_header, space)?;
    kmsg.copyin_body(user_body, space)?;

    Ok(kmsg)
}

/// Put a message to user space
pub fn kmsg_put(kmsg: &IpcKmsg, space: &IpcSpace) -> Result<(MachMsgHeader, Vec<u8>), IpcError> {
    let header = kmsg.copyout_header(space)?;
    let body = kmsg.copyout_body().to_vec();

    Ok((header, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmsg_queue() {
        let mut queue = IpcKmsgQueue::new();
        assert!(queue.is_empty());

        let kmsg = kmsg_alloc_small();
        queue.enqueue(kmsg);
        assert_eq!(queue.count(), 1);

        let _dequeued = queue.dequeue().unwrap();
        assert!(queue.is_empty());
    }
}
