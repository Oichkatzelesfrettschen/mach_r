//! IPC Entry - Port name to capability translation
//!
//! Based on Mach4 ipc/ipc_entry.h
//! Each ipc_entry records a capability (port right) in a task's IPC space.
//! Entries are stored in a table indexed by port name.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use super::port::Port;
use super::IpcError;

/// Port name type - the user-visible handle to a port capability
pub type MachPortName = u32;

/// Port index - position in the entry table
pub type MachPortIndex = u32;

/// Entry bits field type
pub type IpcEntryBits = u32;

// ============================================================================
// Entry Bits Constants (from Mach4 ipc_entry.h)
// ============================================================================

/// Mask for user references (16 bits)
pub const IE_BITS_UREFS_MASK: u32 = 0x0000_FFFF;

/// Extract user references from bits
#[inline]
pub const fn ie_bits_urefs(bits: IpcEntryBits) -> u16 {
    (bits & IE_BITS_UREFS_MASK) as u16
}

/// Mask for capability type (5 bits)
pub const IE_BITS_TYPE_MASK: u32 = 0x001F_0000;

/// Extract type from bits
#[inline]
pub const fn ie_bits_type(bits: IpcEntryBits) -> u32 {
    bits & IE_BITS_TYPE_MASK
}

/// Msg-accepted request bit
pub const IE_BITS_MAREQUEST: u32 = 0x0020_0000;

/// Compatibility mode bit
pub const IE_BITS_COMPAT: u32 = 0x0040_0000;

/// Hash collision bit
pub const IE_BITS_COLLISION: u32 = 0x0080_0000;

/// Mask for all bits relevant to the right
pub const IE_BITS_RIGHT_MASK: u32 = 0x007F_FFFF;

/// Mask for generation number (8 bits)
pub const IE_BITS_GEN_MASK: u32 = 0xFF00_0000;

/// Extract generation from bits
#[inline]
pub const fn ie_bits_gen(bits: IpcEntryBits) -> u32 {
    bits & IE_BITS_GEN_MASK
}

/// One generation increment
pub const IE_BITS_GEN_ONE: u32 = 0x0100_0000;

// ============================================================================
// Port Right Types (shifted into IE_BITS_TYPE position)
// ============================================================================

/// Send right
pub const MACH_PORT_TYPE_SEND: u32 = 0x0001_0000;

/// Receive right
pub const MACH_PORT_TYPE_RECEIVE: u32 = 0x0002_0000;

/// Send-once right
pub const MACH_PORT_TYPE_SEND_ONCE: u32 = 0x0004_0000;

/// Port set
pub const MACH_PORT_TYPE_PORT_SET: u32 = 0x0008_0000;

/// Dead name (port was destroyed)
pub const MACH_PORT_TYPE_DEAD_NAME: u32 = 0x0010_0000;

// ============================================================================
// IPC Object - Generic reference to port or port set
// ============================================================================

/// IPC object types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcObjectType {
    /// Regular port
    Port,
    /// Port set (collection of ports)
    PortSet,
    /// Dead/invalid object
    Dead,
}

/// Reference to an IPC object (port or port set)
#[derive(Debug, Clone)]
pub enum IpcObject {
    /// Port reference
    Port(Arc<Mutex<Port>>),
    /// Port set reference (to be implemented)
    PortSet(u32), // Placeholder - will be PortSet type
    /// No object (free entry)
    None,
}

impl IpcObject {
    /// Check if this is a null/empty object
    pub fn is_none(&self) -> bool {
        matches!(self, IpcObject::None)
    }

    /// Get the object type
    pub fn object_type(&self) -> IpcObjectType {
        match self {
            IpcObject::Port(_) => IpcObjectType::Port,
            IpcObject::PortSet(_) => IpcObjectType::PortSet,
            IpcObject::None => IpcObjectType::Dead,
        }
    }

    /// Try to get as a port reference
    pub fn as_port(&self) -> Option<&Arc<Mutex<Port>>> {
        match self {
            IpcObject::Port(p) => Some(p),
            _ => None,
        }
    }
}

// ============================================================================
// IPC Entry - A single capability record
// ============================================================================

/// IPC Entry - records a single capability in a task's IPC space
///
/// From Mach4:
/// - ie_bits: type, urefs, generation, flags
/// - ie_object: the actual port/port set
/// - ie_next/ie_request: union for free list or request index
/// - ie_index: hash table index for reverse lookups
#[derive(Debug, Clone)]
pub struct IpcEntry {
    /// Capability bits: type | urefs | generation | flags
    bits: IpcEntryBits,

