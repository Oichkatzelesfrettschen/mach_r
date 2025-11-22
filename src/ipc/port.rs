//! Port implementation - the fundamental IPC primitive

use super::{PortName, IpcError, IpcResult};
use spin::Mutex;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::boxed::Box;

use crate::types::ThreadId;
use crate::println;

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
    messages: Mutex<VecDeque<Box<Message>>>,
    waiting_threads: Mutex<Vec<ThreadId>>,
    rights: Mutex<Vec<PortRight>>,
}

impl Port {
    /// Create a new port
    pub fn new() -> Self {
        Self {
            name: PortName::new(),
            state: PortState::Active,
            messages: Mutex::new(VecDeque::new()),
            waiting_threads: Mutex::new(Vec::new()),
            rights: Mutex::new(Vec::new()),
        }
    }
    
    /// Get the port name
    pub fn name(&self) -> PortName {
        self.name
    }

    /// Add a send right to this port (placeholder implementation)
    pub fn add_send_right(&self) {
        // This is a placeholder. Proper implementation requires finding/adding a PortRight to self.rights
        println!("TODO: Implement add_send_right for Port {}", self.name().id());
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
            crate::scheduler::wake_thread(thread_id);
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
            let current = crate::scheduler::current_thread();
            drop(messages); // Release lock before blocking
            
            let mut waiters = self.waiting_threads.lock();
            if let Some(current_thread_arc) = current {
                waiters.push(current_thread_arc.thread_id);
            } else {
                // This scenario indicates a problem in kernel logic if current_thread() is None
                // when a thread is supposed to be blocked.
                // For now, panic or return an error.
                return Err(IpcError::InvalidThread);
            }
            drop(waiters);
            
            crate::scheduler::block_current();
            // When we wake up, loop back to try receiving again
        }
    }
    
    /// Destroy this port
    pub fn destroy(&mut self) {
        self.state = PortState::Dead;
        
        // Wake all waiting threads
        let mut waiters = self.waiting_threads.lock();
        for thread_id in waiters.drain(..) {
            crate::scheduler::wake_thread(thread_id);
        }
        
        // Clear messages
        self.messages.lock().clear();
    }
}

// Placeholder types until we implement them

use super::message::Message;
use super::rights::PortRight;

/// Global port table
static mut PORT_TABLE: Option<Mutex<Vec<Option<Box<Port>>>>> = None;

/// Initialize port subsystem
pub fn init() {
    unsafe {
        PORT_TABLE = Some(Mutex::new(Vec::new()));
    }
}

/// Allocate a new port
pub fn allocate_port() -> IpcResult<PortName> {
    let port = Box::new(Port::new());
    let name = port.name();
    
    unsafe { // The entire block is unsafe due to static mut access
        if let Some(table_mutex) = (&raw mut PORT_TABLE).as_mut() {
            let mut table = table_mutex.as_mut().unwrap().lock();
            
            // Find empty slot or extend
            for (_i, slot) in table.iter_mut().enumerate() {
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
    
    unsafe { // The entire block is unsafe due to static mut access
        if let Some(table_mutex) = (&raw const PORT_TABLE).as_ref() {
            let table = table_mutex.as_ref().unwrap().lock();
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

/// Add a send right to a port
pub fn add_send_right(port_name: PortName) -> IpcResult<()> {
    with_port(port_name, |port| {
        port.add_send_right();
        Ok(())
    }).unwrap_or(Err(IpcError::InvalidPort))
}