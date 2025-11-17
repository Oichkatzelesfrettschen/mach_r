//! Mach port implementation - the fundamental IPC primitive
//!
//! Ports are the core abstraction in Mach. They provide:
//! - Unidirectional communication channels
//! - Capability-based security
//! - Message queuing
//! - Notification mechanisms

use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;
use heapless::Vec;
use crate::types::{TaskId, PortId};
use crate::message::{Message, MessageBody};
use crate::message::PortRightType;

/// Maximum number of messages in a port queue
const MAX_MESSAGES_PER_PORT: usize = 256;

/// Port state as defined in Mach
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    /// Port is dead, contains timestamp of death
    Dead { timestamp: u64 },
    /// Port is active with a receiver
    Active { receiver_task: TaskId },
    /// Port is being transferred to another task
    InTransit { destination: PortId },
    /// Port has no receiver or destination
    Limbo,
}

/// Port rights (capabilities)
#[derive(Debug, Clone, Copy, Default)]
pub struct PortRights {
    /// Receive right - exactly one can exist
    pub receive: bool,
    /// Send rights - can have multiple
    pub send_count: u32,
    /// Send-once rights - single use
    pub send_once_count: u32,
}


/// Message queue for a port
pub struct MessageQueue {
    /// Queued messages
    messages: Mutex<Vec<Message, MAX_MESSAGES_PER_PORT>>,
    /// Number of messages in queue
    count: AtomicU32,
    /// Maximum queue size
    limit: usize,
}

impl MessageQueue {
    /// Create a new message queue
    pub fn new(limit: usize) -> Self {
        MessageQueue {
            messages: Mutex::new(Vec::new()),
            count: AtomicU32::new(0),
            limit: limit.min(MAX_MESSAGES_PER_PORT),
        }
    }
    
    /// Enqueue a message
    pub fn enqueue(&self, msg: Message) -> Result<(), Message> {
        let mut queue = self.messages.lock();
        if queue.len() >= self.limit {
            return Err(msg); // Queue full
        }
        queue.push(msg).map_err(|msg| msg)?;
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    /// Dequeue a message
    pub fn dequeue(&self) -> Option<Message> {
        let mut queue = self.messages.lock();
        if let Some(msg) = queue.pop() {
            self.count.fetch_sub(1, Ordering::Relaxed);
            Some(msg)
        } else {
            None
        }
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.count.load(Ordering::Relaxed) == 0
    }
}

/// A Mach port - the fundamental IPC primitive
pub struct Port {
    /// Unique port identifier
    id: PortId,
    
    /// Current port state
    state: Mutex<PortState>,
    
    /// Send rights count
    send_rights: AtomicU32,
    
    /// Send-once rights count
    send_once_rights: AtomicU32,
    
    /// Message queue
    messages: MessageQueue,
    
    /// Sequence number for message ordering
    sequence: AtomicU64,
    
    /// No-senders notification port
    no_senders_notification: Mutex<Option<Arc<Port>>>,
    
    /// Port death notification port
    port_death_notification: Mutex<Option<Arc<Port>>>,
    /// Dead-name subscribers (notified when port becomes dead or on send to dead)
    dead_name_subscribers: Mutex<Vec<Arc<Port>, 8>>,
}

impl Port {
    /// Create a new port
    pub fn new(receiver: TaskId) -> Arc<Self> {
        let p = Arc::new(Port {
            id: PortId::new(),
            state: Mutex::new(PortState::Active { receiver_task: receiver }),
            send_rights: AtomicU32::new(0),
            send_once_rights: AtomicU32::new(0),
            messages: MessageQueue::new(256),
            sequence: AtomicU64::new(0),
            no_senders_notification: Mutex::new(None),
            port_death_notification: Mutex::new(None),
            dead_name_subscribers: Mutex::new(Vec::new()),
        });
        register_global(&p);
        p
    }
    
    /// Get port ID
    pub fn id(&self) -> PortId {
        self.id
    }
    