    /// The IPC object this entry refers to
    object: IpcObject,

    /// For free entries: next free index
    /// For active entries: pending request index
    index_or_request: u32,

    /// Hash table index for (space, object) -> name lookup
    hash_index: u32,
}

impl IpcEntry {
    /// Create a new empty/free entry
    pub const fn new() -> Self {
        Self {
            bits: 0,
            object: IpcObject::None,
            index_or_request: 0,
            hash_index: 0,
        }
    }

    /// Create a free entry pointing to next free index
    pub fn new_free(next_free: u32, generation: u32) -> Self {
        Self {
            bits: generation & IE_BITS_GEN_MASK,
            object: IpcObject::None,
            index_or_request: next_free,
            hash_index: 0,
        }
    }

    /// Check if this entry is free (no capability)
    #[inline]
    pub fn is_free(&self) -> bool {
        self.object.is_none() && ie_bits_type(self.bits) == 0
    }

    /// Check if this entry has a valid capability
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.is_free()
    }

    /// Get the capability type
    #[inline]
    pub fn entry_type(&self) -> u32 {
        ie_bits_type(self.bits)
    }

    /// Get user reference count
    #[inline]
    pub fn urefs(&self) -> u16 {
        ie_bits_urefs(self.bits)
    }

    /// Get generation number
    #[inline]
    pub fn generation(&self) -> u32 {
        ie_bits_gen(self.bits)
    }

    /// Get the IPC object
    pub fn object(&self) -> &IpcObject {
        &self.object
    }

    /// Get next free index (only valid for free entries)
    #[inline]
    pub fn next_free(&self) -> u32 {
        self.index_or_request
    }

    /// Get request index (only valid for active entries)
    #[inline]
    pub fn request_index(&self) -> u32 {
        self.index_or_request
    }

    /// Set up entry for a send right
    pub fn setup_send_right(&mut self, port: Arc<Mutex<Port>>, urefs: u16) {
        let gen = self.generation();
        self.bits = gen | MACH_PORT_TYPE_SEND | (urefs as u32);
        self.object = IpcObject::Port(port);
        self.index_or_request = 0;
    }

    /// Set up entry for a receive right
    pub fn setup_receive_right(&mut self, port: Arc<Mutex<Port>>) {
        let gen = self.generation();
        self.bits = gen | MACH_PORT_TYPE_RECEIVE | 1; // 1 uref for receive
        self.object = IpcObject::Port(port);
        self.index_or_request = 0;
    }

    /// Set up entry for a send-once right
    pub fn setup_send_once_right(&mut self, port: Arc<Mutex<Port>>) {
        let gen = self.generation();
        self.bits = gen | MACH_PORT_TYPE_SEND_ONCE | 1; // 1 uref for send-once
        self.object = IpcObject::Port(port);
        self.index_or_request = 0;
    }

    /// Set up entry for a port set
    pub fn setup_port_set(&mut self) {
        static NEXT_PSET_ID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
        let gen = self.generation();
        let pset_id = NEXT_PSET_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        self.bits = gen | MACH_PORT_TYPE_PORT_SET | 1; // 1 uref
        self.object = IpcObject::PortSet(pset_id);
        self.index_or_request = 0;
    }

    /// Convert to dead name (port was destroyed)
    pub fn make_dead_name(&mut self) {
        let gen = self.generation();
        let urefs = self.urefs();
        self.bits = gen | MACH_PORT_TYPE_DEAD_NAME | (urefs as u32);
        self.object = IpcObject::None;
    }

    /// Clear the entry (deallocate)
    pub fn clear(&mut self, next_free: u32) {
        // Increment generation on dealloc
        let new_gen = (self.generation() + IE_BITS_GEN_ONE) & IE_BITS_GEN_MASK;
        self.bits = new_gen;
        self.object = IpcObject::None;
        self.index_or_request = next_free;
        self.hash_index = 0;
    }

    /// Add user references
    pub fn add_urefs(&mut self, delta: u16) -> Result<(), IpcError> {
        let current = self.urefs();
        let new_urefs = current.checked_add(delta).ok_or(IpcError::NoSpace)?;
        if new_urefs > (IE_BITS_UREFS_MASK as u16) {
            return Err(IpcError::NoSpace);
        }
        self.bits = (self.bits & !IE_BITS_UREFS_MASK) | (new_urefs as u32);
        Ok(())
    }

    /// Remove user references, returns true if entry should be deallocated
    pub fn remove_urefs(&mut self, delta: u16) -> Result<bool, IpcError> {
        let current = self.urefs();
        if delta > current {
            return Err(IpcError::InvalidRight);
        }
        let new_urefs = current - delta;
        self.bits = (self.bits & !IE_BITS_UREFS_MASK) | (new_urefs as u32);
        Ok(new_urefs == 0)
    }

    /// Check if entry has send right
    #[inline]
    pub fn has_send(&self) -> bool {
        (self.bits & MACH_PORT_TYPE_SEND) != 0
    }

    /// Check if entry has receive right
    #[inline]
    pub fn has_receive(&self) -> bool {
        (self.bits & MACH_PORT_TYPE_RECEIVE) != 0
    }

    /// Check if entry has send-once right
    #[inline]
    pub fn has_send_once(&self) -> bool {
        (self.bits & MACH_PORT_TYPE_SEND_ONCE) != 0
    }

    /// Check if entry is a dead name
    #[inline]
    pub fn is_dead_name(&self) -> bool {
        (self.bits & MACH_PORT_TYPE_DEAD_NAME) != 0
    }
}

