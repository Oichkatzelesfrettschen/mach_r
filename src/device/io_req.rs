//! I/O Request - Asynchronous I/O Management
//!
//! Based on Mach4 device/io_req.h/c
//! Manages I/O requests queued on devices for asynchronous operations.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use crate::device::dev_hdr::DeviceId;
use crate::ipc::PortName;

// ============================================================================
// I/O Operation Flags
// ============================================================================

/// I/O operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoOp(u32);

impl IoOp {
    /// Write operation
    pub const WRITE: Self = Self(0x00000000);
    /// Read operation
    pub const READ: Self = Self(0x00000001);
    /// Open operation
    pub const OPEN: Self = Self(0x00000002);
    /// Operation complete
    pub const DONE: Self = Self(0x00000100);
    /// Error on operation
    pub const ERROR: Self = Self(0x00000200);
    /// Operation in progress
    pub const BUSY: Self = Self(0x00000400);
    /// Wakeup when no longer busy
    pub const WANTED: Self = Self(0x00000800);
    /// Bad disk block
    pub const BAD: Self = Self(0x00001000);
    /// Call io_done_thread when done
    pub const CALL: Self = Self(0x00002000);
    /// MIG call was inband
    pub const INBAND: Self = Self(0x00004000);
    /// Internal, driver-specific
    pub const INTERNAL: Self = Self(0x00008000);
    /// IOR loaned by another module
    pub const LOANED: Self = Self(0x00010000);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(&self) -> u32 {
        self.0
    }

    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn is_read(&self) -> bool {
        (self.0 & 0x1) != 0
    }

    pub const fn is_write(&self) -> bool {
        (self.0 & 0x1) == 0
    }

    pub const fn is_done(&self) -> bool {
        (self.0 & Self::DONE.0) != 0
    }

    pub const fn is_error(&self) -> bool {
        (self.0 & Self::ERROR.0) != 0
    }
}

impl core::ops::BitOr for IoOp {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for IoOp {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Default for IoOp {
    fn default() -> Self {
        Self::empty()
    }
}

// ============================================================================
// I/O Mode
// ============================================================================

/// I/O mode flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoMode(u32);

impl IoMode {
    /// Wait for completion
    pub const WAIT: Self = Self(0x0001);
    /// Truncate on error
    pub const TRUNCATE: Self = Self(0x0002);
    /// Don't block
    pub const NOWAIT: Self = Self(0x0004);
    /// Raw I/O (no buffering)
    pub const RAW: Self = Self(0x0008);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(&self) -> u32 {
        self.0
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Default for IoMode {
    fn default() -> Self {
        Self::empty()
    }
}

// ============================================================================
// I/O Return Status
// ============================================================================

/// I/O return status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum IoReturn {
    /// Success
    #[default]
    Success = 0,
    /// Invalid argument
    InvalidArg = -1,
    /// Device not ready
    NotReady = -2,
    /// I/O error
    Error = -3,
    /// No such device
    NoDevice = -4,
    /// Device offline
    Offline = -5,
    /// Bad sector/block
    BadSector = -6,
    /// Would block
    WouldBlock = -7,
    /// Operation aborted
    Aborted = -8,
    /// Invalid operation
    InvalidOp = -9,
    /// Resource shortage
    ResourceShortage = -10,
    /// Device overrun
    Overrun = -11,
}

// ============================================================================
// I/O Request
// ============================================================================

/// I/O Request identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IoReqId(pub u64);

/// I/O Request
///
/// Based on struct io_req from Mach4.
/// Queued on device for delayed replies.
#[derive(Debug)]
pub struct IoRequest {
    /// Request ID
    pub id: IoReqId,

    /// Device this request is for
    pub device_id: DeviceId,

    /// Unit number (minor device)
    pub unit: u32,

    /// I/O operation
    pub op: AtomicU32,

    /// Operation mode
    pub mode: IoMode,

    /// Starting record number (for random-access devices)
    pub recnum: u64,

    /// Data buffer
    pub data: Mutex<Vec<u8>>,

    /// Amount requested
    pub count: u64,

    /// Amount allocated
    pub alloc_size: u64,

    /// Amount NOT done (residual)
    pub residual: AtomicU32,

    /// Error code
    pub error: Mutex<IoReturn>,

    /// Reply port (for async messages)
    pub reply_port: Mutex<Option<PortName>>,

    /// Reply port type (send or send-once)
    pub reply_port_type: u32,

    /// Total operation size (for write)
    pub total: u64,

