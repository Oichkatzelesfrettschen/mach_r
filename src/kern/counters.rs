//! Kernel Performance Counters
//!
//! Based on Mach4 kern/counters.h/c by CMU (1987-1991)
//!
//! This module provides kernel-wide performance counters for tracking
//! various events and paths through the kernel. These are useful for
//! debugging, profiling, and performance analysis.
//!
//! ## Counter Categories
//!
//! - **Thread**: Context switches, invoke hits/misses, handoffs
//! - **Stack**: Current/max/min stack usage
//! - **IPC**: Message queue blocking events
//! - **VM**: Page faults, pageout events
//! - **Scheduler**: Thread switches, blocking
//! - **System**: Clock ticks, AST handling

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Counter Type
// ============================================================================

/// Mach counter type (atomically updated)
#[derive(Debug)]
pub struct MachCounter {
    value: AtomicU64,
    name: &'static str,
}

impl MachCounter {
    /// Create a new counter
    pub const fn new(name: &'static str) -> Self {
        Self {
            value: AtomicU64::new(0),
            name,
        }
    }

    /// Increment counter by 1
    #[inline]
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment counter by n
    #[inline]
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Decrement counter by 1
    #[inline]
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Set counter to specific value
    #[inline]
    pub fn set(&self, v: u64) {
        self.value.store(v, Ordering::Relaxed);
    }