impl Default for IpcEntry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Entry Table - Dynamic array of entries
// ============================================================================

/// Initial table size (number of entries)
pub const IPC_ENTRY_TABLE_MIN: usize = 16;

/// Maximum table size
pub const IPC_ENTRY_TABLE_MAX: usize = 65536;

/// Entry table with free list management
#[derive(Debug)]
pub struct IpcEntryTable {
    /// The entries array
    entries: alloc::vec::Vec<IpcEntry>,

    /// Index of first free entry (0 = head of free list)
    free_head: u32,

    /// Number of active (non-free) entries
    active_count: u32,

    /// Next generation counter for new entries
    next_gen: AtomicU32,
}

impl IpcEntryTable {
    /// Create a new entry table
    pub fn new(initial_size: usize) -> Self {
        let size = initial_size.max(IPC_ENTRY_TABLE_MIN);
        let mut entries = alloc::vec::Vec::with_capacity(size);

        // Entry 0 is always free and is the head of free list
        for i in 0..size {
            let next = if i + 1 < size { (i + 1) as u32 } else { 0 };
            entries.push(IpcEntry::new_free(next, 0));
        }

        Self {
            entries,
            free_head: 1, // Start allocating from index 1 (0 is reserved)
            active_count: 0,
            next_gen: AtomicU32::new(IE_BITS_GEN_ONE),
        }
    }

    /// Get table size
    #[inline]
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Get number of active entries
    #[inline]
    pub fn active_count(&self) -> u32 {
        self.active_count
    }

    /// Look up an entry by name (index + generation check)
    pub fn lookup(&self, name: MachPortName) -> Option<&IpcEntry> {
        let index = name as usize;
        if index >= self.entries.len() {
            return None;
        }

        let entry = &self.entries[index];

        // Verify generation matches (upper 8 bits of name)
        let name_gen = (name & IE_BITS_GEN_MASK) as u32;
        if entry.generation() != name_gen && name_gen != 0 {
            return None;
        }

        if entry.is_free() {
            return None;
        }

        Some(entry)
    }

    /// Look up an entry mutably
    pub fn lookup_mut(&mut self, name: MachPortName) -> Option<&mut IpcEntry> {
        let index = name as usize;
        if index >= self.entries.len() {
            return None;
        }

        // Check generation and validity
        let entry = &self.entries[index];
        let name_gen = (name & IE_BITS_GEN_MASK) as u32;
        if entry.generation() != name_gen && name_gen != 0 {
            return None;
        }
        if entry.is_free() {
            return None;
        }

        Some(&mut self.entries[index])
    }

    /// Allocate a new entry, returns (name, entry_mut)
    pub fn alloc(&mut self) -> Result<(MachPortName, &mut IpcEntry), IpcError> {
        if self.free_head == 0 {
            // Need to grow table
            self.grow()?;
        }

        let index = self.free_head as usize;
        let entry = &mut self.entries[index];

        // Get next free from this entry before we modify it
        self.free_head = entry.next_free();

        // Generate the port name with generation embedded
        let gen = entry.generation();
        let name = (index as u32) | gen;

        self.active_count += 1;

        Ok((name, &mut self.entries[index]))
    }

