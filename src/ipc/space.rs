//! IPC Space - Per-task IPC capability namespace
//!
//! Based on Mach4 ipc/ipc_space.h
//! Every task has an IPC space containing its port capabilities.
//! The space manages a table of IpcEntry records.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use super::entry::{IpcEntry, IpcEntryTable, IpcObject, MachPortName};
use super::port::Port;
use super::IpcError;

/// Space reference count type
pub type IpcSpaceRefs = u32;

/// Space ID type - newtype for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SpaceId(pub u64);

// ============================================================================
// IPC Space State
// ============================================================================

/// IPC Space states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceState {
    /// Space is active and usable
    Active,
    /// Space is currently growing its table
    Growing,
    /// Space is being destroyed
    Dead,
}

// ============================================================================
// IPC Space - Per-task capability namespace
// ============================================================================

/// IPC Space - contains all port capabilities for a task
///
/// From Mach4 ipc_space.h:
/// - is_references: reference count
/// - is_active: is the space alive?
/// - is_growing: is the table being grown?
/// - is_table: array of entries
/// - is_table_size: current table size
#[derive(Debug)]
pub struct IpcSpace {
    /// Unique space identifier
    id: SpaceId,

    /// Reference count
    references: AtomicU32,

    /// Space state
    state: Mutex<SpaceState>,

    /// Is the space currently growing?
    is_growing: AtomicBool,

    /// Entry table
    table: Mutex<IpcEntryTable>,
}

/// Next space ID counter
static NEXT_SPACE_ID: AtomicU32 = AtomicU32::new(1);

impl IpcSpace {
    /// Create a new IPC space
    pub fn new() -> Arc<Self> {
        let id = SpaceId(NEXT_SPACE_ID.fetch_add(1, Ordering::SeqCst) as u64);

        Arc::new(Self {
            id,
            references: AtomicU32::new(1),
            state: Mutex::new(SpaceState::Active),
            is_growing: AtomicBool::new(false),
            table: Mutex::new(IpcEntryTable::default()),
        })
    }

    /// Create a new IPC space with specified initial table size
    pub fn with_size(initial_size: usize) -> Arc<Self> {
        let id = SpaceId(NEXT_SPACE_ID.fetch_add(1, Ordering::SeqCst) as u64);

        Arc::new(Self {
            id,
            references: AtomicU32::new(1),
            state: Mutex::new(SpaceState::Active),
            is_growing: AtomicBool::new(false),
            table: Mutex::new(IpcEntryTable::new(initial_size)),
        })
    }

    /// Get space ID
    #[inline]
    pub fn id(&self) -> SpaceId {
        self.id
    }

    /// Check if space is active
    pub fn is_active(&self) -> bool {
        *self.state.lock() == SpaceState::Active
    }

    /// Check if space is dead
    pub fn is_dead(&self) -> bool {
        *self.state.lock() == SpaceState::Dead
    }

    /// Get current reference count
    pub fn ref_count(&self) -> u32 {
        self.references.load(Ordering::SeqCst)
    }

    /// Add a reference
    pub fn reference(&self) {
        self.references.fetch_add(1, Ordering::SeqCst);
    }

    /// Remove a reference, returns true if space should be freed
    pub fn release(&self) -> bool {
        let old = self.references.fetch_sub(1, Ordering::SeqCst);
        old == 1
    }

    // ========================================================================
    // Entry Operations
    // ========================================================================