    /// Get current value
    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset to zero
    #[inline]
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }

    /// Get counter name
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Update max if current value is higher
    pub fn update_max(&self, current: u64) {
        loop {
            let max = self.value.load(Ordering::Relaxed);
            if current <= max {
                break;
            }
            if self
                .value
                .compare_exchange_weak(max, current, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Update min if current value is lower (or counter is zero)
    pub fn update_min(&self, current: u64) {
        loop {
            let min = self.value.load(Ordering::Relaxed);
            if min != 0 && current >= min {
                break;
            }
            if self
                .value
                .compare_exchange_weak(min, current, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

// ============================================================================
// Thread Counters (always enabled)
// ============================================================================

/// Thread invoke hits (fast path)
pub static C_THREAD_INVOKE_HITS: MachCounter = MachCounter::new("thread_invoke_hits");
/// Thread invoke misses (slow path)
pub static C_THREAD_INVOKE_MISSES: MachCounter = MachCounter::new("thread_invoke_misses");
/// Thread invoke context switches
pub static C_THREAD_INVOKE_CSW: MachCounter = MachCounter::new("thread_invoke_csw");
/// Thread handoff hits
pub static C_THREAD_HANDOFF_HITS: MachCounter = MachCounter::new("thread_handoff_hits");
/// Thread handoff misses
pub static C_THREAD_HANDOFF_MISSES: MachCounter = MachCounter::new("thread_handoff_misses");

// ============================================================================
// Thread/Stack Tracking Counters
// ============================================================================

/// Current number of threads
pub static C_THREADS_CURRENT: MachCounter = MachCounter::new("threads_current");
/// Maximum threads ever
pub static C_THREADS_MAX: MachCounter = MachCounter::new("threads_max");
/// Minimum threads (after startup)
pub static C_THREADS_MIN: MachCounter = MachCounter::new("threads_min");
/// Total threads created
pub static C_THREADS_TOTAL: MachCounter = MachCounter::new("threads_total");

/// Current number of stacks
pub static C_STACKS_CURRENT: MachCounter = MachCounter::new("stacks_current");
/// Maximum stacks ever
pub static C_STACKS_MAX: MachCounter = MachCounter::new("stacks_max");
/// Minimum stacks (after startup)
pub static C_STACKS_MIN: MachCounter = MachCounter::new("stacks_min");
/// Total stacks allocated
pub static C_STACKS_TOTAL: MachCounter = MachCounter::new("stacks_total");

// ============================================================================
// System Counters
// ============================================================================

/// Clock ticks
pub static C_CLOCK_TICKS: MachCounter = MachCounter::new("clock_ticks");

// ============================================================================
// IPC Counters
// ============================================================================

/// IPC message queue send blocks
pub static C_IPC_MQUEUE_SEND_BLOCK: MachCounter = MachCounter::new("ipc_mqueue_send_block");
/// IPC message queue receive blocks (user)
pub static C_IPC_MQUEUE_RECEIVE_BLOCK_USER: MachCounter =
    MachCounter::new("ipc_mqueue_receive_block_user");
/// IPC message queue receive blocks (kernel)
pub static C_IPC_MQUEUE_RECEIVE_BLOCK_KERNEL: MachCounter =
    MachCounter::new("ipc_mqueue_receive_block_kernel");
/// mach_msg trap blocks (fast path)
pub static C_MACH_MSG_TRAP_BLOCK_FAST: MachCounter = MachCounter::new("mach_msg_trap_block_fast");
/// mach_msg trap blocks (slow path)
pub static C_MACH_MSG_TRAP_BLOCK_SLOW: MachCounter = MachCounter::new("mach_msg_trap_block_slow");
/// mach_msg trap blocks (exception)
pub static C_MACH_MSG_TRAP_BLOCK_EXC: MachCounter = MachCounter::new("mach_msg_trap_block_exc");
/// Exception raise blocks
pub static C_EXCEPTION_RAISE_BLOCK: MachCounter = MachCounter::new("exception_raise_block");

// ============================================================================
// Scheduler Counters
// ============================================================================

/// swtch() blocks
pub static C_SWTCH_BLOCK: MachCounter = MachCounter::new("swtch_block");
/// swtch_pri() blocks
pub static C_SWTCH_PRI_BLOCK: MachCounter = MachCounter::new("swtch_pri_block");
/// thread_switch() blocks
pub static C_THREAD_SWITCH_BLOCK: MachCounter = MachCounter::new("thread_switch_block");
/// thread_switch() handoffs
pub static C_THREAD_SWITCH_HANDOFF: MachCounter = MachCounter::new("thread_switch_handoff");
/// AST taken blocks
pub static C_AST_TAKEN_BLOCK: MachCounter = MachCounter::new("ast_taken_block");
/// Thread halt self blocks
pub static C_THREAD_HALT_SELF_BLOCK: MachCounter = MachCounter::new("thread_halt_self_block");

// ============================================================================
// VM Counters
// ============================================================================

/// VM fault page busy blocks (user)
pub static C_VM_FAULT_PAGE_BLOCK_BUSY_USER: MachCounter =
    MachCounter::new("vm_fault_page_block_busy_user");
/// VM fault page busy blocks (kernel)
pub static C_VM_FAULT_PAGE_BLOCK_BUSY_KERNEL: MachCounter =
    MachCounter::new("vm_fault_page_block_busy_kernel");
/// VM fault page backoff blocks (user)
pub static C_VM_FAULT_PAGE_BLOCK_BACKOFF_USER: MachCounter =
    MachCounter::new("vm_fault_page_block_backoff_user");
/// VM fault page backoff blocks (kernel)
pub static C_VM_FAULT_PAGE_BLOCK_BACKOFF_KERNEL: MachCounter =
    MachCounter::new("vm_fault_page_block_backoff_kernel");
/// VM page wait blocks (user)
pub static C_VM_PAGE_WAIT_BLOCK_USER: MachCounter = MachCounter::new("vm_page_wait_block_user");
/// VM page wait blocks (kernel)
pub static C_VM_PAGE_WAIT_BLOCK_KERNEL: MachCounter = MachCounter::new("vm_page_wait_block_kernel");
/// VM pageout blocks
pub static C_VM_PAGEOUT_BLOCK: MachCounter = MachCounter::new("vm_pageout_block");
/// VM pageout scan blocks
pub static C_VM_PAGEOUT_SCAN_BLOCK: MachCounter = MachCounter::new("vm_pageout_scan_block");

// ============================================================================
// Kernel Thread Counters
// ============================================================================

/// Idle thread blocks
pub static C_IDLE_THREAD_BLOCK: MachCounter = MachCounter::new("idle_thread_block");
/// Idle thread handoffs
pub static C_IDLE_THREAD_HANDOFF: MachCounter = MachCounter::new("idle_thread_handoff");
/// Scheduler thread blocks
pub static C_SCHED_THREAD_BLOCK: MachCounter = MachCounter::new("sched_thread_block");
/// I/O done thread blocks
pub static C_IO_DONE_THREAD_BLOCK: MachCounter = MachCounter::new("io_done_thread_block");
/// Network thread blocks
pub static C_NET_THREAD_BLOCK: MachCounter = MachCounter::new("net_thread_block");
/// Reaper thread blocks
pub static C_REAPER_THREAD_BLOCK: MachCounter = MachCounter::new("reaper_thread_block");
/// Swapin thread blocks
pub static C_SWAPIN_THREAD_BLOCK: MachCounter = MachCounter::new("swapin_thread_block");
/// Action thread blocks
pub static C_ACTION_THREAD_BLOCK: MachCounter = MachCounter::new("action_thread_block");

// ============================================================================
// Counter Macros (compile-time switchable)
// ============================================================================

/// Whether counters are enabled (compile-time feature)
/// Default to true for now - can be disabled via feature flag
pub const MACH_COUNTERS_ENABLED: bool = true;

/// Increment counter (only if counters enabled)
#[macro_export]
macro_rules! counter {
    ($counter:expr) => {
        if $crate::kern::counters::MACH_COUNTERS_ENABLED {
            $counter.inc();
        }
    };
}

/// Increment counter always (regardless of feature flag)
#[macro_export]
macro_rules! counter_always {
    ($counter:expr) => {
        $counter.inc();
    };
}

// ============================================================================
// Counter Groups
// ============================================================================

/// Counter group for categorized access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterGroup {
    Thread,
    Stack,
    System,
    Ipc,
    Scheduler,
    Vm,
    KernelThread,
}

/// Get all counter values as a snapshot
#[derive(Debug, Clone, Default)]
pub struct CounterSnapshot {
    // Thread
    pub thread_invoke_hits: u64,
    pub thread_invoke_misses: u64,
    pub thread_invoke_csw: u64,
    pub thread_handoff_hits: u64,
    pub thread_handoff_misses: u64,
    pub threads_current: u64,
    pub threads_max: u64,
    pub threads_total: u64,

    // Stack
    pub stacks_current: u64,
    pub stacks_max: u64,
    pub stacks_total: u64,

    // System
    pub clock_ticks: u64,

    // IPC
    pub ipc_mqueue_send_block: u64,
    pub ipc_mqueue_receive_block_user: u64,
    pub ipc_mqueue_receive_block_kernel: u64,
    pub mach_msg_trap_block_fast: u64,
    pub mach_msg_trap_block_slow: u64,

    // VM
    pub vm_fault_page_block_busy_user: u64,
    pub vm_fault_page_block_busy_kernel: u64,
    pub vm_pageout_block: u64,
    pub vm_pageout_scan_block: u64,

    // Kernel threads
    pub idle_thread_block: u64,
    pub sched_thread_block: u64,
    pub reaper_thread_block: u64,
}

impl CounterSnapshot {
    /// Take a snapshot of all counters
    pub fn capture() -> Self {
        Self {
            thread_invoke_hits: C_THREAD_INVOKE_HITS.get(),
            thread_invoke_misses: C_THREAD_INVOKE_MISSES.get(),
            thread_invoke_csw: C_THREAD_INVOKE_CSW.get(),
            thread_handoff_hits: C_THREAD_HANDOFF_HITS.get(),
            thread_handoff_misses: C_THREAD_HANDOFF_MISSES.get(),
            threads_current: C_THREADS_CURRENT.get(),
            threads_max: C_THREADS_MAX.get(),
            threads_total: C_THREADS_TOTAL.get(),

            stacks_current: C_STACKS_CURRENT.get(),
            stacks_max: C_STACKS_MAX.get(),
            stacks_total: C_STACKS_TOTAL.get(),

            clock_ticks: C_CLOCK_TICKS.get(),

            ipc_mqueue_send_block: C_IPC_MQUEUE_SEND_BLOCK.get(),
            ipc_mqueue_receive_block_user: C_IPC_MQUEUE_RECEIVE_BLOCK_USER.get(),
            ipc_mqueue_receive_block_kernel: C_IPC_MQUEUE_RECEIVE_BLOCK_KERNEL.get(),
            mach_msg_trap_block_fast: C_MACH_MSG_TRAP_BLOCK_FAST.get(),
            mach_msg_trap_block_slow: C_MACH_MSG_TRAP_BLOCK_SLOW.get(),

            vm_fault_page_block_busy_user: C_VM_FAULT_PAGE_BLOCK_BUSY_USER.get(),
            vm_fault_page_block_busy_kernel: C_VM_FAULT_PAGE_BLOCK_BUSY_KERNEL.get(),
            vm_pageout_block: C_VM_PAGEOUT_BLOCK.get(),
            vm_pageout_scan_block: C_VM_PAGEOUT_SCAN_BLOCK.get(),

            idle_thread_block: C_IDLE_THREAD_BLOCK.get(),
            sched_thread_block: C_SCHED_THREAD_BLOCK.get(),
            reaper_thread_block: C_REAPER_THREAD_BLOCK.get(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Track thread creation
pub fn thread_created() {
    C_THREADS_CURRENT.inc();
    C_THREADS_TOTAL.inc();
    let current = C_THREADS_CURRENT.get();
    C_THREADS_MAX.update_max(current);
}

/// Track thread destruction
pub fn thread_destroyed() {
    C_THREADS_CURRENT.dec();
    let current = C_THREADS_CURRENT.get();
    C_THREADS_MIN.update_min(current);
}

/// Track stack allocation
pub fn stack_allocated() {
    C_STACKS_CURRENT.inc();
    C_STACKS_TOTAL.inc();
    let current = C_STACKS_CURRENT.get();
    C_STACKS_MAX.update_max(current);
}

/// Track stack deallocation
pub fn stack_freed() {
    C_STACKS_CURRENT.dec();
    let current = C_STACKS_CURRENT.get();
    C_STACKS_MIN.update_min(current);
}

/// Record a clock tick
pub fn clock_tick() {
    C_CLOCK_TICKS.inc();
}

/// Track successful thread invoke (fast path)
pub fn thread_invoke_hit() {
    C_THREAD_INVOKE_HITS.inc();
}

/// Track failed thread invoke (slow path)
pub fn thread_invoke_miss() {
    C_THREAD_INVOKE_MISSES.inc();
}

/// Track context switch
pub fn context_switch() {
    C_THREAD_INVOKE_CSW.inc();
}

/// Reset all counters to zero
pub fn reset_all_counters() {
    C_THREAD_INVOKE_HITS.reset();
    C_THREAD_INVOKE_MISSES.reset();
    C_THREAD_INVOKE_CSW.reset();
    C_THREAD_HANDOFF_HITS.reset();
    C_THREAD_HANDOFF_MISSES.reset();
    C_THREADS_TOTAL.reset();
    C_STACKS_TOTAL.reset();
    C_CLOCK_TICKS.reset();
    C_IPC_MQUEUE_SEND_BLOCK.reset();
    C_IPC_MQUEUE_RECEIVE_BLOCK_USER.reset();
    C_IPC_MQUEUE_RECEIVE_BLOCK_KERNEL.reset();
    C_MACH_MSG_TRAP_BLOCK_FAST.reset();
    C_MACH_MSG_TRAP_BLOCK_SLOW.reset();
    C_MACH_MSG_TRAP_BLOCK_EXC.reset();
    C_EXCEPTION_RAISE_BLOCK.reset();
    C_SWTCH_BLOCK.reset();
    C_SWTCH_PRI_BLOCK.reset();
    C_THREAD_SWITCH_BLOCK.reset();
    C_THREAD_SWITCH_HANDOFF.reset();
    C_AST_TAKEN_BLOCK.reset();
    C_THREAD_HALT_SELF_BLOCK.reset();
    C_VM_FAULT_PAGE_BLOCK_BUSY_USER.reset();
    C_VM_FAULT_PAGE_BLOCK_BUSY_KERNEL.reset();
    C_VM_FAULT_PAGE_BLOCK_BACKOFF_USER.reset();
    C_VM_FAULT_PAGE_BLOCK_BACKOFF_KERNEL.reset();
    C_VM_PAGE_WAIT_BLOCK_USER.reset();
    C_VM_PAGE_WAIT_BLOCK_KERNEL.reset();
    C_VM_PAGEOUT_BLOCK.reset();
    C_VM_PAGEOUT_SCAN_BLOCK.reset();
    C_IDLE_THREAD_BLOCK.reset();
    C_IDLE_THREAD_HANDOFF.reset();
    C_SCHED_THREAD_BLOCK.reset();
    C_IO_DONE_THREAD_BLOCK.reset();
    C_NET_THREAD_BLOCK.reset();
    C_REAPER_THREAD_BLOCK.reset();
    C_SWAPIN_THREAD_BLOCK.reset();
    C_ACTION_THREAD_BLOCK.reset();
}

/// Initialize the counters subsystem
pub fn init() {
    // Counters are statically initialized, nothing to do
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_basic() {
        let counter = MachCounter::new("test");
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.add(5);
        assert_eq!(counter.get(), 6);

        counter.dec();
        assert_eq!(counter.get(), 5);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_counter_max() {
        let counter = MachCounter::new("max_test");
        counter.update_max(10);
        assert_eq!(counter.get(), 10);

        counter.update_max(5);
        assert_eq!(counter.get(), 10); // Should stay at 10

        counter.update_max(20);
        assert_eq!(counter.get(), 20);
    }

    #[test]
    fn test_counter_min() {
        let counter = MachCounter::new("min_test");
        counter.update_min(10);
        assert_eq!(counter.get(), 10); // First value accepted

        counter.update_min(15);
        assert_eq!(counter.get(), 10); // Should stay at 10

        counter.update_min(5);
        assert_eq!(counter.get(), 5);
    }

    #[test]
    fn test_thread_tracking() {
        let initial = C_THREADS_TOTAL.get();

        thread_created();
        assert_eq!(C_THREADS_TOTAL.get(), initial + 1);

        thread_created();
        assert_eq!(C_THREADS_TOTAL.get(), initial + 2);

        thread_destroyed();
        // Total doesn't decrease, only current
    }

    #[test]
    fn test_snapshot() {
        let snapshot = CounterSnapshot::capture();
        // Just verify it doesn't panic and has reasonable values
        assert!(snapshot.clock_ticks >= 0);
    }
}