    /// Is request busy?
    pub busy: AtomicBool,

    /// Physical record number mapping
    pub physrec: u64,

    /// Total number of blocks to move
    pub rectotal: u64,
}

impl IoRequest {
    /// Create a new I/O request
    pub fn new(id: IoReqId, device_id: DeviceId, unit: u32) -> Self {
        Self {
            id,
            device_id,
            unit,
            op: AtomicU32::new(0),
            mode: IoMode::empty(),
            recnum: 0,
            data: Mutex::new(Vec::new()),
            count: 0,
            alloc_size: 0,
            residual: AtomicU32::new(0),
            error: Mutex::new(IoReturn::Success),
            reply_port: Mutex::new(None),
            reply_port_type: 0,
            total: 0,
            busy: AtomicBool::new(false),
            physrec: 0,
            rectotal: 0,
        }
    }

    /// Create a read request
    pub fn read(id: IoReqId, device_id: DeviceId, unit: u32, recnum: u64, count: u64) -> Self {
        let mut req = Self::new(id, device_id, unit);
        req.op.store(IoOp::READ.0, Ordering::SeqCst);
        req.recnum = recnum;
        req.count = count;
        req.alloc_size = count;
        *req.data.lock() = vec![0u8; count as usize];
        req
    }

    /// Create a write request
    pub fn write(id: IoReqId, device_id: DeviceId, unit: u32, recnum: u64, data: Vec<u8>) -> Self {
        let mut req = Self::new(id, device_id, unit);
        req.op.store(IoOp::WRITE.0, Ordering::SeqCst);
        req.recnum = recnum;
        req.count = data.len() as u64;
        req.alloc_size = data.len() as u64;
        req.total = data.len() as u64;
        *req.data.lock() = data;
        req
    }

    /// Get operation
    pub fn get_op(&self) -> IoOp {
        IoOp::from_bits_truncate(self.op.load(Ordering::SeqCst))
    }

    /// Set operation flags
    pub fn set_op(&self, op: IoOp) {
        self.op.fetch_or(op.bits(), Ordering::SeqCst);
    }

    /// Clear operation flags
    pub fn clear_op(&self, op: IoOp) {
        self.op.fetch_and(!op.bits(), Ordering::SeqCst);
    }

    /// Check if request is done
    pub fn is_done(&self) -> bool {
        self.get_op().is_done()
    }

    /// Check if request has error
    pub fn has_error(&self) -> bool {
        self.get_op().is_error()
    }

    /// Mark request as done
    pub fn mark_done(&self) {
        self.set_op(IoOp::DONE);
        self.clear_op(IoOp::BUSY);
    }

    /// Mark request as error
    pub fn mark_error(&self, error: IoReturn) {
        *self.error.lock() = error;
        self.set_op(IoOp::DONE | IoOp::ERROR);
        self.clear_op(IoOp::BUSY);
    }

    /// Get error code
    pub fn get_error(&self) -> IoReturn {
        *self.error.lock()
    }

    /// Lock the request
    pub fn lock(&self) -> bool {
        !self.busy.swap(true, Ordering::SeqCst)
    }

    /// Unlock the request
    pub fn unlock(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }

    /// Get residual (amount not transferred)
    pub fn get_residual(&self) -> u32 {
        self.residual.load(Ordering::SeqCst)
    }

    /// Set residual
    pub fn set_residual(&self, residual: u32) {
        self.residual.store(residual, Ordering::SeqCst);
    }

    /// Set reply port
    pub fn set_reply_port(&mut self, port: PortName, port_type: u32) {
        *self.reply_port.lock() = Some(port);
        self.reply_port_type = port_type;
    }

    /// Get data for read operations
    pub fn get_data(&self) -> Vec<u8> {
        self.data.lock().clone()
    }

    /// Set data for write operations
    pub fn set_data(&self, data: Vec<u8>) {
        *self.data.lock() = data;
    }
}

// ============================================================================
// I/O Request Queue
// ============================================================================

/// Queue of I/O requests
#[derive(Debug)]
pub struct IoQueue {
    /// Pending requests
    pending: VecDeque<Arc<IoRequest>>,
    /// In-progress requests
    in_progress: Vec<Arc<IoRequest>>,
    /// Completed requests
    completed: VecDeque<Arc<IoRequest>>,
}

impl IoQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            in_progress: Vec::new(),
            completed: VecDeque::new(),
        }
    }