    /// Look up an entry by port name
    pub fn lookup(&self, name: MachPortName) -> Result<IpcEntry, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let table = self.table.lock();
        table.lookup(name).cloned().ok_or(IpcError::InvalidPort)
    }

    /// Allocate a new entry
    pub fn entry_alloc(&self) -> Result<MachPortName, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let (name, _entry) = table.alloc()?;
        Ok(name)
    }

    /// Allocate an entry and set up a send right
    pub fn alloc_send_right(
        &self,
        port: Arc<Mutex<Port>>,
        urefs: u16,
    ) -> Result<MachPortName, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let (name, entry) = table.alloc()?;
        entry.setup_send_right(port, urefs);
        Ok(name)
    }

    /// Allocate an entry and set up a receive right
    pub fn alloc_receive_right(&self, port: Arc<Mutex<Port>>) -> Result<MachPortName, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let (name, entry) = table.alloc()?;
        entry.setup_receive_right(port);
        Ok(name)
    }

    /// Allocate an entry and set up a send-once right
    pub fn alloc_send_once_right(&self, port: Arc<Mutex<Port>>) -> Result<MachPortName, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let (name, entry) = table.alloc()?;
        entry.setup_send_once_right(port);
        Ok(name)
    }

    /// Deallocate an entry
    pub fn entry_dealloc(&self, name: MachPortName) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        table.dealloc(name)
    }

    /// Find an entry by its IPC object (reverse lookup)
    pub fn find_entry(&self, object: &IpcObject) -> Option<(MachPortName, IpcEntry)> {
        if !self.is_active() {
            return None;
        }

        let table = self.table.lock();
        table
            .find_by_object(object)
            .map(|(name, entry)| (name, entry.clone()))
    }

    /// Get number of active entries
    pub fn entry_count(&self) -> u32 {
        let table = self.table.lock();
        table.active_count()
    }

    /// Get table size
    pub fn table_size(&self) -> usize {
        let table = self.table.lock();
        table.size()
    }

    /// Grow the entry table
    pub fn grow_table(&self) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        // Prevent concurrent grows
        if self.is_growing.swap(true, Ordering::SeqCst) {
            return Err(IpcError::WouldBlock);
        }

        let result = {
            let mut table = self.table.lock();
            table.grow()
        };

        self.is_growing.store(false, Ordering::SeqCst);
        result
    }

    // ========================================================================
    // Space Lifecycle
    // ========================================================================

    /// Destroy the space - clean up all entries
    pub fn destroy(&self) -> Result<(), IpcError> {
        // Mark as dead
        {
            let mut state = self.state.lock();
            if *state == SpaceState::Dead {
                return Err(IpcError::InvalidPort);
            }
            *state = SpaceState::Dead;
        }

        // Clean up all entries
        // In a full implementation, this would:
        // 1. Cancel all dead name requests
        // 2. Destroy all receive rights
        // 3. Release all send rights
        // 4. Clean up port sets

        Ok(())
    }

    /// Modify entry rights (add/remove urefs)
    pub fn modify_refs(&self, name: MachPortName, delta: i32) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let entry = table.lookup_mut(name).ok_or(IpcError::InvalidPort)?;

        if delta > 0 {
            entry.add_urefs(delta as u16)?;
        } else if delta < 0 {
            let should_dealloc = entry.remove_urefs((-delta) as u16)?;
            if should_dealloc {
                let _ = entry;
                table.dealloc(name)?;
            }
        }

        Ok(())
    }

    /// Get port from entry (if it's a port right)
    pub fn get_port(&self, name: MachPortName) -> Result<Arc<Mutex<Port>>, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let table = self.table.lock();
        let entry = table.lookup(name).ok_or(IpcError::InvalidPort)?;

        match entry.object() {
            IpcObject::Port(port) => Ok(Arc::clone(port)),
            IpcObject::None => Err(IpcError::PortDead),
            IpcObject::PortSet(_) => Err(IpcError::InvalidRight),
        }
    }

    // ========================================================================
    // Named allocation methods (for insert_right operations)
    // ========================================================================

    /// Allocate a receive right with a specific name
    pub fn alloc_receive_right_with_name(
        &self,
        port: Arc<Mutex<Port>>,
        name: MachPortName,
    ) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();

        // Check if name is already in use
        if table.lookup(name).is_some() {
            return Err(IpcError::NoSpace);
        }

        let entry = table.alloc_with_name(name)?;
        entry.setup_receive_right(port);
        Ok(())
    }

    /// Allocate a send right with a specific name
    pub fn alloc_send_right_with_name(
        &self,
        port: Arc<Mutex<Port>>,
        name: MachPortName,
        urefs: u16,
    ) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();

        // Check if name is already in use
        if table.lookup(name).is_some() {
            return Err(IpcError::NoSpace);
        }

        let entry = table.alloc_with_name(name)?;
        entry.setup_send_right(port, urefs);
        Ok(())
    }

    /// Allocate a send-once right with a specific name
    pub fn alloc_send_once_right_with_name(
        &self,
        port: Arc<Mutex<Port>>,
        name: MachPortName,
    ) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();

        // Check if name is already in use
        if table.lookup(name).is_some() {
            return Err(IpcError::NoSpace);
        }

        let entry = table.alloc_with_name(name)?;
        entry.setup_send_once_right(port);
        Ok(())
    }

    /// Allocate a port set
    pub fn alloc_port_set(&self) -> Result<MachPortName, IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();
        let (name, entry) = table.alloc()?;
        entry.setup_port_set();
        Ok(name)
    }

    /// Allocate a port set with a specific name
    pub fn alloc_port_set_with_name(&self, name: MachPortName) -> Result<(), IpcError> {
        if !self.is_active() {
            return Err(IpcError::InvalidPort);
        }

        let mut table = self.table.lock();

        // Check if name is already in use
        if table.lookup(name).is_some() {
            return Err(IpcError::NoSpace);
        }

        let entry = table.alloc_with_name(name)?;
        entry.setup_port_set();
        Ok(())
    }

    /// Check if entry has specific right type
    pub fn has_right(&self, name: MachPortName, right_type: u32) -> bool {
        if !self.is_active() {
            return false;
        }

        let table = self.table.lock();
        if let Some(entry) = table.lookup(name) {
            (entry.entry_type() & right_type) != 0
        } else {
            false
        }
    }

    /// Iterate over all entries (for debugging/cleanup)
    pub fn for_each_entry<F>(&self, mut f: F)
    where
        F: FnMut(MachPortName, &IpcEntry),
    {
        if !self.is_active() {
            return;
        }

        let table = self.table.lock();
        for (name, entry) in table.iter_active() {
            f(name, entry);
        }
    }
}

