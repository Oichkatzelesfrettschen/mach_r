//! Exception Handling
//!
//! Based on Mach4 kern/exception.c
//!
//! When a thread catches an exception, Mach sends a message to the thread's
//! exception port. If that fails, it tries the task's exception port.
//! If that also fails, the thread is terminated.
//!
//! Exception types include:
//! - Bad access (memory protection violation)
//! - Bad instruction (illegal instruction)
//! - Arithmetic (divide by zero, overflow)
//! - Breakpoint (debugger trap)
//! - Software exceptions

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::kern::thread::{TaskId, ThreadId};

// ============================================================================
// Exception Types
// ============================================================================

/// Exception type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum ExceptionType {
    /// Invalid (no exception)
    #[default]
    None = 0,
    /// Bad access (memory protection violation)
    BadAccess = 1,
    /// Bad instruction (illegal/privileged instruction)
    BadInstruction = 2,
    /// Arithmetic exception (divide by zero, overflow)
    Arithmetic = 3,
    /// Emulation (instruction emulation support)
    Emulation = 4,
    /// Software exception (user-generated)
    Software = 5,
    /// Breakpoint (debugger trap)
    Breakpoint = 6,
    /// System call exception
    Syscall = 7,
    /// Mach system call exception
    MachSyscall = 8,
    /// RPC alert
    RpcAlert = 9,
}

impl ExceptionType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::None),
            1 => Some(Self::BadAccess),
            2 => Some(Self::BadInstruction),
            3 => Some(Self::Arithmetic),
            4 => Some(Self::Emulation),
            5 => Some(Self::Software),
            6 => Some(Self::Breakpoint),
            7 => Some(Self::Syscall),
            8 => Some(Self::MachSyscall),
            9 => Some(Self::RpcAlert),
            _ => None,
        }
    }

    /// Get exception name for debugging
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BadAccess => "bad_access",
            Self::BadInstruction => "bad_instruction",
            Self::Arithmetic => "arithmetic",
            Self::Emulation => "emulation",
            Self::Software => "software",
            Self::Breakpoint => "breakpoint",
            Self::Syscall => "syscall",
            Self::MachSyscall => "mach_syscall",
            Self::RpcAlert => "rpc_alert",
        }
    }
}

// ============================================================================
// Exception Behavior
// ============================================================================

/// Exception behavior flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExceptionBehavior(pub u32);

impl ExceptionBehavior {
    /// Default behavior (send exception message)
    pub const DEFAULT: Self = Self(1);
    /// State behavior (include thread state)
    pub const STATE: Self = Self(2);
    /// State identity behavior (include thread state and identity)
    pub const STATE_IDENTITY: Self = Self(3);

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn includes_state(self) -> bool {
        self.0 >= 2
    }

    pub fn includes_identity(self) -> bool {
        self.0 >= 3
    }
}

// ============================================================================
// Exception Mask
// ============================================================================

/// Exception mask (bitfield of exception types)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExceptionMask(pub u32);

impl ExceptionMask {
    pub const NONE: Self = Self(0);
    pub const BAD_ACCESS: Self = Self(1 << 1);
    pub const BAD_INSTRUCTION: Self = Self(1 << 2);
    pub const ARITHMETIC: Self = Self(1 << 3);
    pub const EMULATION: Self = Self(1 << 4);
    pub const SOFTWARE: Self = Self(1 << 5);
    pub const BREAKPOINT: Self = Self(1 << 6);
    pub const SYSCALL: Self = Self(1 << 7);
    pub const MACH_SYSCALL: Self = Self(1 << 8);
    pub const RPC_ALERT: Self = Self(1 << 9);
    pub const ALL: Self = Self(0x3FE);

    pub fn contains(self, exc: ExceptionType) -> bool {
        (self.0 & (1 << (exc as u32))) != 0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn from_exception(exc: ExceptionType) -> Self {
        Self(1 << (exc as u32))
    }
}

impl core::ops::BitOr for ExceptionMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// ============================================================================
// Exception Port Info
// ============================================================================

/// Information about an exception port
#[derive(Debug, Clone)]
pub struct ExceptionPortInfo {
    /// Port name
    pub port: PortName,
    /// Exception mask this port handles
    pub mask: ExceptionMask,
    /// Behavior when exception occurs
    pub behavior: ExceptionBehavior,
    /// Thread state flavor to send
    pub flavor: u32,
}

impl ExceptionPortInfo {
    pub fn new(port: PortName, mask: ExceptionMask) -> Self {
        Self {
            port,
            mask,
            behavior: ExceptionBehavior::DEFAULT,
            flavor: 0,
        }
    }