    /// Enqueue a request
    pub fn enqueue(&mut self, req: Arc<IoRequest>) {
        self.pending.push_back(req);
    }

    /// Dequeue next pending request
    pub fn dequeue(&mut self) -> Option<Arc<IoRequest>> {
        self.pending.pop_front()
    }

    /// Mark request as in-progress
    pub fn start(&mut self, req: Arc<IoRequest>) {
        req.set_op(IoOp::BUSY);
        self.in_progress.push(req);
    }

    /// Mark request as complete
    pub fn complete(&mut self, id: IoReqId) {
        if let Some(pos) = self.in_progress.iter().position(|r| r.id == id) {
            let req = self.in_progress.remove(pos);
            req.mark_done();
            self.completed.push_back(req);
        }
    }

    /// Get completed request
    pub fn get_completed(&mut self) -> Option<Arc<IoRequest>> {
        self.completed.pop_front()
    }

    /// Number of pending requests
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Number of in-progress requests
    pub fn in_progress_count(&self) -> usize {
        self.in_progress.len()
    }
}

impl Default for IoQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global I/O Request Manager
// ============================================================================

/// I/O Request manager
pub struct IoManager {
    /// All requests
    requests: BTreeMap<IoReqId, Arc<IoRequest>>,
    /// Per-device queues
    device_queues: BTreeMap<DeviceId, IoQueue>,
    /// Next request ID
    next_id: u64,
}

use alloc::collections::BTreeMap;

impl IoManager {
    pub fn new() -> Self {
        Self {
            requests: BTreeMap::new(),
            device_queues: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a new I/O request
    pub fn alloc_request(&mut self, device_id: DeviceId, unit: u32) -> Arc<IoRequest> {
        let id = IoReqId(self.next_id);
        self.next_id += 1;

        let req = Arc::new(IoRequest::new(id, device_id, unit));
        self.requests.insert(id, Arc::clone(&req));
        req
    }

    /// Free an I/O request
    pub fn free_request(&mut self, id: IoReqId) {
        self.requests.remove(&id);
    }

    /// Get device queue
    pub fn get_queue(&mut self, device_id: DeviceId) -> &mut IoQueue {
        self.device_queues.entry(device_id).or_default()
    }

    /// Submit a request to a device
    pub fn submit(&mut self, req: Arc<IoRequest>) {
        let device_id = req.device_id;
        self.get_queue(device_id).enqueue(req);
    }

    /// Get next request for a device
    pub fn get_next(&mut self, device_id: DeviceId) -> Option<Arc<IoRequest>> {
        self.get_queue(device_id).dequeue()
    }

    /// Complete a request
    pub fn complete(&mut self, req: &IoRequest) {
        let device_id = req.device_id;
        self.get_queue(device_id).complete(req.id);
    }
}

impl Default for IoManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static IO_MANAGER: spin::Once<Mutex<IoManager>> = spin::Once::new();

/// Initialize I/O request subsystem
pub fn init() {
    IO_MANAGER.call_once(|| Mutex::new(IoManager::new()));
}

/// Get I/O manager
pub fn io_manager() -> &'static Mutex<IoManager> {
    IO_MANAGER.get().expect("I/O manager not initialized")
}

/// Allocate an I/O request
pub fn io_req_alloc(device_id: DeviceId, unit: u32) -> Arc<IoRequest> {
    io_manager().lock().alloc_request(device_id, unit)
}

/// Free an I/O request
pub fn io_req_free(id: IoReqId) {
    io_manager().lock().free_request(id);
}

/// Standard completion routine
pub fn iodone(req: &IoRequest) {
    req.mark_done();
    io_manager().lock().complete(req);
    // Would send reply message here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_op() {
        let op = IoOp::READ;
        assert!(op.is_read());
        assert!(!op.is_write());

        let op2 = IoOp::WRITE | IoOp::BUSY;
        assert!(!op2.is_read());
    }

    #[test]
    fn test_io_request() {
        let req = IoRequest::read(IoReqId(1), DeviceId(1), 0, 0, 512);
        assert!(!req.is_done());
        req.mark_done();
        assert!(req.is_done());
    }

    #[test]
    fn test_io_queue() {
        let mut queue = IoQueue::new();
        let req = Arc::new(IoRequest::new(IoReqId(1), DeviceId(1), 0));

        queue.enqueue(req);
        assert_eq!(queue.pending_count(), 1);

        let dequeued = queue.dequeue();
        assert!(dequeued.is_some());
        assert_eq!(queue.pending_count(), 0);
    }
}
