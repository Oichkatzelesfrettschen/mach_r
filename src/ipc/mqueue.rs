//! IPC Message Queue - Port message queue management
//!
//! Based on Mach4 ipc/ipc_mqueue.c
//! Each port has a message queue for pending messages.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use spin::Mutex;

use super::kmsg::IpcKmsg;
use super::IpcError;

// ============================================================================
// Queue Limits
// ============================================================================

/// Default queue limit (number of messages)
pub const MQUEUE_DEFAULT_LIMIT: usize = 5;

/// Maximum queue limit
pub const MQUEUE_MAX_LIMIT: usize = 1024;

/// No limit sentinel
pub const MQUEUE_NO_LIMIT: usize = usize::MAX;

// ============================================================================
// Waiter for blocking receive
// ============================================================================

/// Thread waiting on a message queue
#[derive(Debug)]
pub struct MqueueWaiter {
    /// Thread ID waiting
    pub thread_id: u64,
    /// Maximum message size the thread can receive
    pub max_size: usize,
    /// Wake-up channel (in real impl, would be thread/event)
    pub wakeup: bool,
}

impl MqueueWaiter {
    /// Create new waiter
    pub fn new(thread_id: u64, max_size: usize) -> Self {
        Self {
            thread_id,
            max_size,
            wakeup: false,
        }
    }

    /// Wake up this waiter
    pub fn wake(&mut self) {
        self.wakeup = true;
    }

    /// Check if waiter has been woken
    pub fn is_woken(&self) -> bool {
        self.wakeup
    }
}

// ============================================================================
// Message Queue
// ============================================================================

/// Message queue state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqueueState {
    /// Queue is active and accepting messages
    Active,
    /// Queue is full (at limit)
    Full,
    /// Queue is being destroyed
    Dead,
}

/// Message queue for a port
///
/// From Mach4:
/// - Messages are stored in a FIFO queue
/// - Queue has a limit on number of messages
/// - Threads can wait for messages (receive) or for space (send)
#[derive(Debug)]
pub struct IpcMqueue {
    /// Queue state
    state: MqueueState,

    /// Message queue (FIFO)
    messages: VecDeque<Box<IpcKmsg>>,

    /// Maximum number of messages
    limit: usize,

    /// Threads waiting for messages (receivers)
    receivers: VecDeque<MqueueWaiter>,

    /// Threads waiting for space (senders)
    senders: VecDeque<MqueueWaiter>,

    /// Sequence number for ordering
    seqno: u64,

    /// Port set membership (if any)
    port_set: Option<u32>,
}

impl IpcMqueue {
    /// Create a new message queue
    pub fn new() -> Self {
        Self::with_limit(MQUEUE_DEFAULT_LIMIT)
    }

    /// Create a queue with specified limit
    pub fn with_limit(limit: usize) -> Self {
        Self {
            state: MqueueState::Active,
            messages: VecDeque::new(),
            limit: limit.min(MQUEUE_MAX_LIMIT),
            receivers: VecDeque::new(),
            senders: VecDeque::new(),
            seqno: 0,
            port_set: None,
        }
    }

    /// Get queue state
    pub fn state(&self) -> MqueueState {
        self.state
    }

    /// Check if queue is active
    pub fn is_active(&self) -> bool {
        self.state == MqueueState::Active
    }