    pub fn with_behavior(mut self, behavior: ExceptionBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn with_flavor(mut self, flavor: u32) -> Self {
        self.flavor = flavor;
        self
    }
}

// ============================================================================
// Exception State
// ============================================================================

/// Saved exception state for retry
#[derive(Debug, Clone, Default)]
pub struct ExceptionState {
    /// Exception type
    pub exception: ExceptionType,
    /// Exception code
    pub code: u64,
    /// Exception subcode
    pub subcode: u64,
}

impl ExceptionState {
    pub fn new(exception: ExceptionType, code: u64, subcode: u64) -> Self {
        Self {
            exception,
            code,
            subcode,
        }
    }

    pub fn clear(&mut self) {
        self.exception = ExceptionType::None;
        self.code = 0;
        self.subcode = 0;
    }

    pub fn is_valid(&self) -> bool {
        self.exception != ExceptionType::None
    }
}

// ============================================================================
// Exception Handler Result
// ============================================================================

/// Result of exception handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionResult {
    /// Exception handled successfully
    Success,
    /// Exception port not valid
    NoPort,
    /// Exception message send failed
    SendFailed,
    /// Exception reply indicated failure
    ReplyFailed,
    /// Thread should be terminated
    Terminate,
    /// No exception handler available
    NoHandler,
}

// ============================================================================
// Exception Handler
// ============================================================================

/// Exception handler entry
#[derive(Debug, Clone)]
struct ExceptionHandler {
    /// Exception mask
    mask: ExceptionMask,
    /// Handler port
    port: PortName,
    /// Behavior
    behavior: ExceptionBehavior,
    /// Thread state flavor
    flavor: u32,
}

/// Exception handlers for a thread or task
#[derive(Debug, Default)]
pub struct ExceptionHandlers {
    /// Handlers (searched in order)
    handlers: Vec<ExceptionHandler>,
}

impl ExceptionHandlers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set exception port for mask
    pub fn set_port(
        &mut self,
        mask: ExceptionMask,
        port: PortName,
        behavior: ExceptionBehavior,
        flavor: u32,
    ) {
        // Remove any existing handler for this mask
        self.handlers.retain(|h| (h.mask.0 & mask.0) == 0);

        // Add new handler
        self.handlers.push(ExceptionHandler {
            mask,
            port,
            behavior,
            flavor,
        });
    }

    /// Get port for exception type
    pub fn get_port(&self, exc: ExceptionType) -> Option<ExceptionPortInfo> {
        for handler in &self.handlers {
            if handler.mask.contains(exc) {
                return Some(ExceptionPortInfo {
                    port: handler.port,
                    mask: handler.mask,
                    behavior: handler.behavior,
                    flavor: handler.flavor,
                });
            }
        }
        None
    }

    /// Get all exception ports
    pub fn get_all_ports(&self) -> Vec<ExceptionPortInfo> {
        self.handlers
            .iter()
            .map(|h| ExceptionPortInfo {
                port: h.port,
                mask: h.mask,
                behavior: h.behavior,
                flavor: h.flavor,
            })
            .collect()
    }

    /// Clear all handlers
    pub fn clear(&mut self) {
        self.handlers.clear();
    }
}

// ============================================================================
// Exception Context
// ============================================================================

/// Exception context for delivery
#[derive(Debug)]
pub struct ExceptionContext {
    /// Thread that raised exception
    pub thread_id: ThreadId,
    /// Task containing thread
    pub task_id: TaskId,
    /// Exception type
    pub exception: ExceptionType,
    /// Exception code
    pub code: u64,
    /// Exception subcode
    pub subcode: u64,
    /// Exception port to use
    pub port: Option<PortName>,
    /// Behavior
    pub behavior: ExceptionBehavior,
    /// Thread state flavor
    pub flavor: u32,
}

impl ExceptionContext {
    pub fn new(
        thread_id: ThreadId,
        task_id: TaskId,
        exception: ExceptionType,
        code: u64,
        subcode: u64,
    ) -> Self {
        Self {
            thread_id,
            task_id,
            exception,
            code,
            subcode,
            port: None,
            behavior: ExceptionBehavior::DEFAULT,
            flavor: 0,
        }
    }

    pub fn with_port(mut self, port: PortName, behavior: ExceptionBehavior, flavor: u32) -> Self {
        self.port = Some(port);
        self.behavior = behavior;
        self.flavor = flavor;
        self
    }
}

// ============================================================================
// Exception Manager
// ============================================================================

