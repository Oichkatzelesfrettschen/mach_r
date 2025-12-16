//! Processor and Processor Set Management
//!
//! Based on Mach4 kern/processor.h/c
//! Manages physical processors and processor sets for scheduling.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::types::{TaskId, ThreadId};

// Import scheduler types for idle thread creation
use crate::scheduler::{SchedThread, TIME_QUANTUM_MS};
use crate::task::{Context, ThreadState};
use alloc::sync::Arc as AllocArc;

// ============================================================================
// Processor State
// ============================================================================

/// Processor states (from Mach4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ProcessorState {
    /// Not in system
    OffLine = 0,
    /// Running normally
    Running = 1,
    /// Idle - waiting for work
    Idle = 2,
    /// Dispatching (idle -> running transition)
    Dispatching = 3,
    /// Assignment is changing
    Assign = 4,
    /// Being shutdown
    Shutdown = 5,
}

// ============================================================================
// Run Queue
// ============================================================================

/// Number of run queue priority levels (Mach4 uses 32, reduced in tests)
#[cfg(not(test))]
pub const NRQS: usize = 32;
#[cfg(test)]
pub const NRQS: usize = 4;

/// Run queue for a processor or processor set
#[derive(Debug)]
pub struct RunQueue {
    /// One queue per priority level
    queues: [VecDeque<ThreadId>; NRQS],
    /// Lowest priority queue with threads
    low: AtomicUsize,
    /// Count of runnable threads
    count: AtomicUsize,
}

impl RunQueue {
    /// Create a new run queue
    pub const fn new() -> Self {
        Self {
            queues: [const { VecDeque::new() }; NRQS],
            low: AtomicUsize::new(NRQS),
            count: AtomicUsize::new(0),
        }
    }

    /// Enqueue a thread at the given priority
    pub fn enqueue(&mut self, thread_id: ThreadId, priority: usize) {
        let pri = priority.min(NRQS - 1);
        self.queues[pri].push_back(thread_id);
        self.count.fetch_add(1, Ordering::SeqCst);

        // Update low watermark
        let current_low = self.low.load(Ordering::SeqCst);
        if pri < current_low {
            self.low.store(pri, Ordering::SeqCst);
        }
    }

    /// Dequeue highest priority thread
    pub fn dequeue(&mut self) -> Option<ThreadId> {
        // Search from highest priority (NRQS-1) to lowest (0)
        for pri in (0..NRQS).rev() {
            if let Some(thread_id) = self.queues[pri].pop_front() {
                self.count.fetch_sub(1, Ordering::SeqCst);
                self.update_low();
                return Some(thread_id);
            }
        }
        None
    }

    /// Remove a specific thread from the queue
    pub fn remove(&mut self, thread_id: ThreadId) -> bool {
        for pri in 0..NRQS {
            if let Some(pos) = self.queues[pri].iter().position(|&t| t == thread_id) {
                self.queues[pri].remove(pos);
                self.count.fetch_sub(1, Ordering::SeqCst);
                self.update_low();
                return true;
            }
        }
        false
    }

    /// Update low watermark after removal
    fn update_low(&mut self) {
        for pri in 0..NRQS {
            if !self.queues[pri].is_empty() {
                self.low.store(pri, Ordering::SeqCst);
                return;
            }
        }
        self.low.store(NRQS, Ordering::SeqCst);
    }

