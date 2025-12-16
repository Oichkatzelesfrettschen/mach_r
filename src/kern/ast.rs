//! AST - Asynchronous System Traps
//!
//! Based on Mach4 kern/ast.h/c
//! ASTs are pending work that a thread should handle at the next
//! opportunity (e.g., returning from kernel to user mode).

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::types::ThreadId;

// ============================================================================
// AST Reasons (bit flags)
// ============================================================================

/// AST reason flags (from Mach4)
pub mod ast_flags {
    /// No ASTs pending
    pub const AST_NONE: u32 = 0x00;
    /// Need to halt the thread
    pub const AST_HALT: u32 = 0x01;
    /// Need to terminate the thread
    pub const AST_TERMINATE: u32 = 0x02;
    /// Need to block the thread
    pub const AST_BLOCK: u32 = 0x04;
    /// Network interrupt pending
    pub const AST_NETWORK: u32 = 0x08;
    /// Need rescheduling
    pub const AST_QUANTUM: u32 = 0x10;
    /// Scheduler wants preemption
    pub const AST_PREEMPT: u32 = 0x20;

    /// Machine-dependent AST reasons start here
    pub const AST_MACHINE_SHIFT: u32 = 8;

    /// All scheduler-related ASTs
    pub const AST_SCHED_MASK: u32 = AST_QUANTUM | AST_PREEMPT;

    /// All termination-related ASTs
    pub const AST_TERM_MASK: u32 = AST_HALT | AST_TERMINATE;
}

pub use ast_flags::*;

// ============================================================================
// AST State
// ============================================================================

/// AST state for a thread
#[derive(Debug, Default)]
pub struct AstState {
    /// Pending AST reasons (bit flags)
    pending: AtomicU32,
}

impl AstState {
    /// Create new AST state
    pub const fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
        }
    }

    /// Check if any ASTs are pending
    pub fn check(&self) -> u32 {
        self.pending.load(Ordering::SeqCst)
    }

    /// Set AST reason(s)
    pub fn set(&self, reasons: u32) {
        self.pending.fetch_or(reasons, Ordering::SeqCst);
    }

    /// Clear AST reason(s)
    pub fn clear(&self, reasons: u32) {
        self.pending.fetch_and(!reasons, Ordering::SeqCst);
    }

    /// Clear all ASTs
    pub fn clear_all(&self) {
        self.pending.store(0, Ordering::SeqCst);
    }

    /// Check if thread should halt
    pub fn should_halt(&self) -> bool {
        (self.check() & AST_TERM_MASK) != 0
    }

    /// Check if rescheduling needed
    pub fn need_resched(&self) -> bool {
        (self.check() & AST_SCHED_MASK) != 0
    }
}

// ============================================================================
// Per-Thread AST Management
// ============================================================================

/// Global AST state per thread
static THREAD_AST: spin::Once<Mutex<BTreeMap<ThreadId, AstState>>> = spin::Once::new();

/// Initialize thread AST map
fn init_thread_ast() {
    THREAD_AST.call_once(|| Mutex::new(BTreeMap::new()));
}

fn thread_ast_map() -> &'static Mutex<BTreeMap<ThreadId, AstState>> {
    init_thread_ast();
    THREAD_AST.get().expect("Thread AST map not initialized")
}

/// Initialize AST for a thread
pub fn ast_init(thread_id: ThreadId) {
    thread_ast_map().lock().insert(thread_id, AstState::new());
}

/// Clean up AST for a thread
pub fn ast_cleanup(thread_id: ThreadId) {
    thread_ast_map().lock().remove(&thread_id);
}

/// Check ASTs for a thread
pub fn ast_check(thread_id: ThreadId) -> u32 {
    thread_ast_map()
        .lock()
        .get(&thread_id)
        .map(|s| s.check())
        .unwrap_or(0)
}

/// Set AST for a thread
pub fn ast_on(thread_id: ThreadId, reasons: u32) {
    let map = thread_ast_map();
    let mut guard = map.lock();
    if let Some(state) = guard.get(&thread_id) {
        state.set(reasons);
    } else {
        let state = AstState::new();
        state.set(reasons);
        guard.insert(thread_id, state);
    }
}

/// Clear AST for a thread
pub fn ast_off(thread_id: ThreadId, reasons: u32) {
    if let Some(state) = thread_ast_map().lock().get(&thread_id) {
        state.clear(reasons);
    }
}

/// Check if thread should halt
pub fn thread_should_halt(thread_id: ThreadId) -> bool {
    thread_ast_map()
        .lock()
        .get(&thread_id)
        .map(|s| s.should_halt())
        .unwrap_or(false)
}

// ============================================================================
// AST Processing
// ============================================================================

/// AST handler result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstResult {
    /// Continue normal execution
    Continue,
    /// Thread should be terminated
    Terminate,
    /// Thread should be halted
    Halt,
    /// Need to reschedule
    Reschedule,
}

/// Process pending ASTs for current thread
/// Called at kernel exit points (returning to user mode)
pub fn ast_taken(thread_id: ThreadId) -> AstResult {
    let reasons = ast_check(thread_id);

    if reasons == AST_NONE {
        return AstResult::Continue;
    }

    // Handle termination first
    if (reasons & AST_TERMINATE) != 0 {
        ast_off(thread_id, AST_TERMINATE);
        return AstResult::Terminate;
    }

    // Handle halt
    if (reasons & AST_HALT) != 0 {
        ast_off(thread_id, AST_HALT);
        return AstResult::Halt;
    }

    // Handle rescheduling
    if (reasons & AST_SCHED_MASK) != 0 {
        ast_off(thread_id, AST_SCHED_MASK);
        return AstResult::Reschedule;
    }

    // Handle block
    if (reasons & AST_BLOCK) != 0 {
        ast_off(thread_id, AST_BLOCK);
        // Thread should block itself
        crate::scheduler::block_current();
    }

    AstResult::Continue
}

/// Request AST check on another CPU (for SMP)
/// In a real implementation, this would send an IPI
pub fn ast_check_cpu(_cpu: usize) {
    // Would send inter-processor interrupt
}

/// Request preemption of a thread
pub fn ast_preempt(thread_id: ThreadId) {
    ast_on(thread_id, AST_PREEMPT);
}

/// Request quantum expiration handling
pub fn ast_quantum(thread_id: ThreadId) {
    ast_on(thread_id, AST_QUANTUM);
}

/// Request thread termination
pub fn ast_terminate(thread_id: ThreadId) {
    ast_on(thread_id, AST_TERMINATE);
}

/// Request thread halt
pub fn ast_halt(thread_id: ThreadId) {
    ast_on(thread_id, AST_HALT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_state() {
        let state = AstState::new();
        assert_eq!(state.check(), AST_NONE);

        state.set(AST_QUANTUM);
        assert!(state.need_resched());
        assert!(!state.should_halt());

        state.set(AST_TERMINATE);
        assert!(state.should_halt());

        state.clear(AST_QUANTUM);
        assert!(!state.need_resched());
        assert!(state.should_halt());

        state.clear_all();
        assert_eq!(state.check(), AST_NONE);
    }

    #[test]
    fn test_ast_flags() {
        assert_eq!(AST_NONE, 0);
        assert_eq!(AST_HALT | AST_TERMINATE, AST_TERM_MASK);
        assert_eq!(AST_QUANTUM | AST_PREEMPT, AST_SCHED_MASK);
    }
}
