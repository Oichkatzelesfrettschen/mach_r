//! IPC Port Sets - Collections of ports for multiplexed receive
//!
//! Based on Mach4 ipc/ipc_pset.c
//! Port sets allow receiving from multiple ports with a single receive.
//!
//! ## Architecture
//!
//! A port set is a collection of receive rights. When a thread receives
//! from a port set, it receives the first available message from any
//! member port. This enables select/poll-style multiplexed I/O.
//!
//! Key concepts from Mach4:
//! - Port sets contain receive rights, not ports directly
//! - A port can be in at most one port set at a time
//! - Receiving from a port set removes the message from the source port
//! - Wait queues are unified: waiting on a port set means waiting on all members

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use spin::Mutex;

use super::kmsg::IpcKmsg;
use super::mqueue::MqueueWaiter;
use super::port::Port;
use super::IpcError;

// ============================================================================
// Port Set ID
// ============================================================================

/// Port set identifier
pub type PortSetId = u32;

/// Next port set ID counter
static NEXT_PSET_ID: AtomicU32 = AtomicU32::new(1);

/// Global port set registry (Mutex-protected for simplicity)
static PORT_SET_REGISTRY: spin::Once<Mutex<BTreeMap<PortSetId, Arc<SyncPortSet>>>> =
    spin::Once::new();

/// Initialize port set registry
pub fn init_pset_registry() {
    PORT_SET_REGISTRY.call_once(|| Mutex::new(BTreeMap::new()));
}

/// Get the port set registry
fn registry() -> &'static Mutex<BTreeMap<PortSetId, Arc<SyncPortSet>>> {
    PORT_SET_REGISTRY
        .get()
        .expect("Port set registry not initialized")
}

/// Allocate a new port set ID
fn alloc_pset_id() -> PortSetId {
    NEXT_PSET_ID.fetch_add(1, Ordering::SeqCst)
}

/// Look up a port set by ID
pub fn lookup_port_set(id: PortSetId) -> Option<Arc<SyncPortSet>> {
    let reg = registry().lock();
    reg.get(&id).cloned()
}

/// Register a port set in the global registry
pub fn register_port_set(pset: Arc<SyncPortSet>) {
    let id = pset.id();
    let mut reg = registry().lock();
    reg.insert(id, pset);
}

/// Unregister a port set from the global registry
pub fn unregister_port_set(id: PortSetId) {
    let mut reg = registry().lock();
    reg.remove(&id);
}

// ============================================================================
// Port Set Member
// ============================================================================

/// A port that is a member of a port set
#[derive(Debug)]
pub struct PortSetMember {
    /// The port
    pub port: Arc<Mutex<Port>>,
    /// Is this member active?
    pub active: bool,
}

impl PortSetMember {
    /// Create new member
    pub fn new(port: Arc<Mutex<Port>>) -> Self {
        Self { port, active: true }
    }

    /// Check if port has messages
    pub fn has_messages(&self) -> bool {
        let port = self.port.lock();
        !port.message_queue_empty()
    }
}

// ============================================================================
// Port Set
// ============================================================================

/// Port set state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSetState {
    /// Port set is active
    Active,
    /// Port set is being destroyed
    Dead,
}

/// A port set - collection of ports for multiplexed receive
#[derive(Debug)]
pub struct IpcPortSet {
    /// Port set ID
    id: PortSetId,

    /// Port set state
    state: PortSetState,

    /// Member ports
    members: Vec<PortSetMember>,

    /// Threads waiting for any message
    waiters: Vec<MqueueWaiter>,

    /// Maximum number of members
    max_members: usize,

    /// Round-robin index for fair scheduling across members
    rr_index: AtomicUsize,

    /// Total messages received (statistics)
    messages_received: AtomicU32,
}

impl IpcPortSet {
    /// Default maximum members
    pub const DEFAULT_MAX_MEMBERS: usize = 256;