    /// Get count of runnable threads
    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    /// Get lowest priority with threads
    pub fn low(&self) -> usize {
        self.low.load(Ordering::SeqCst)
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Processor Set
// ============================================================================

/// Processor set ID
pub type ProcessorSetId = u32;

/// Next processor set ID
static NEXT_PSET_ID: AtomicU32 = AtomicU32::new(1);

fn alloc_pset_id() -> ProcessorSetId {
    NEXT_PSET_ID.fetch_add(1, Ordering::SeqCst)
}

/// Processor set - groups processors for scheduling
///
/// From Mach4 processor.h:
/// - runq: run queue for this set
/// - idle_queue: idle processors
/// - processors: all processors in set
/// - tasks: tasks assigned to this set
/// - threads: threads in this set
#[derive(Debug)]
pub struct ProcessorSet {
    /// Processor set ID
    id: ProcessorSetId,

    /// Run queue for this set
    pub runq: Mutex<RunQueue>,

    /// Idle processors in this set
    idle_queue: Mutex<Vec<ProcessorId>>,

    /// Count of idle processors
    idle_count: AtomicUsize,

    /// All processors in this set
    processors: Mutex<Vec<ProcessorId>>,

    /// Processor count
    processor_count: AtomicUsize,

    /// Is set empty (no processors)?
    empty: AtomicBool,

    /// Tasks assigned to this set
    tasks: Mutex<Vec<TaskId>>,

    /// Task count
    task_count: AtomicUsize,

    /// Threads in this set
    threads: Mutex<Vec<ThreadId>>,

    /// Thread count
    thread_count: AtomicUsize,

    /// Reference count
    ref_count: AtomicU32,

    /// Is this set active?
    active: AtomicBool,

    /// Port for operations
    pset_self: PortName,

    /// Port for information
    pset_name_self: PortName,

    /// Maximum priority for threads in this set
    max_priority: AtomicUsize,

    /// Scheduling policies enabled (bit vector)
    policies: AtomicU32,

    /// Current default quantum
    set_quantum: AtomicU32,

    /// Mach factor (load metric)
    mach_factor: AtomicU32,

    /// Load average
    load_average: AtomicU32,

    /// Scheduler load (for priority calculations)
    sched_load: AtomicU32,
}

impl ProcessorSet {
    /// Default quantum in ticks
    pub const DEFAULT_QUANTUM: u32 = 10;

    /// Create a new processor set
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            id: alloc_pset_id(),
            runq: Mutex::new(RunQueue::new()),
            idle_queue: Mutex::new(Vec::new()),
            idle_count: AtomicUsize::new(0),
            processors: Mutex::new(Vec::new()),
            processor_count: AtomicUsize::new(0),
            empty: AtomicBool::new(true),
            tasks: Mutex::new(Vec::new()),
            task_count: AtomicUsize::new(0),
            threads: Mutex::new(Vec::new()),
            thread_count: AtomicUsize::new(0),
            ref_count: AtomicU32::new(1),
            active: AtomicBool::new(true),
            pset_self: PortName::NULL,
            pset_name_self: PortName::NULL,
            max_priority: AtomicUsize::new(NRQS - 1),
            policies: AtomicU32::new(0x1), // POLICY_TIMESHARE enabled
            set_quantum: AtomicU32::new(Self::DEFAULT_QUANTUM),
            mach_factor: AtomicU32::new(0),
            load_average: AtomicU32::new(0),
            sched_load: AtomicU32::new(0),
        })
    }

    /// Get processor set ID
    pub fn id(&self) -> ProcessorSetId {
        self.id
    }

    /// Check if active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Add a reference
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Release a reference
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    // ========================================================================
    // Processor Management
    // ========================================================================

    /// Add a processor to this set
    pub fn add_processor(&self, processor_id: ProcessorId) {
        let mut processors = self.processors.lock();
        if !processors.contains(&processor_id) {
            processors.push(processor_id);
            self.processor_count.fetch_add(1, Ordering::SeqCst);
            self.empty.store(false, Ordering::SeqCst);
        }
    }

    /// Remove a processor from this set
    pub fn remove_processor(&self, processor_id: ProcessorId) {
        let mut processors = self.processors.lock();
        if let Some(pos) = processors.iter().position(|&p| p == processor_id) {
            processors.remove(pos);
            let count = self.processor_count.fetch_sub(1, Ordering::SeqCst);
            if count == 1 {
                self.empty.store(true, Ordering::SeqCst);
            }
        }

        // Also remove from idle queue
        let mut idle = self.idle_queue.lock();
        if let Some(pos) = idle.iter().position(|&p| p == processor_id) {
            idle.remove(pos);
            self.idle_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Mark processor as idle
    pub fn processor_idle(&self, processor_id: ProcessorId) {
        let mut idle = self.idle_queue.lock();
        if !idle.contains(&processor_id) {
            idle.push(processor_id);
            self.idle_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Mark processor as no longer idle
    pub fn processor_active(&self, processor_id: ProcessorId) {
        let mut idle = self.idle_queue.lock();
        if let Some(pos) = idle.iter().position(|&p| p == processor_id) {
            idle.remove(pos);
            self.idle_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Get an idle processor (for dispatch)
    pub fn get_idle_processor(&self) -> Option<ProcessorId> {
        self.idle_queue.lock().pop()
    }

    // ========================================================================
    // Task Management
    // ========================================================================

    /// Add a task to this set
    pub fn add_task(&self, task_id: TaskId) {
        let mut tasks = self.tasks.lock();
        if !tasks.contains(&task_id) {
            tasks.push(task_id);
            self.task_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Remove a task from this set
    pub fn remove_task(&self, task_id: TaskId) {
        let mut tasks = self.tasks.lock();
        if let Some(pos) = tasks.iter().position(|&t| t == task_id) {
            tasks.remove(pos);
            self.task_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    // ========================================================================
    // Thread Management
    // ========================================================================

    /// Add a thread to this set
    pub fn add_thread(&self, thread_id: ThreadId) {
        let mut threads = self.threads.lock();
        if !threads.contains(&thread_id) {
            threads.push(thread_id);
            self.thread_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Remove a thread from this set
    pub fn remove_thread(&self, thread_id: ThreadId) {
        let mut threads = self.threads.lock();
        if let Some(pos) = threads.iter().position(|&t| t == thread_id) {
            threads.remove(pos);
            self.thread_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    // ========================================================================
    // Scheduling
    // ========================================================================

    /// Enqueue a thread for scheduling
    pub fn enqueue_thread(&self, thread_id: ThreadId, priority: usize) {
        self.runq.lock().enqueue(thread_id, priority);
    }

    /// Dequeue highest priority thread
    pub fn dequeue_thread(&self) -> Option<ThreadId> {
        self.runq.lock().dequeue()
    }

    /// Get run queue count
    pub fn runq_count(&self) -> usize {
        self.runq.lock().count()
    }

    /// Get scheduler load
    pub fn sched_load(&self) -> u32 {
        self.sched_load.load(Ordering::SeqCst)
    }

    /// Update load statistics
    pub fn update_load(&self) {
        let thread_count = self.thread_count.load(Ordering::SeqCst);
        let processor_count = self.processor_count.load(Ordering::SeqCst);

        if processor_count > 0 {
            // Simple load calculation
            let load = (thread_count * 1000) / processor_count;
            self.sched_load.store(load as u32, Ordering::SeqCst);
        }
    }
}

impl Default for ProcessorSet {
    fn default() -> Self {
        Arc::try_unwrap(Self::new()).unwrap_or_else(|_arc| {
            // This shouldn't happen with a fresh Arc
            panic!("Cannot unwrap ProcessorSet Arc")
        })
    }
}

// ============================================================================
// Processor
// ============================================================================

/// Processor ID - newtype for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ProcessorId(pub u32);

/// Next processor ID
static NEXT_PROCESSOR_ID: AtomicU32 = AtomicU32::new(0);

fn alloc_processor_id() -> ProcessorId {
    ProcessorId(NEXT_PROCESSOR_ID.fetch_add(1, Ordering::SeqCst))
}

/// Physical processor
///
/// From Mach4 processor.h:
/// - runq: local run queue
/// - state: processor state
/// - next_thread: next thread to dispatch
/// - idle_thread: this processor's idle thread
/// - quantum: quantum for current thread
#[derive(Debug)]
pub struct Processor {
    /// Processor ID (slot number)
    id: ProcessorId,

    /// Local run queue
    pub runq: Mutex<RunQueue>,

    /// Processor state
    state: AtomicU32,

    /// Next thread to run (if dispatched)
    next_thread: Mutex<Option<ThreadId>>,

    /// This processor's idle thread ID
    idle_thread: Mutex<Option<ThreadId>>,

    /// This processor's idle SchedThread (for scheduling)
    idle_sched_thread: Mutex<Option<AllocArc<SchedThread>>>,

    /// Current quantum
    quantum: AtomicU32,

    /// First quantum in succession?
    first_quantum: AtomicBool,

    /// Last quantum assigned
    last_quantum: AtomicU32,

    /// Processor set this belongs to
    processor_set: Mutex<Option<Arc<ProcessorSet>>>,

    /// Port for operations
    processor_self: PortName,
}

impl Processor {
    /// Create a new processor
    pub fn new() -> Self {
        Self {
            id: alloc_processor_id(),
            runq: Mutex::new(RunQueue::new()),
            state: AtomicU32::new(ProcessorState::OffLine as u32),
            next_thread: Mutex::new(None),
            idle_thread: Mutex::new(None),
            idle_sched_thread: Mutex::new(None),
            quantum: AtomicU32::new(ProcessorSet::DEFAULT_QUANTUM),
            first_quantum: AtomicBool::new(true),
            last_quantum: AtomicU32::new(ProcessorSet::DEFAULT_QUANTUM),
            processor_set: Mutex::new(None),
            processor_self: PortName::NULL,
        }
    }

    /// Get processor ID
    pub fn id(&self) -> ProcessorId {
        self.id
    }

    /// Get processor state
    pub fn state(&self) -> ProcessorState {
        match self.state.load(Ordering::SeqCst) {
            0 => ProcessorState::OffLine,
            1 => ProcessorState::Running,
            2 => ProcessorState::Idle,
            3 => ProcessorState::Dispatching,
            4 => ProcessorState::Assign,
            5 => ProcessorState::Shutdown,
            _ => ProcessorState::OffLine,
        }
    }

    /// Set processor state
    pub fn set_state(&self, state: ProcessorState) {
        self.state.store(state as u32, Ordering::SeqCst);
    }

    /// Check if processor is idle
    pub fn is_idle(&self) -> bool {
        self.state() == ProcessorState::Idle
    }

    /// Set the idle thread for this processor
    pub fn set_idle_thread(&self, thread_id: ThreadId) {
        *self.idle_thread.lock() = Some(thread_id);
    }

    /// Get the idle thread
    pub fn idle_thread(&self) -> Option<ThreadId> {
        *self.idle_thread.lock()
    }

    /// Set next thread to dispatch
    pub fn set_next_thread(&self, thread_id: Option<ThreadId>) {
        *self.next_thread.lock() = thread_id;
    }

    /// Get next thread to dispatch
    pub fn next_thread(&self) -> Option<ThreadId> {
        *self.next_thread.lock()
    }

    /// Assign to a processor set
    pub fn assign_pset(&self, pset: Arc<ProcessorSet>) {
        let mut current_pset = self.processor_set.lock();

        // Remove from old pset
        if let Some(old_pset) = current_pset.take() {
            old_pset.remove_processor(self.id);
        }

        // Add to new pset
        pset.add_processor(self.id);
        *current_pset = Some(pset);
    }

    /// Get processor set
    pub fn processor_set(&self) -> Option<Arc<ProcessorSet>> {
        self.processor_set.lock().clone()
    }

    /// Start the processor
    pub fn start(&self) {
        // Create idle thread for this processor if not already created
        self.create_idle_thread();
        self.set_state(ProcessorState::Running);
    }

    /// Create the idle thread for this processor
    ///
    /// Each processor has its own idle thread that runs when there's no work.
    /// The idle thread uses architecture-specific low-power instructions:
    /// - ARM64: wfi (Wait For Interrupt)
    /// - x86_64: hlt (Halt until interrupt)
    pub fn create_idle_thread(&self) {
        let mut idle_thread_guard = self.idle_thread.lock();
        if idle_thread_guard.is_some() {
            return; // Already created
        }

        // Generate a unique thread ID for this processor's idle thread
        // Use negative IDs starting from processor ID to avoid conflicts
        let idle_thread_id = ThreadId(0x8000_0000 | self.id.0 as u64);

        // Create the idle SchedThread
        let idle_sched = AllocArc::new(SchedThread {
            thread_id: idle_thread_id,
            task_id: TaskId(0), // Kernel task
            priority: 0,       // Lowest priority - only runs when nothing else to do
            quantum: core::sync::atomic::AtomicU64::new(TIME_QUANTUM_MS),
            state: Mutex::new(ThreadState::Ready),
            affinity: AtomicUsize::new(1 << (self.id.0 as usize)), // Pin to this CPU
            context: Mutex::new({
                let mut ctx = Context::new();
                // Set PC to idle thread entry point
                ctx.pc = idle_thread_entry as usize as u64;
                // Stack would be allocated separately in a full implementation
                ctx
            }),
        });

        *idle_thread_guard = Some(idle_thread_id);
        *self.idle_sched_thread.lock() = Some(idle_sched);
    }

    /// Get the idle SchedThread for this processor
    pub fn get_idle_sched_thread(&self) -> Option<AllocArc<SchedThread>> {
        self.idle_sched_thread.lock().clone()
    }

    /// Enter idle state
    ///
    /// Called when the processor has no threads to run.
    /// Transitions from Running to Idle state.
    pub fn enter_idle(&self) {
        if self.state() == ProcessorState::Running {
            self.set_state(ProcessorState::Idle);

            // Notify processor set that we're idle
            if let Some(pset) = self.processor_set() {
                pset.processor_idle(self.id);
            }
        }
    }

    /// Exit idle state
    ///
    /// Called when work becomes available on an idle processor.
    /// Transitions from Idle to Dispatching to Running state.
    pub fn exit_idle(&self) {
        if self.state() == ProcessorState::Idle {
            self.set_state(ProcessorState::Dispatching);

            // Notify processor set that we're active
            if let Some(pset) = self.processor_set() {
                pset.processor_active(self.id);
            }

            self.set_state(ProcessorState::Running);
        }
    }

    /// Check if this processor should go idle
    ///
    /// Returns true if both local and processor set run queues are empty.
    pub fn should_idle(&self) -> bool {
        if !self.runq.lock().is_empty() {
            return false;
        }

        if let Some(pset) = self.processor_set() {
            if !pset.runq.lock().is_empty() {
                return false;
            }
        }

        true
    }

    /// Shutdown the processor
    pub fn shutdown(&self) {
        self.set_state(ProcessorState::Shutdown);

        // Remove from processor set
        let mut pset = self.processor_set.lock();
        if let Some(ps) = pset.take() {
            ps.remove_processor(self.id);
        }
    }

    /// Enqueue thread on local run queue
    pub fn enqueue(&self, thread_id: ThreadId, priority: usize) {
        self.runq.lock().enqueue(thread_id, priority);
    }

    /// Dequeue from local run queue
    pub fn dequeue(&self) -> Option<ThreadId> {
        self.runq.lock().dequeue()
    }

    /// Get local runq count
    pub fn runq_count(&self) -> usize {
        self.runq.lock().count()
    }

    /// Consume quantum
    pub fn tick_quantum(&self) -> bool {
        let q = self.quantum.fetch_sub(1, Ordering::SeqCst);
        q == 1 // Returns true when quantum exhausted
    }

    /// Reset quantum
    pub fn reset_quantum(&self, quantum: u32) {
        self.quantum.store(quantum, Ordering::SeqCst);
        self.first_quantum.store(true, Ordering::SeqCst);
    }
}

impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Processor Management
// ============================================================================

/// Maximum number of CPUs supported
pub const NCPUS: usize = 64;

/// Global processor array
static PROCESSORS: spin::Once<Mutex<Vec<Processor>>> = spin::Once::new();

/// Default processor set
static DEFAULT_PSET: spin::Once<Arc<ProcessorSet>> = spin::Once::new();

/// Master processor
static MASTER_PROCESSOR: AtomicU32 = AtomicU32::new(0);

/// Initialize processor subsystem
pub fn init() {
    PROCESSORS.call_once(|| Mutex::new(Vec::new()));
    DEFAULT_PSET.call_once(ProcessorSet::new);

    // Create and register the boot processor
    let boot_processor = Processor::new();
    let boot_id = boot_processor.id();

    // Assign to default pset
    boot_processor.assign_pset(default_pset());
    boot_processor.start();

    MASTER_PROCESSOR.store(boot_id.0, Ordering::SeqCst);

    PROCESSORS.get().unwrap().lock().push(boot_processor);
}

/// Get the default processor set
pub fn default_pset() -> Arc<ProcessorSet> {
    DEFAULT_PSET
        .get()
        .expect("Processor subsystem not initialized")
        .clone()
}

/// Get the master processor
pub fn master_processor() -> Option<ProcessorId> {
    if PROCESSORS.get().is_some() {
        Some(ProcessorId(MASTER_PROCESSOR.load(Ordering::SeqCst)))
    } else {
        None
    }
}

/// Register a new processor
pub fn register_processor() -> ProcessorId {
    let processor = Processor::new();
    let id = processor.id();

    processor.assign_pset(default_pset());

    PROCESSORS.get().unwrap().lock().push(processor);

    id
}

/// Get processor by ID
pub fn get_processor(id: ProcessorId) -> Option<ProcessorId> {
    let processors = PROCESSORS.get()?.lock();
    processors.iter().find(|p| p.id() == id).map(|p| p.id())
}

/// Apply function to processor
pub fn with_processor<F, R>(id: ProcessorId, f: F) -> Option<R>
where
    F: FnOnce(&Processor) -> R,
{
    let processors = PROCESSORS.get()?.lock();
    processors.iter().find(|p| p.id() == id).map(f)
}

/// Get current processor (based on CPU number)
pub fn current_processor() -> Option<ProcessorId> {
    // In a real implementation, this would use cpu_number()
    // For now, return master processor
    master_processor()
}

/// Get current processor set
pub fn current_pset() -> Arc<ProcessorSet> {
    // For now, always return default pset
    default_pset()
}

// ============================================================================
// Per-Processor Idle Thread Entry
// ============================================================================

/// Idle thread entry point for per-CPU idle threads
///
/// This function never returns. It executes a low-power wait instruction
/// in a loop, allowing the processor to save power while waiting for work.
/// The timer interrupt will wake the processor when needed.
#[cfg(not(test))]
pub extern "C" fn idle_thread_entry() -> ! {
    loop {
        // Check if there's work before going idle
        if let Some(proc_id) = current_processor() {
            with_processor(proc_id, |proc| {
                if !proc.should_idle() {
                    // Work available - exit idle state
                    proc.exit_idle();
                    return;
                }
                proc.enter_idle();
            });
        }

        // Execute architecture-specific low-power wait
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("wfi"); // Wait For Interrupt
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt"); // Halt until interrupt
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            // Spin for other architectures
            core::hint::spin_loop();
        }
    }
}

/// Idle thread entry point (test mode - no privileged instructions)
#[cfg(test)]
pub extern "C" fn idle_thread_entry() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Wake an idle processor to handle new work
///
/// Called when a thread is enqueued and an idle processor should be woken.
pub fn kick_idle_processor(pset: &ProcessorSet) {
    // Find an idle processor in this set
    if let Some(proc_id) = pset.get_idle_processor() {
        with_processor(proc_id, |proc| {
            proc.exit_idle();

            // On ARM64, we could send an SGI (Software Generated Interrupt)
            // to wake the processor. On x86_64, we could use an IPI.
            // For now, the timer interrupt will wake it.
            #[cfg(target_arch = "aarch64")]
            {
                // SEV (Send Event) wakes any processor waiting in WFE
                // Note: WFI requires interrupt to wake, SEV wakes WFE
                unsafe {
                    core::arch::asm!("sev");
                }
            }
        });
    }
}

/// Initialize per-CPU idle threads for all registered processors
pub fn init_idle_threads() {
    if let Some(processors) = PROCESSORS.get() {
        let processors_guard = processors.lock();
        for proc in processors_guard.iter() {
            proc.create_idle_thread();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_queue() {
        let mut runq = RunQueue::new();
        assert!(runq.is_empty());

        runq.enqueue(ThreadId(1), 16);
        runq.enqueue(ThreadId(2), 20);
        runq.enqueue(ThreadId(3), 10);

        assert_eq!(runq.count(), 3);

        // Should get highest priority first (20)
        assert_eq!(runq.dequeue(), Some(ThreadId(2)));
        assert_eq!(runq.dequeue(), Some(ThreadId(1)));
        assert_eq!(runq.dequeue(), Some(ThreadId(3)));
        assert!(runq.is_empty());
    }

    #[test]
    fn test_processor_state() {
        let proc = Processor::new();
        assert_eq!(proc.state(), ProcessorState::OffLine);

        proc.start();
        assert_eq!(proc.state(), ProcessorState::Running);
    }
}