/// Global exception manager
pub struct ExceptionManager {
    /// Thread exception handlers
    thread_handlers: BTreeMap<ThreadId, ExceptionHandlers>,

    /// Task exception handlers
    task_handlers: BTreeMap<TaskId, ExceptionHandlers>,

    /// Host exception handlers (last resort)
    host_handlers: ExceptionHandlers,

    /// Statistics
    pub stats: ExceptionStats,
}

/// Exception statistics
#[derive(Debug, Default)]
pub struct ExceptionStats {
    /// Total exceptions raised
    pub raised: AtomicU32,
    /// Exceptions handled by thread port
    pub thread_handled: AtomicU32,
    /// Exceptions handled by task port
    pub task_handled: AtomicU32,
    /// Exceptions handled by host port
    pub host_handled: AtomicU32,
    /// Exceptions with no handler (terminated)
    pub no_handler: AtomicU32,
}

impl ExceptionManager {
    pub fn new() -> Self {
        Self {
            thread_handlers: BTreeMap::new(),
            task_handlers: BTreeMap::new(),
            host_handlers: ExceptionHandlers::new(),
            stats: ExceptionStats::default(),
        }
    }

    /// Set thread exception port
    pub fn set_thread_port(
        &mut self,
        thread_id: ThreadId,
        mask: ExceptionMask,
        port: PortName,
        behavior: ExceptionBehavior,
        flavor: u32,
    ) {
        self.thread_handlers
            .entry(thread_id)
            .or_default()
            .set_port(mask, port, behavior, flavor);
    }

    /// Set task exception port
    pub fn set_task_port(
        &mut self,
        task_id: TaskId,
        mask: ExceptionMask,
        port: PortName,
        behavior: ExceptionBehavior,
        flavor: u32,
    ) {
        self.task_handlers
            .entry(task_id)
            .or_default()
            .set_port(mask, port, behavior, flavor);
    }

    /// Set host exception port
    pub fn set_host_port(
        &mut self,
        mask: ExceptionMask,
        port: PortName,
        behavior: ExceptionBehavior,
        flavor: u32,
    ) {
        self.host_handlers.set_port(mask, port, behavior, flavor);
    }

    /// Get thread exception ports
    pub fn get_thread_ports(&self, thread_id: ThreadId) -> Vec<ExceptionPortInfo> {
        self.thread_handlers
            .get(&thread_id)
            .map(|h| h.get_all_ports())
            .unwrap_or_default()
    }

    /// Get task exception ports
    pub fn get_task_ports(&self, task_id: TaskId) -> Vec<ExceptionPortInfo> {
        self.task_handlers
            .get(&task_id)
            .map(|h| h.get_all_ports())
            .unwrap_or_default()
    }

    /// Find exception port for an exception
    ///
    /// Searches in order: thread ports, task ports, host ports
    pub fn find_port(
        &self,
        thread_id: ThreadId,
        task_id: TaskId,
        exc: ExceptionType,
    ) -> Option<(ExceptionPortInfo, ExceptionPortLevel)> {
        // Try thread ports first
        if let Some(handlers) = self.thread_handlers.get(&thread_id) {
            if let Some(info) = handlers.get_port(exc) {
                return Some((info, ExceptionPortLevel::Thread));
            }
        }

        // Try task ports
        if let Some(handlers) = self.task_handlers.get(&task_id) {
            if let Some(info) = handlers.get_port(exc) {
                return Some((info, ExceptionPortLevel::Task));
            }
        }

        // Try host ports
        if let Some(info) = self.host_handlers.get_port(exc) {
            return Some((info, ExceptionPortLevel::Host));
        }

        None
    }

    /// Raise an exception
    ///
    /// This is the main entry point for exception handling.
    pub fn raise(
        &self,
        thread_id: ThreadId,
        task_id: TaskId,
        exception: ExceptionType,
        _code: u64,
        _subcode: u64,
    ) -> ExceptionResult {
        self.stats.raised.fetch_add(1, Ordering::Relaxed);

        if exception == ExceptionType::None {
            return ExceptionResult::Success;
        }

        // Find handler
        let (_port_info, level) = match self.find_port(thread_id, task_id, exception) {
            Some(p) => p,
            None => {
                self.stats.no_handler.fetch_add(1, Ordering::Relaxed);
                return ExceptionResult::NoHandler;
            }
        };

        // Record which level handled it
        match level {
            ExceptionPortLevel::Thread => {
                self.stats.thread_handled.fetch_add(1, Ordering::Relaxed);
            }
            ExceptionPortLevel::Task => {
                self.stats.task_handled.fetch_add(1, Ordering::Relaxed);
            }
            ExceptionPortLevel::Host => {
                self.stats.host_handled.fetch_add(1, Ordering::Relaxed);
            }
        }

        // In a real implementation, we would:
        // 1. Suspend the thread
        // 2. Send an exception message to the port
        // 3. Wait for reply
        // 4. Based on reply, resume or terminate

        // For now, return success indicating handler was found
        ExceptionResult::Success
    }