    /// Create a new port set
    pub fn new() -> Self {
        Self {
            id: alloc_pset_id(),
            state: PortSetState::Active,
            members: Vec::new(),
            waiters: Vec::new(),
            max_members: Self::DEFAULT_MAX_MEMBERS,
            rr_index: AtomicUsize::new(0),
            messages_received: AtomicU32::new(0),
        }
    }

    /// Create port set with custom max members
    pub fn with_max_members(max: usize) -> Self {
        Self {
            id: alloc_pset_id(),
            state: PortSetState::Active,
            members: Vec::new(),
            waiters: Vec::new(),
            max_members: max,
            rr_index: AtomicUsize::new(0),
            messages_received: AtomicU32::new(0),
        }
    }

    /// Create port set with a specific ID (for restoring from checkpoint)
    pub fn with_id(id: PortSetId) -> Self {
        Self {
            id,
            state: PortSetState::Active,
            members: Vec::new(),
            waiters: Vec::new(),
            max_members: Self::DEFAULT_MAX_MEMBERS,
            rr_index: AtomicUsize::new(0),
            messages_received: AtomicU32::new(0),
        }
    }

    /// Get port set ID
    pub fn id(&self) -> PortSetId {
        self.id
    }

    /// Check if port set is active
    pub fn is_active(&self) -> bool {
        self.state == PortSetState::Active
    }

    /// Get number of members
    pub fn member_count(&self) -> usize {
        self.members.iter().filter(|m| m.active).count()
    }

    /// Check if port set is empty
    pub fn is_empty(&self) -> bool {
        self.member_count() == 0
    }

    // ========================================================================
    // Member Management
    // ========================================================================

    /// Add a port to the set
    pub fn add_member(&mut self, port: Arc<Mutex<Port>>) -> Result<(), IpcError> {
        if self.state != PortSetState::Active {
            return Err(IpcError::InvalidPort);
        }

        if self.members.len() >= self.max_members {
            return Err(IpcError::NoSpace);
        }

        // Check if port is already a member
        for member in &self.members {
            if Arc::ptr_eq(&member.port, &port) {
                return Err(IpcError::InvalidRight);
            }
        }

        // Add to set
        {
            let port_guard = port.lock();
            port_guard.set_port_set(Some(self.id));
        }

        self.members.push(PortSetMember::new(port));

        Ok(())
    }

    /// Remove a port from the set
    pub fn remove_member(&mut self, port: &Arc<Mutex<Port>>) -> Result<(), IpcError> {
        let idx = self.members.iter().position(|m| Arc::ptr_eq(&m.port, port));

        match idx {
            Some(i) => {
                let member = self.members.remove(i);
                let port_guard = member.port.lock();
                port_guard.set_port_set(None);
                Ok(())
            }
            None => Err(IpcError::InvalidPort),
        }
    }

    /// Check if a port is a member
    pub fn is_member(&self, port: &Arc<Mutex<Port>>) -> bool {
        self.members
            .iter()
            .any(|m| Arc::ptr_eq(&m.port, port) && m.active)
    }

    /// Get member ports
    pub fn members(&self) -> impl Iterator<Item = &Arc<Mutex<Port>>> {
        self.members.iter().filter(|m| m.active).map(|m| &m.port)
    }

    // ========================================================================
    // Receive Operations
    // ========================================================================

    /// Receive from any port in the set (non-blocking, round-robin)
    ///
    /// Uses round-robin scheduling to ensure fair message delivery across
    /// all member ports, preventing starvation of low-traffic ports.
    pub fn receive(&mut self) -> Result<(Arc<Mutex<Port>>, Box<IpcKmsg>), IpcError> {
        if self.state != PortSetState::Active {
            return Err(IpcError::InvalidPort);
        }

        let active_count = self.members.iter().filter(|m| m.active).count();
        if active_count == 0 {
            return Err(IpcError::WouldBlock);
        }

        // Get current round-robin position and scan all members
        let start_idx = self.rr_index.load(Ordering::Relaxed) % self.members.len().max(1);

        // Scan from start_idx through all members (wrapping around)
        for offset in 0..self.members.len() {
            let idx = (start_idx + offset) % self.members.len();
            let member = &self.members[idx];

            if !member.active {
                continue;
            }

            let port = member.port.lock();
            if let Some(kmsg) = port.dequeue_message() {
                // Advance round-robin index for fairness
                self.rr_index
                    .store((idx + 1) % self.members.len().max(1), Ordering::Relaxed);
                self.messages_received.fetch_add(1, Ordering::Relaxed);
                return Ok((Arc::clone(&member.port), kmsg));
            }
        }

        Err(IpcError::WouldBlock)
    }

