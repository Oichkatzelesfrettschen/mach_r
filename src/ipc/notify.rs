//! IPC Notifications - Port death and other notifications
//!
//! Based on Mach4 ipc/ipc_notify.c
//! Handles dead-name and no-senders notifications.

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use super::entry::MachPortName;
use super::kmsg::{kmsg_alloc, MachMsgHeader};
use super::port::Port;
use super::space::{IpcSpace, SpaceId};
use super::IpcError;

// ============================================================================
// Notification Types
// ============================================================================

/// Types of port notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum NotifyType {
    /// Port was destroyed (dead name notification)
    DeadName = 0x0041,
    /// No more send rights exist (no-senders notification)
    NoSenders = 0x0046,
    /// Send-once right was used (send-once notification)
    SendOnce = 0x0047,
    /// Port set no longer empty
    PortDeleted = 0x0048,
    /// Port destroyed for receiver
    PortDestroyed = 0x0049,
}

impl NotifyType {
    /// Get message ID for this notification type
    pub fn msg_id(self) -> i32 {
        self as i32
    }
}

// ============================================================================
// Notification Request
// ============================================================================

/// A pending notification request
#[derive(Debug, Clone)]
pub struct NotifyRequest {
    /// Type of notification requested
    pub notify_type: NotifyType,
    /// Port to send notification to
    pub notify_port: Arc<Mutex<Port>>,
    /// Name of the port being watched
    pub watched_name: MachPortName,
    /// Space that made the request
    pub space_id: SpaceId,
}

impl NotifyRequest {
    /// Create new notification request
    pub fn new(
        notify_type: NotifyType,
        notify_port: Arc<Mutex<Port>>,
        watched_name: MachPortName,
        space_id: SpaceId,
    ) -> Self {
        Self {
            notify_type,
            notify_port,
            watched_name,
            space_id,
        }
    }
}

// ============================================================================
// Notification Manager
// ============================================================================

/// Manages pending notification requests
#[derive(Debug)]
pub struct NotifyManager {
    /// Pending dead-name requests
    dead_name_requests: Vec<NotifyRequest>,
    /// Pending no-senders requests
    no_senders_requests: Vec<NotifyRequest>,
    /// Pending port-destroyed requests
    port_destroyed_requests: Vec<NotifyRequest>,
}

impl NotifyManager {
    /// Create new notification manager
    pub const fn new() -> Self {
        Self {
            dead_name_requests: Vec::new(),
            no_senders_requests: Vec::new(),
            port_destroyed_requests: Vec::new(),
        }
    }

    /// Register a dead-name notification request
    pub fn request_dead_name(
        &mut self,
        notify_port: Arc<Mutex<Port>>,
        watched_name: MachPortName,
        space_id: SpaceId,
    ) {
        self.dead_name_requests.push(NotifyRequest::new(
            NotifyType::DeadName,
            notify_port,
            watched_name,
            space_id,
        ));
    }

    /// Register a no-senders notification request
    pub fn request_no_senders(
        &mut self,
        notify_port: Arc<Mutex<Port>>,
        watched_name: MachPortName,
        space_id: SpaceId,
    ) {
        self.no_senders_requests.push(NotifyRequest::new(
            NotifyType::NoSenders,
            notify_port,
            watched_name,
            space_id,
        ));
    }

    /// Register a port-destroyed notification request
    pub fn request_port_destroyed(
        &mut self,
        notify_port: Arc<Mutex<Port>>,
        watched_name: MachPortName,
        space_id: SpaceId,
    ) {
        self.port_destroyed_requests.push(NotifyRequest::new(
            NotifyType::PortDestroyed,
            notify_port,
            watched_name,
            space_id,
        ));
    }

    /// Cancel a notification request
    pub fn cancel_request(&mut self, watched_name: MachPortName, space_id: SpaceId) {
        self.dead_name_requests
            .retain(|r| r.watched_name != watched_name || r.space_id != space_id);
        self.no_senders_requests
            .retain(|r| r.watched_name != watched_name || r.space_id != space_id);
        self.port_destroyed_requests
            .retain(|r| r.watched_name != watched_name || r.space_id != space_id);
    }

    /// Send dead-name notifications for a port
    pub fn send_dead_name_notifications(&mut self, watched_name: MachPortName) {
        let requests: Vec<_> = self
            .dead_name_requests
            .drain(..)
            .filter(|r| r.watched_name == watched_name)
            .collect();

        for request in requests {
            let _ = send_notification(&request.notify_port, NotifyType::DeadName, watched_name);
        }
    }