    /// Check if queue is full
    pub fn is_full(&self) -> bool {
        self.messages.len() >= self.limit
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get number of messages in queue
    pub fn count(&self) -> usize {
        self.messages.len()
    }

    /// Get queue limit
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Set queue limit
    pub fn set_limit(&mut self, new_limit: usize) {
        self.limit = new_limit.min(MQUEUE_MAX_LIMIT);

        // Update state
        if self.is_full() {
            self.state = MqueueState::Full;
        } else if self.state == MqueueState::Full {
            self.state = MqueueState::Active;
            // Wake up waiting senders
            self.wake_one_sender();
        }
    }

    /// Get next sequence number
    pub fn next_seqno(&mut self) -> u64 {
        let seqno = self.seqno;
        self.seqno += 1;
        seqno
    }

    // ========================================================================
    // Send Operations
    // ========================================================================

    /// Enqueue a message (non-blocking)
    pub fn send(&mut self, kmsg: Box<IpcKmsg>) -> Result<(), (IpcError, Box<IpcKmsg>)> {
        if self.state == MqueueState::Dead {
            return Err((IpcError::PortDead, kmsg));
        }

        if self.is_full() {
            return Err((IpcError::NoSpace, kmsg));
        }

        self.messages.push_back(kmsg);

        // Update state
        if self.is_full() {
            self.state = MqueueState::Full;
        }

        // Wake up a waiting receiver
        self.wake_one_receiver();

        Ok(())
    }

    /// Try to enqueue, waiting if necessary
    pub fn send_wait(
        &mut self,
        kmsg: Box<IpcKmsg>,
        thread_id: u64,
    ) -> Result<(), (IpcError, Box<IpcKmsg>)> {
        if self.state == MqueueState::Dead {
            return Err((IpcError::PortDead, kmsg));
        }

        if !self.is_full() {
            return self.send(kmsg);
        }

        // Queue is full - add to wait list
        self.senders.push_back(MqueueWaiter::new(thread_id, 0));

        // In a real implementation, the thread would block here
        // and the message would be sent when woken up

        Err((IpcError::WouldBlock, kmsg))
    }

    // ========================================================================
    // Receive Operations
    // ========================================================================

    /// Dequeue a message (non-blocking)
    pub fn receive(&mut self) -> Result<Box<IpcKmsg>, IpcError> {
        if self.state == MqueueState::Dead && self.messages.is_empty() {
            return Err(IpcError::PortDead);
        }

        match self.messages.pop_front() {
            Some(kmsg) => {
                // Update state
                if self.state == MqueueState::Full {
                    self.state = MqueueState::Active;
                    // Wake up a waiting sender
                    self.wake_one_sender();
                }
                Ok(kmsg)
            }
            None => Err(IpcError::WouldBlock),
        }
    }

    /// Try to receive, waiting if necessary
    pub fn receive_wait(
        &mut self,
        thread_id: u64,
        max_size: usize,
    ) -> Result<Box<IpcKmsg>, IpcError> {
        // Try immediate receive first
        if let Ok(kmsg) = self.receive() {
            return Ok(kmsg);
        }

        if self.state == MqueueState::Dead {
            return Err(IpcError::PortDead);
        }

        // Add to wait list
        self.receivers
            .push_back(MqueueWaiter::new(thread_id, max_size));

        // In a real implementation, the thread would block here
        Err(IpcError::WouldBlock)
    }

    /// Peek at first message without removing
    pub fn peek(&self) -> Option<&IpcKmsg> {
        self.messages.front().map(|b| b.as_ref())
    }

    // ========================================================================
    // Waiter Management
    // ========================================================================

    /// Wake one waiting receiver
    fn wake_one_receiver(&mut self) {
        if let Some(mut waiter) = self.receivers.pop_front() {
            waiter.wake();
            // In a real implementation, this would unblock the thread
        }
    }

    /// Wake one waiting sender
    fn wake_one_sender(&mut self) {
        if let Some(mut waiter) = self.senders.pop_front() {
            waiter.wake();
            // In a real implementation, this would unblock the thread
        }
    }

    /// Wake all waiting receivers (e.g., on port death)
    fn wake_all_receivers(&mut self) {
        for mut waiter in self.receivers.drain(..) {
            waiter.wake();
        }
    }

    /// Wake all waiting senders (e.g., on port death)
    fn wake_all_senders(&mut self) {
        for mut waiter in self.senders.drain(..) {
            waiter.wake();
        }
    }

    /// Get number of waiting receivers
    pub fn receiver_count(&self) -> usize {
        self.receivers.len()
    }

    /// Get number of waiting senders
    pub fn sender_count(&self) -> usize {
        self.senders.len()
    }

    // ========================================================================
    // Port Set Support
    // ========================================================================

    /// Add this queue to a port set
    pub fn add_to_set(&mut self, set_id: u32) {
        self.port_set = Some(set_id);
    }

    /// Remove from port set
    pub fn remove_from_set(&mut self) {
        self.port_set = None;
    }

    /// Get port set ID if any
    pub fn port_set(&self) -> Option<u32> {
        self.port_set
    }

    // ========================================================================
    // Lifecycle
    // ========================================================================

    /// Destroy the queue
    pub fn destroy(&mut self) {
        self.state = MqueueState::Dead;

        // Wake all waiters with error
        self.wake_all_receivers();
        self.wake_all_senders();

        // Clean up all queued messages
        for mut kmsg in self.messages.drain(..) {
            kmsg.clean();
        }
    }

    /// Drain all messages (for receive right transfer)
    pub fn drain(&mut self) -> impl Iterator<Item = Box<IpcKmsg>> + '_ {
        // Update state
        if self.state == MqueueState::Full {
            self.state = MqueueState::Active;
        }

        // Wake all senders since queue is empty
        self.wake_all_senders();

        self.messages.drain(..)
    }
}

impl Default for IpcMqueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IpcMqueue {
    fn drop(&mut self) {
        self.destroy();
    }
}

// ============================================================================
// Thread-safe wrapper
// ============================================================================

/// Thread-safe message queue
#[derive(Debug)]
pub struct SyncMqueue(Mutex<IpcMqueue>);

impl SyncMqueue {
    /// Create new synchronized queue
    pub fn new() -> Self {
        Self(Mutex::new(IpcMqueue::new()))
    }

    /// Create with limit
    pub fn with_limit(limit: usize) -> Self {
        Self(Mutex::new(IpcMqueue::with_limit(limit)))
    }

    /// Send a message
    pub fn send(&self, kmsg: Box<IpcKmsg>) -> Result<(), (IpcError, Box<IpcKmsg>)> {
        self.0.lock().send(kmsg)
    }

    /// Receive a message
    pub fn receive(&self) -> Result<Box<IpcKmsg>, IpcError> {
        self.0.lock().receive()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.0.lock().is_empty()
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.0.lock().count()
    }

    /// Get state
    pub fn state(&self) -> MqueueState {
        self.0.lock().state()
    }

    /// Set limit
    pub fn set_limit(&self, limit: usize) {
        self.0.lock().set_limit(limit);
    }

    /// Destroy
    pub fn destroy(&self) {
        self.0.lock().destroy();
    }
}

impl Default for SyncMqueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::kmsg::kmsg_alloc_small;
    use super::*;

    #[test]
    fn test_mqueue_basic() {
        let mut queue = IpcMqueue::new();
        assert!(queue.is_empty());
        assert!(queue.is_active());

        let kmsg = kmsg_alloc_small();
        queue.send(kmsg).unwrap();
        assert_eq!(queue.count(), 1);

        let _received = queue.receive().unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_mqueue_limit() {
        let mut queue = IpcMqueue::with_limit(2);

        let kmsg1 = kmsg_alloc_small();
        let kmsg2 = kmsg_alloc_small();
        let kmsg3 = kmsg_alloc_small();

        queue.send(kmsg1).unwrap();
        queue.send(kmsg2).unwrap();

        // Should fail - queue full
        let result = queue.send(kmsg3);
        assert!(result.is_err());
    }
}