    /// Get current state
    pub fn state(&self) -> PortState {
        *self.state.lock()
    }
    
    /// Check if port is active
    pub fn is_active(&self) -> bool {
        matches!(*self.state.lock(), PortState::Active { .. })
    }
    
    /// Add a send right
    pub fn add_send_right(&self) {
        self.send_rights.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Remove a send right
    pub fn remove_send_right(&self) -> u32 {
        let count = self.send_rights.fetch_sub(1, Ordering::Relaxed);
        if count == 1 {
            // Last send right removed, trigger no-senders notification
            self.notify_no_senders();
        }
        count - 1
    }
    
    /// Add a send-once right
    pub fn add_send_once_right(&self) {
        self.send_once_rights.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Use a send-once right (consumes it)
    pub fn use_send_once_right(&self) -> bool {
        self.send_once_rights.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |count| if count > 0 { Some(count - 1) } else { None }
        ).is_ok()
    }
    
    /// Send a message to this port
    pub fn send(&self, msg: Message) -> Result<(), Message> {
        if !self.is_active() {
            // Deliver dead-name notifications
            self.notify_port_death();
            return Err(msg); // Port is dead
        }
        // If this is a port-right transfer, apply right semantics
        match &msg.body {
            MessageBody::PortRight { port: _p, right_type } => {
                match right_type {
                    PortRightType::Send => { self.add_send_right(); },
                    PortRightType::CopySend => { self.add_send_right(); },
                    PortRightType::MakeSend => { self.add_send_right(); },
                    PortRightType::SendOnce => { self.add_send_once_right(); },
                    PortRightType::Receive => { /* receive right transfer not modeled */ },
                }
            }
            _ => {}
        }
        self.messages.enqueue(msg)
    }
    
    /// Receive a message from this port
    pub fn receive(&self) -> Option<Message> {
        if !self.is_active() {
            return None; // Port is dead
        }
        self.messages.dequeue()
    }
    
    /// Destroy the port
    pub fn destroy(&self, timestamp: u64) {
        let mut state = self.state.lock();
        *state = PortState::Dead { timestamp };
        // Trigger port death notification
        self.notify_port_death();
    }
    
    /// Set no-senders notification port
    pub fn set_no_senders_notification(&self, port: Option<Arc<Port>>) {
        *self.no_senders_notification.lock() = port;
    }
    
    /// Set port death notification port
    pub fn set_port_death_notification(&self, port: Option<Arc<Port>>) {
        *self.port_death_notification.lock() = port;
    }

    /// Subscribe to dead-name notifications
    pub fn subscribe_dead_name(&self, notify: Arc<Port>) {
        let mut subs = self.dead_name_subscribers.lock();
        let _ = subs.push(notify);
    }
    
    /// Notify no senders
    fn notify_no_senders(&self) {
        if let Some(ref port) = *self.no_senders_notification.lock() {
            // Send notification message
            if let Ok(notification) = Message::new_inline(port.id(), b"no_senders") {
                let _ = port.send(notification);
            }
        }
    }
    
    /// Notify port death
    fn notify_port_death(&self) {
        if let Some(ref port) = *self.port_death_notification.lock() {
            // Send notification message
            if let Ok(notification) = Message::new_inline(port.id(), b"port_death") {
                let _ = port.send(notification);
            }
        }
        for sub in self.dead_name_subscribers.lock().iter() {
            if let Ok(notification) = Message::new_inline(sub.id(), b"dead_name") {
                let _ = sub.send(notification);
            }
        }
    }
}

/// Global port subsystem state
static PORT_SUBSYSTEM_INITIALIZED: AtomicU32 = AtomicU32::new(0);

/// Initialize the port subsystem
pub fn init() {
    PORT_SUBSYSTEM_INITIALIZED.store(1, Ordering::Relaxed);
    // Additional initialization as needed
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc as StdArc;
    
    #[test]
    fn test_port_creation() {
        let task_id = TaskId(1);
        let port = Port::new(task_id);
        
        assert!(port.is_active());
        assert_eq!(port.state(), PortState::Active { receiver_task: task_id });
    }
    
    #[test]
    fn test_send_rights() {
        let port = Port::new(TaskId(1));
        
        port.add_send_right();
        port.add_send_right();
        assert_eq!(port.send_rights.load(Ordering::Relaxed), 2);
        
        port.remove_send_right();
        assert_eq!(port.send_rights.load(Ordering::Relaxed), 1);
    }
    
    #[test]
    fn test_message_send_receive() {
        let port = Port::new(TaskId(1));
        
        let msg = Message::new_inline(port.id(), b"test").unwrap();
        
        assert!(port.send(msg).is_ok());
        assert!(port.receive().is_some());
        assert!(port.receive().is_none()); // Queue should be empty
    }
    
    #[test]
    fn test_port_destruction() {
        let port = Port::new(TaskId(1));
        
        port.destroy(12345);
        assert!(!port.is_active());
        assert_eq!(port.state(), PortState::Dead { timestamp: 12345 });
        
        // Can't send to dead port
        let msg = Message::new_inline(port.id(), b"test").unwrap();
        assert!(port.send(msg).is_err());
    }

    #[test]
    fn no_senders_notification_triggers() {
        let t = TaskId(1);
        let notify = Port::new(t);
        let p = Port::new(t);
        p.set_no_senders_notification(Some(StdArc::clone(&notify)));
        // Add then remove a single send right
        p.add_send_right();
        let remaining = p.remove_send_right();
        assert_eq!(remaining, 0);
        // Notification should be enqueued on notify port
        let msg = notify.receive();
        assert!(msg.is_some());
        if let Some(m) = msg { assert_eq!(m.data(), b"no_senders"); }
    }

    #[test]
    fn dead_name_notification_on_send_to_dead() {
        let t = TaskId(1);
        let subscriber = Port::new(t);
        let p = Port::new(t);
        p.subscribe_dead_name(StdArc::clone(&subscriber));
        p.destroy(999);
        // Attempt to send after death triggers dead_name notification
        let msg = Message::new_inline(p.id(), b"x").unwrap();
        assert!(p.send(msg).is_err());
        let note = subscriber.receive();
        assert!(note.is_some());
        if let Some(m) = note { assert_eq!(m.data(), b"dead_name"); }
    }
}

/// Port registry for name resolution
pub struct PortRegistry {
    ports: Mutex<alloc::collections::BTreeMap<alloc::string::String, PortId>>,
}

impl PortRegistry {
    pub const fn new() -> Self {
        Self {
            ports: Mutex::new(alloc::collections::BTreeMap::new()),
        }
    }
    