    /// Allocate a new entry with a specific name
    pub fn alloc_with_name(&mut self, name: MachPortName) -> Result<&mut IpcEntry, IpcError> {
        let index = (name & !IE_BITS_GEN_MASK) as usize;

        // Ensure table is large enough
        while index >= self.entries.len() {
            self.grow()?;
        }

        let entry = &self.entries[index];
        if !entry.is_free() {
            return Err(IpcError::NoSpace);
        }

        // Remove from free list (need to search for it)
        if self.free_head == index as u32 {
            let entry = &self.entries[index];
            self.free_head = entry.next_free();
        } else {
            // Search through free list to remove this entry
            let mut prev_idx = self.free_head as usize;
            while prev_idx != 0 && prev_idx < self.entries.len() {
                let next_free = self.entries[prev_idx].next_free();
                if next_free as usize == index {
                    // Found it - remove from list
                    let removed_next = self.entries[index].next_free();
                    self.entries[prev_idx].index_or_request = removed_next;
                    break;
                }
                prev_idx = next_free as usize;
            }
        }

        self.active_count += 1;
        Ok(&mut self.entries[index])
    }

    /// Deallocate an entry
    pub fn dealloc(&mut self, name: MachPortName) -> Result<(), IpcError> {
        let index = name as usize;
        if index >= self.entries.len() || index == 0 {
            return Err(IpcError::InvalidPort);
        }

        let entry = &mut self.entries[index];
        if entry.is_free() {
            return Err(IpcError::InvalidPort);
        }

        // Clear entry and add to free list
        entry.clear(self.free_head);
        self.free_head = index as u32;
        self.active_count -= 1;

        Ok(())
    }

    /// Grow the table to accommodate more entries
    pub fn grow(&mut self) -> Result<(), IpcError> {
        let old_size = self.entries.len();
        let new_size = (old_size * 2).min(IPC_ENTRY_TABLE_MAX);

        if new_size == old_size {
            return Err(IpcError::NoSpace);
        }

        let gen = self.next_gen.fetch_add(IE_BITS_GEN_ONE, Ordering::SeqCst);

        // Add new entries to free list
        self.entries.reserve(new_size - old_size);
        for i in old_size..new_size {
            let next = if i + 1 < new_size {
                (i + 1) as u32
            } else {
                self.free_head
            };
            self.entries.push(IpcEntry::new_free(next, gen));
        }

        // Point free head to first new entry
        self.free_head = old_size as u32;

        Ok(())
    }

    /// Find entry by object (reverse lookup)
    pub fn find_by_object(&self, target: &IpcObject) -> Option<(MachPortName, &IpcEntry)> {
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.is_valid() {
                // Compare objects
                match (&entry.object, target) {
                    (IpcObject::Port(a), IpcObject::Port(b)) => {
                        if Arc::ptr_eq(a, b) {
                            let name = (index as u32) | entry.generation();
                            return Some((name, entry));
                        }
                    }
                    (IpcObject::PortSet(a), IpcObject::PortSet(b)) if a == b => {
                        let name = (index as u32) | entry.generation();
                        return Some((name, entry));
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Iterate over all active entries
    pub fn iter_active(&self) -> impl Iterator<Item = (MachPortName, &IpcEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                if entry.is_valid() {
                    let name = (index as u32) | entry.generation();
                    Some((name, entry))
                } else {
                    None
                }
            })
    }
}

impl Default for IpcEntryTable {
    fn default() -> Self {
        Self::new(IPC_ENTRY_TABLE_MIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_bits() {
        assert_eq!(ie_bits_urefs(0x0000_1234), 0x1234);
        assert_eq!(ie_bits_type(MACH_PORT_TYPE_SEND), MACH_PORT_TYPE_SEND);
        assert_eq!(ie_bits_gen(0xFF00_0000), 0xFF00_0000);
    }

    #[test]
    fn test_entry_table_alloc() {
        let mut table = IpcEntryTable::new(4);

        // Allocate an entry
        let (name, entry) = table.alloc().unwrap();
        assert!(!entry.is_free());
        assert_eq!(table.active_count(), 1);

        // Deallocate
        table.dealloc(name).unwrap();
        assert_eq!(table.active_count(), 0);
    }
}
