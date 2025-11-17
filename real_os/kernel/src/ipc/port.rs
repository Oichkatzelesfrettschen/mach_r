//! Port implementation - the fundamental IPC primitive

use super::{PortName, IpcError, IpcResult};
use crate::sync::SpinLock;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Maximum messages queued on a port
const MAX_MESSAGES: usize = 256;

/// Port state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Active,
    Dead,
}

/// A Mach-style port
pub struct Port {
    name: PortName,
    state: PortState,
    messages: SpinLock<VecDeque<Box<Message>>>,
    waiting_threads: SpinLock<Vec<ThreadId>>,
    rights: SpinLock<Vec<PortRight>>,
}

impl Port {
    /// Create a new port
    pub fn new() -> Self {
        Self {
            name: PortName::new(),
            state: PortState::Active,
            messages: SpinLock::new(VecDeque::new()),
            waiting_threads: SpinLock::new(Vec::new()),
            rights: SpinLock::new(Vec::new()),
        }
    }
    
    /// Get the port name
    pub fn name(&self) -> PortName {
        self.name
    }
    
    /// Send a message to this port
    pub fn send(&self, msg: Message) -> IpcResult<()> {
        if self.state == PortState::Dead {
            return Err(IpcError::PortDead);
        }
        
        let mut messages = self.messages.lock();
        if messages.len() >= MAX_MESSAGES {
            return Err(IpcError::NoSpace);
        }
        
        messages.push_back(Box::new(msg));
        
        // Wake up any waiting threads
        let mut waiters = self.waiting_threads.lock();
        if let Some(thread_id) = waiters.pop() {
            // Wake up the thread (scheduler integration needed)
            crate::task::wake_thread(thread_id);
        }
        
        Ok(())
    }
    
    /// Receive a message from this port
    pub fn receive(&self, block: bool) -> IpcResult<Message> {
        if self.state == PortState::Dead {
            return Err(IpcError::PortDead);
        }
        
        loop {
            let mut messages = self.messages.lock();
            if let Some(msg) = messages.pop_front() {
                return Ok(*msg);
            }
            
            if !block {
                return Err(IpcError::WouldBlock);
            }
            
            // Block the current thread
            let current = crate::task::current_thread();
            drop(messages); // Release lock before blocking
            
            let mut waiters = self.waiting_threads.lock();
            waiters.push(current);
            drop(waiters);
            
            crate::task::block_current();
            // When we wake up, loop back to try receiving again
        }
    }
    
    /// Destroy this port
    pub fn destroy(&mut self) {
        self.state = PortState::Dead;
        
        // Wake all waiting threads
        let mut waiters = self.waiting_threads.lock();
        for thread_id in waiters.drain(..) {
            crate::task::wake_thread(thread_id);
        }
        
        // Clear messages
        self.messages.lock().clear();
    }
}

// Placeholder types until we implement them
use crate::task::ThreadId;
use super::message::Message;
use super::rights::PortRight;

/// Global port table
static mut PORT_TABLE: Option<SpinLock<Vec<Option<Box<Port>>>>> = None;

/// Initialize port subsystem
pub fn init() {
    unsafe {
        PORT_TABLE = Some(SpinLock::new(Vec::new()));
    }
}

/// Allocate a new port
pub fn allocate_port() -> IpcResult<PortName> {
    let port = Box::new(Port::new());
    let name = port.name();
    
    unsafe {
        if let Some(table) = &PORT_TABLE {
            let mut table = table.lock();
            
            // Find empty slot or extend
            for (i, slot) in table.iter_mut().enumerate() {
                if slot.is_none() {
                    *slot = Some(port);
                    return Ok(name);
                }
            }
            
            // No empty slot, extend table
            table.push(Some(port));
            Ok(name)
        } else {
            Err(IpcError::NoMemory)
        }
    }
}

/// Look up a port by name and apply a function to it
pub fn with_port<F, R>(name: PortName, f: F) -> Option<R>
where
    F: FnOnce(&Port) -> R,
{
    if name.is_null() {
        return None;
    }
    
    unsafe {
        if let Some(table) = &PORT_TABLE {
            let table = table.lock();
            // Simple linear search for now
            for slot in table.iter() {
                if let Some(port) = slot {
                    if port.name() == name {
                        return Some(f(&**port));
                    }
                }
            }
        }
    }
    None
}