    pub fn register_port(&self, name: &str, port_id: PortId) {
        let mut ports = self.ports.lock();
        ports.insert(alloc::string::String::from(name), port_id);
    }
    
    pub fn lookup_port(&self, name: &str) -> Option<PortId> {
        let ports = self.ports.lock();
        ports.get(name).copied()
    }
    
    pub fn unregister_port(&self, name: &str) -> Option<PortId> {
        let mut ports = self.ports.lock();
        ports.remove(name)
    }
}

/// Global port registry
pub static PORT_REGISTRY: PortRegistry = PortRegistry::new();

/// Global Port table mapping `PortId` to `Arc<Port>` for routing
static PORT_TABLE: Mutex<BTreeMap<PortId, Arc<Port>>> = Mutex::new(BTreeMap::new());

fn register_global(port: &Arc<Port>) {
    let mut tbl = PORT_TABLE.lock();
    tbl.insert(port.id(), Arc::clone(port));
}

fn lookup_port_by_id(id: PortId) -> Option<Arc<Port>> {
    PORT_TABLE.lock().get(&id).cloned()
}

/// Send a message to a port
pub fn send_message(port_id: PortId, message: Message) -> Result<(), ()> {
    if let Some(port) = lookup_port_by_id(port_id) {
        port.send(message).map_err(|_| ())
    } else {
        Err(())
    }
}