    /// Remove all exception handlers for a thread
    pub fn thread_terminate(&mut self, thread_id: ThreadId) {
        self.thread_handlers.remove(&thread_id);
    }

    /// Remove all exception handlers for a task
    pub fn task_terminate(&mut self, task_id: TaskId) {
        self.task_handlers.remove(&task_id);
    }
}

impl Default for ExceptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Level at which exception port was found
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionPortLevel {
    Thread,
    Task,
    Host,
}

// ============================================================================
// Global State
// ============================================================================

static EXCEPTION_MANAGER: spin::Once<Mutex<ExceptionManager>> = spin::Once::new();

fn exception_manager() -> &'static Mutex<ExceptionManager> {
    EXCEPTION_MANAGER.call_once(|| Mutex::new(ExceptionManager::new()));
    EXCEPTION_MANAGER.get().unwrap()
}

/// Initialize exception subsystem
pub fn init() {
    let _ = exception_manager();
}

/// Raise an exception for current thread
pub fn exception_raise(
    thread_id: ThreadId,
    task_id: TaskId,
    exception: ExceptionType,
    code: u64,
    subcode: u64,
) -> ExceptionResult {
    exception_manager()
        .lock()
        .raise(thread_id, task_id, exception, code, subcode)
}

/// Set thread exception port
pub fn thread_set_exception_port(
    thread_id: ThreadId,
    mask: ExceptionMask,
    port: PortName,
    behavior: ExceptionBehavior,
    flavor: u32,
) {
    exception_manager()
        .lock()
        .set_thread_port(thread_id, mask, port, behavior, flavor);
}

/// Set task exception port
pub fn task_set_exception_port(
    task_id: TaskId,
    mask: ExceptionMask,
    port: PortName,
    behavior: ExceptionBehavior,
    flavor: u32,
) {
    exception_manager()
        .lock()
        .set_task_port(task_id, mask, port, behavior, flavor);
}

/// Get thread exception ports
pub fn thread_get_exception_ports(thread_id: ThreadId) -> Vec<ExceptionPortInfo> {
    exception_manager().lock().get_thread_ports(thread_id)
}

/// Get task exception ports
pub fn task_get_exception_ports(task_id: TaskId) -> Vec<ExceptionPortInfo> {
    exception_manager().lock().get_task_ports(task_id)
}

/// Notify exception system that thread is terminating
pub fn exception_thread_terminate(thread_id: ThreadId) {
    exception_manager().lock().thread_terminate(thread_id);
}

/// Notify exception system that task is terminating
pub fn exception_task_terminate(task_id: TaskId) {
    exception_manager().lock().task_terminate(task_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_type() {
        assert_eq!(ExceptionType::BadAccess.name(), "bad_access");
        assert_eq!(ExceptionType::from_u32(1), Some(ExceptionType::BadAccess));
        assert_eq!(ExceptionType::from_u32(100), None);
    }

    #[test]
    fn test_exception_mask() {
        let mask = ExceptionMask::BAD_ACCESS | ExceptionMask::BREAKPOINT;
        assert!(mask.contains(ExceptionType::BadAccess));
        assert!(mask.contains(ExceptionType::Breakpoint));
        assert!(!mask.contains(ExceptionType::Arithmetic));
    }

    #[test]
    fn test_exception_handlers() {
        let mut handlers = ExceptionHandlers::new();

        handlers.set_port(
            ExceptionMask::ALL,
            PortName(100),
            ExceptionBehavior::DEFAULT,
            0,
        );

        let info = handlers.get_port(ExceptionType::BadAccess).unwrap();
        assert_eq!(info.port, PortName(100));
    }

    #[test]
    fn test_exception_manager() {
        let mut mgr = ExceptionManager::new();

        mgr.set_task_port(
            TaskId(1),
            ExceptionMask::ALL,
            PortName(200),
            ExceptionBehavior::STATE,
            1,
        );

        let (info, level) = mgr
            .find_port(ThreadId(1), TaskId(1), ExceptionType::BadAccess)
            .unwrap();

        assert_eq!(info.port, PortName(200));
        assert_eq!(level, ExceptionPortLevel::Task);
    }
}