impl Default for IpcSpace {
    fn default() -> Self {
        Self {
            id: SpaceId(NEXT_SPACE_ID.fetch_add(1, Ordering::SeqCst) as u64),
            references: AtomicU32::new(1),
            state: Mutex::new(SpaceState::Active),
            is_growing: AtomicBool::new(false),
            table: Mutex::new(IpcEntryTable::default()),
        }
    }
}

// ============================================================================
// Global Spaces
// ============================================================================

/// Kernel's IPC space (for kernel tasks)
static KERNEL_SPACE: spin::Once<Arc<IpcSpace>> = spin::Once::new();

/// Initialize the kernel IPC space
pub fn init_kernel_space() {
    KERNEL_SPACE.call_once(|| IpcSpace::with_size(256));
}

/// Get the kernel's IPC space
pub fn kernel_space() -> &'static Arc<IpcSpace> {
    KERNEL_SPACE
        .get()
        .expect("Kernel IPC space not initialized")
}

/// Create a new IPC space for a task
pub fn create_space() -> Arc<IpcSpace> {
    IpcSpace::new()
}

/// Create a new IPC space with specific size
pub fn create_space_with_size(size: usize) -> Arc<IpcSpace> {
    IpcSpace::with_size(size)
}

/// Get the current task's IPC space
///
/// In a full implementation, this would return the space from the current thread's task.
/// For now, we return the kernel space as a fallback.
pub fn current_space() -> Option<Arc<IpcSpace>> {
    // For now, just return kernel space
    // TODO: integrate with scheduler to get current thread's task's space
    KERNEL_SPACE.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_creation() {
        let space = IpcSpace::new();
        assert!(space.is_active());
        assert_eq!(space.entry_count(), 0);
    }

    #[test]
    fn test_space_entry_alloc() {
        let space = IpcSpace::new();

        let name = space.entry_alloc().unwrap();
        assert_eq!(space.entry_count(), 1);

        space.entry_dealloc(name).unwrap();
        assert_eq!(space.entry_count(), 0);
    }
}