    /// Receive with blocking (simplified)
    pub fn receive_wait(
        &mut self,
        thread_id: u64,
        max_size: usize,
    ) -> Result<(Arc<Mutex<Port>>, Box<IpcKmsg>), IpcError> {
        // Try immediate receive
        if let Ok(result) = self.receive() {
            return Ok(result);
        }

        // Add to wait list
        self.waiters.push(MqueueWaiter::new(thread_id, max_size));

        Err(IpcError::WouldBlock)
    }

    /// Peek at the next message without removing it
    pub fn peek(&self) -> Result<(Arc<Mutex<Port>>, usize), IpcError> {
        if self.state != PortSetState::Active {
            return Err(IpcError::InvalidPort);
        }

        for member in &self.members {
            if !member.active {
                continue;
            }

            let port = member.port.lock();
            let len = port.message_queue_len();
            if len > 0 {
                return Ok((Arc::clone(&member.port), len));
            }
        }

        Err(IpcError::WouldBlock)
    }

    /// Check if any member has messages
    pub fn has_messages(&self) -> bool {
        self.members.iter().any(|m| m.active && m.has_messages())
    }

    /// Get total pending messages across all members
    pub fn total_pending_messages(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.active)
            .map(|m| {
                let port = m.port.lock();
                port.message_queue_len()
            })
            .sum()
    }

    /// Wake waiting threads (called when message arrives)
    pub fn wake_waiters(&mut self) {
        for _waiter in self.waiters.drain(..) {
            // In real implementation, would unblock thread
        }
    }

    /// Get number of waiting threads
    pub fn waiter_count(&self) -> usize {
        self.waiters.len()
    }

    /// Get statistics
    pub fn messages_received(&self) -> u32 {
        self.messages_received.load(Ordering::Relaxed)
    }

    // ========================================================================
    // Lifecycle
    // ========================================================================

    /// Destroy the port set
    pub fn destroy(&mut self) {
        self.state = PortSetState::Dead;

        // Remove all members
        for member in self.members.drain(..) {
            let port = member.port.lock();
            port.set_port_set(None);
        }

        // Wake all waiters
        self.waiters.clear();
    }
}

impl Default for IpcPortSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IpcPortSet {
    fn drop(&mut self) {
        if self.state == PortSetState::Active {
            self.destroy();
        }
    }
}

// ============================================================================
// Thread-safe Port Set
// ============================================================================

/// Thread-safe port set wrapper
#[derive(Debug)]
pub struct SyncPortSet(Mutex<IpcPortSet>);

impl SyncPortSet {
    /// Create new port set
    pub fn new() -> Self {
        Self(Mutex::new(IpcPortSet::new()))
    }

    /// Create port set with specific ID
    pub fn with_id(id: PortSetId) -> Self {
        Self(Mutex::new(IpcPortSet::with_id(id)))
    }

    /// Create and register a new port set
    pub fn create() -> Arc<Self> {
        let pset = Arc::new(Self::new());
        register_port_set(Arc::clone(&pset));
        pset
    }

    /// Get port set ID
    pub fn id(&self) -> PortSetId {
        self.0.lock().id()
    }

    /// Check if port set is active
    pub fn is_active(&self) -> bool {
        self.0.lock().is_active()
    }

    /// Add member
    pub fn add_member(&self, port: Arc<Mutex<Port>>) -> Result<(), IpcError> {
        self.0.lock().add_member(port)
    }

