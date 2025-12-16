//! Port implementation - the fundamental IPC primitive

use super::kmsg::IpcKmsg;
use super::mqueue::IpcMqueue;
use super::pset::PortSetId;
use super::{IpcError, IpcResult, PortName};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::types::ThreadId;

/// Maximum messages queued on a port
const MAX_MESSAGES: usize = 256;

/// Port state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Active,
    Dead,
}

/// A Mach-style port
#[derive(Debug)]
pub struct Port {
    name: PortName,
    state: PortState,
    messages: Mutex<VecDeque<Box<Message>>>,
    waiting_threads: Mutex<Vec<ThreadId>>,
    rights: Mutex<Vec<PortRight>>,
    /// Kernel message queue (IpcMqueue for proper Mach semantics)
    mqueue: IpcMqueue,
    /// Legacy kernel message queue (deprecated)
    kmsg_queue: Mutex<VecDeque<Box<IpcKmsg>>>,
    /// Send right reference count
    send_rights: AtomicU32,
    /// Send-once right count
    send_once_rights: AtomicU32,
    /// Port set membership (if any)
    port_set: Mutex<Option<PortSetId>>,
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
            mqueue: IpcMqueue::new(),
            kmsg_queue: Mutex::new(VecDeque::new()),
            send_rights: AtomicU32::new(0),
            send_once_rights: AtomicU32::new(0),
            port_set: Mutex::new(None),
        }
    }

    /// Get the port name
    pub fn name(&self) -> PortName {
        self.name
    }

    /// Add a send right to this port
    pub fn add_send_right(&self) {
        self.send_rights.fetch_add(1, Ordering::SeqCst);
    }

    /// Release a send right
    pub fn release_send_right(&self) {
        self.send_rights.fetch_sub(1, Ordering::SeqCst);
    }

    /// Make a new send right (from receive right)
    pub fn make_send_right(&self) {
        self.send_rights.fetch_add(1, Ordering::SeqCst);
    }

    /// Make a new send-once right (from receive right)
    pub fn make_send_once_right(&self) {
        self.send_once_rights.fetch_add(1, Ordering::SeqCst);
    }

    /// Release a send-once right
    pub fn release_send_once_right(&self) {
        self.send_once_rights.fetch_sub(1, Ordering::SeqCst);
    }

    /// Get send right count
    pub fn send_right_count(&self) -> u32 {
        self.send_rights.load(Ordering::SeqCst)
    }

    /// Get send-once right count
    pub fn send_once_right_count(&self) -> u32 {
        self.send_once_rights.load(Ordering::SeqCst)
    }

    /// Set port set membership
    pub fn set_port_set(&self, pset: Option<PortSetId>) {
        *self.port_set.lock() = pset;
    }

    /// Get port set membership
    pub fn port_set(&self) -> Option<PortSetId> {
        *self.port_set.lock()
    }

    /// Enqueue a kernel message
    pub fn enqueue_message(&self, kmsg: Box<IpcKmsg>) {
        self.kmsg_queue.lock().push_back(kmsg);
    }

    /// Dequeue a kernel message
    pub fn dequeue_message(&self) -> Option<Box<IpcKmsg>> {
        self.kmsg_queue.lock().pop_front()
    }

    /// Check if message queue is empty
    pub fn message_queue_empty(&self) -> bool {
        self.kmsg_queue.lock().is_empty()
    }

    /// Get message queue length
    pub fn message_queue_len(&self) -> usize {
        self.kmsg_queue.lock().len()
    }

    /// Check if port is dead
    pub fn is_dead(&self) -> bool {
        self.state == PortState::Dead
    }

    /// Check if port is active
    pub fn is_active(&self) -> bool {
        self.state == PortState::Active
    }

    /// Get reference to the message queue (for read operations)
    pub fn mqueue(&self) -> &IpcMqueue {
        &self.mqueue
    }

    /// Get mutable reference to the message queue (for send/receive)
    pub fn mqueue_mut(&mut self) -> &mut IpcMqueue {
        &mut self.mqueue
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

    unsafe {
        // The entire block is unsafe due to static mut access
        if let Some(table_mutex) = (&raw mut PORT_TABLE).as_mut() {
            let mut table = table_mutex.as_mut().unwrap().lock();

            // Find empty slot or extend
            for slot in table.iter_mut() {
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
        // The entire block is unsafe due to static mut access
        if let Some(table_mutex) = (&raw const PORT_TABLE).as_ref() {
            let table = table_mutex.as_ref().unwrap().lock();
            // Simple linear search for now
            for port in table.iter().flatten() {
                if port.name() == name {
                    return Some(f(port));
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
    })
    .unwrap_or(Err(IpcError::InvalidPort))
}