    /// Send no-senders notification
    pub fn send_no_senders_notification(&mut self, watched_name: MachPortName) {
        let requests: Vec<_> = self
            .no_senders_requests
            .drain(..)
            .filter(|r| r.watched_name == watched_name)
            .collect();

        for request in requests {
            let _ = send_notification(&request.notify_port, NotifyType::NoSenders, watched_name);
        }
    }

    /// Send port-destroyed notification
    pub fn send_port_destroyed_notification(&mut self, watched_name: MachPortName) {
        let requests: Vec<_> = self
            .port_destroyed_requests
            .drain(..)
            .filter(|r| r.watched_name == watched_name)
            .collect();

        for request in requests {
            let _ = send_notification(&request.notify_port, NotifyType::PortDestroyed, watched_name);
        }
    }

    /// Clean up all requests for a space
    pub fn cleanup_space(&mut self, space_id: SpaceId) {
        self.dead_name_requests.retain(|r| r.space_id != space_id);
        self.no_senders_requests.retain(|r| r.space_id != space_id);
        self.port_destroyed_requests.retain(|r| r.space_id != space_id);
    }
}

impl Default for NotifyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Notification Messages
// ============================================================================

/// Dead-name notification message body
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DeadNameNotify {
    /// The name that became dead
    pub not_port: MachPortName,
}

/// No-senders notification message body
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NoSendersNotify {
    /// Send-right count at time of notification
    pub not_count: u32,
}

/// Send a notification message
fn send_notification(
    notify_port: &Arc<Mutex<Port>>,
    notify_type: NotifyType,
    port_name: MachPortName,
) -> Result<(), IpcError> {
    // Create notification message
    let mut kmsg = kmsg_alloc(64);

    // Set up header
    // In a full implementation, we'd properly format the notification message
    // For now, just set the message ID
    let _header = MachMsgHeader {
        msgh_bits: 0,
        msgh_size: 32,
        msgh_remote_port: 0,
        msgh_local_port: 0,
        msgh_reserved: 0,
        msgh_id: notify_type.msg_id(),
    };

    // Set up body based on notification type
    match notify_type {
        NotifyType::DeadName => {
            let body = DeadNameNotify {
                not_port: port_name,
            };
            kmsg.body_mut()
                .extend_from_slice(&body.not_port.to_ne_bytes());
        }
        NotifyType::NoSenders => {
            let body = NoSendersNotify { not_count: 0 };
            kmsg.body_mut()
                .extend_from_slice(&body.not_count.to_ne_bytes());
        }
        _ => {}
    }

    // Send to notification port
    let port = notify_port.lock();
    port.enqueue_message(kmsg);

    Ok(())
}

// ============================================================================
// Global Notification Manager
// ============================================================================

/// Global notification manager
static NOTIFY_MANAGER: spin::Once<Mutex<NotifyManager>> = spin::Once::new();

/// Initialize the notification subsystem
pub fn init() {
    NOTIFY_MANAGER.call_once(|| Mutex::new(NotifyManager::new()));
}

/// Get the notification manager
pub fn manager() -> &'static Mutex<NotifyManager> {
    NOTIFY_MANAGER
        .get()
        .expect("Notify manager not initialized")
}

/// Request dead-name notification
pub fn request_dead_name(
    notify_port: Arc<Mutex<Port>>,
    watched_name: MachPortName,
    space: &IpcSpace,
) {
    let mut mgr = manager().lock();
    mgr.request_dead_name(notify_port, watched_name, space.id());
}

/// Request no-senders notification
pub fn request_no_senders(
    notify_port: Arc<Mutex<Port>>,
    watched_name: MachPortName,
    space: &IpcSpace,
) {
    let mut mgr = manager().lock();
    mgr.request_no_senders(notify_port, watched_name, space.id());
}

/// Trigger dead-name notifications
pub fn trigger_dead_name(port_name: MachPortName) {
    let mut mgr = manager().lock();
    mgr.send_dead_name_notifications(port_name);
}

/// Trigger no-senders notification
pub fn trigger_no_senders(port_name: MachPortName) {
    let mut mgr = manager().lock();
    mgr.send_no_senders_notification(port_name);
}

/// Request port-destroyed notification
pub fn request_port_destroyed(
    notify_port: Arc<Mutex<Port>>,
    watched_name: MachPortName,
    space: &IpcSpace,
) {
    let mut mgr = manager().lock();
    mgr.request_port_destroyed(notify_port, watched_name, space.id());
}

/// Trigger port-destroyed notification
pub fn trigger_port_destroyed(port_name: MachPortName) {
    let mut mgr = manager().lock();
    mgr.send_port_destroyed_notification(port_name);
}