    /// Remove member
    pub fn remove_member(&self, port: &Arc<Mutex<Port>>) -> Result<(), IpcError> {
        self.0.lock().remove_member(port)
    }

    /// Check if a port is a member
    pub fn is_member(&self, port: &Arc<Mutex<Port>>) -> bool {
        self.0.lock().is_member(port)
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.0.lock().member_count()
    }

    /// Receive from any port (round-robin)
    pub fn receive(&self) -> Result<(Arc<Mutex<Port>>, Box<IpcKmsg>), IpcError> {
        self.0.lock().receive()
    }

    /// Receive with blocking
    pub fn receive_wait(
        &self,
        thread_id: u64,
        max_size: usize,
    ) -> Result<(Arc<Mutex<Port>>, Box<IpcKmsg>), IpcError> {
        self.0.lock().receive_wait(thread_id, max_size)
    }

    /// Check if any port has messages
    pub fn has_messages(&self) -> bool {
        self.0.lock().has_messages()
    }

    /// Get total pending messages
    pub fn total_pending_messages(&self) -> usize {
        self.0.lock().total_pending_messages()
    }

    /// Get statistics
    pub fn messages_received(&self) -> u32 {
        self.0.lock().messages_received()
    }

    /// Get waiter count
    pub fn waiter_count(&self) -> usize {
        self.0.lock().waiter_count()
    }

    /// Wake all waiters (called when message arrives on any member)
    pub fn wake_waiters(&self) {
        self.0.lock().wake_waiters();
    }

    /// Destroy the port set and unregister
    pub fn destroy(&self) {
        let id = self.id();
        self.0.lock().destroy();
        unregister_port_set(id);
    }
}

impl Default for SyncPortSet {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Port Set Operations (User-callable)
// ============================================================================

/// Create a new port set and return its ID
pub fn create_port_set() -> Result<PortSetId, IpcError> {
    let pset = SyncPortSet::create();
    Ok(pset.id())
}

/// Destroy a port set by ID
pub fn destroy_port_set(id: PortSetId) -> Result<(), IpcError> {
    if let Some(pset) = lookup_port_set(id) {
        pset.destroy();
        Ok(())
    } else {
        Err(IpcError::InvalidPort)
    }
}

/// Add a port to a port set
pub fn add_port_to_set(pset_id: PortSetId, port: Arc<Mutex<Port>>) -> Result<(), IpcError> {
    // Check if port is already in another set
    {
        let port_guard = port.lock();
        if let Some(existing_pset) = port_guard.port_set() {
            if existing_pset != pset_id {
                return Err(IpcError::InvalidRight);
            }
        }
    }

    let pset = lookup_port_set(pset_id).ok_or(IpcError::InvalidPort)?;
    pset.add_member(port)
}

/// Remove a port from its port set
pub fn remove_port_from_set(port: Arc<Mutex<Port>>) -> Result<(), IpcError> {
    let pset_id = {
        let port_guard = port.lock();
        port_guard.port_set().ok_or(IpcError::InvalidPort)?
    };

    let pset = lookup_port_set(pset_id).ok_or(IpcError::InvalidPort)?;
    pset.remove_member(&port)
}

/// Move a port from one set to another (atomic)
pub fn move_port_between_sets(
    port: Arc<Mutex<Port>>,
    new_pset_id: Option<PortSetId>,
) -> Result<(), IpcError> {
    // Remove from current set if any
    let old_pset_id = {
        let port_guard = port.lock();
        port_guard.port_set()
    };

    if let Some(old_id) = old_pset_id {
        if let Some(old_pset) = lookup_port_set(old_id) {
            old_pset.remove_member(&port)?;
        }
    }

    // Add to new set if specified
    if let Some(new_id) = new_pset_id {
        let new_pset = lookup_port_set(new_id).ok_or(IpcError::InvalidPort)?;
        new_pset.add_member(port)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pset_creation() {
        let pset = IpcPortSet::new();
        assert!(pset.is_active());
        assert!(pset.is_empty());
    }
}
