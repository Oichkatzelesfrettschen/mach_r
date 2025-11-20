//! Task scheduler for Mach_R
//!
//! Implements preemptive round-robin scheduling with priority support.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use crate::types::{TaskId, ThreadId};
use crate::task::ThreadState;

/// Number of priority levels
pub const PRIORITY_LEVELS: usize = 32;

/// Default priority
pub const DEFAULT_PRIORITY: usize = 16;

/// Time quantum in milliseconds
pub const TIME_QUANTUM_MS: u64 = 10;

/// Scheduler statistics
pub struct SchedStats {
    /// Total context switches
    pub context_switches: AtomicU64,
    /// Total ticks
    pub ticks: AtomicU64,
    /// Idle ticks
    pub idle_ticks: AtomicU64,
}

/// Thread control block for scheduling
#[derive(Debug)]
pub struct SchedThread {
    /// Thread ID
    pub thread_id: ThreadId,
    /// Owning task
    pub task_id: TaskId,
    /// Thread priority (0-31, higher is better)
    pub priority: usize,
    /// Remaining time quantum
    pub quantum: AtomicU64,
    /// Thread state
    pub state: Mutex<ThreadState>,
    /// CPU affinity mask
    pub affinity: AtomicUsize,
}

impl SchedThread {
    /// Create a new schedulable thread
    pub fn new(thread_id: ThreadId, task_id: TaskId, priority: usize) -> Arc<Self> {
        Arc::new(SchedThread {
            thread_id,
            task_id,
            priority: priority.min(PRIORITY_LEVELS - 1),
            quantum: AtomicU64::new(TIME_QUANTUM_MS),
            state: Mutex::new(ThreadState::Ready),
            affinity: AtomicUsize::new(usize::MAX), // All CPUs
        })
    }
    
    /// Reset time quantum
    pub fn reset_quantum(&self) {
        self.quantum.store(TIME_QUANTUM_MS, Ordering::Relaxed);
    }
    
    /// Decrease quantum
    pub fn tick(&self) -> bool {
        self.quantum.fetch_sub(1, Ordering::Relaxed) == 1
    }
}

/// Run queue for a single priority level
#[derive(Debug)]
struct RunQueue {
    /// Ready threads at this priority
    threads: VecDeque<Arc<SchedThread>>,
}

impl RunQueue {
    /// Create a new run queue
    fn new() -> Self {
        RunQueue {
            threads: VecDeque::new(),
        }
    }
    
    /// Add thread to queue
    fn push(&mut self, thread: Arc<SchedThread>) {
        self.threads.push_back(thread);
    }
    
    /// Remove thread from front
    fn pop(&mut self) -> Option<Arc<SchedThread>> {
        self.threads.pop_front()
    }
    
    /// Check if empty
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }
}

/// Main scheduler structure
pub struct Scheduler {
    /// Run queues for each priority level
    run_queues: Mutex<[RunQueue; PRIORITY_LEVELS]>,
    /// Currently running thread
    current: Mutex<Option<Arc<SchedThread>>>,
    /// Idle thread (runs when nothing else is ready)
    idle_thread: Option<Arc<SchedThread>>,
    /// Scheduler statistics
    stats: SchedStats,
    /// Need reschedule flag
    need_resched: AtomicBool,
    /// Scheduler enabled flag
    enabled: AtomicBool,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        let mut queues = Vec::new();
        for _ in 0..PRIORITY_LEVELS {
            queues.push(RunQueue::new());
        }
        
        Scheduler {
            run_queues: Mutex::new(queues.try_into().unwrap()),
            current: Mutex::new(None),
            idle_thread: None,
            stats: SchedStats {
                context_switches: AtomicU64::new(0),
                ticks: AtomicU64::new(0),
                idle_ticks: AtomicU64::new(0),
            },
            need_resched: AtomicBool::new(false),
            enabled: AtomicBool::new(false),
        }
    }
    
    /// Initialize scheduler with idle thread
    pub fn init(&mut self, idle_thread: Arc<SchedThread>) {
        self.idle_thread = Some(idle_thread.clone());
        *self.current.lock() = Some(idle_thread);
        self.enabled.store(true, Ordering::Release);
    }
    
    /// Add a thread to the scheduler
    pub fn add_thread(&self, thread: Arc<SchedThread>) {
        let mut queues = self.run_queues.lock();
        queues[thread.priority].push(thread);
        self.need_resched.store(true, Ordering::Release);
    }
    
    /// Remove a thread from the scheduler
    pub fn remove_thread(&self, thread_id: ThreadId) {
        let mut queues = self.run_queues.lock();
        for queue in queues.iter_mut() {
            queue.threads.retain(|t| t.thread_id != thread_id);
        }
    }
    
    /// Block current thread
    pub fn block_current(&self) {
        if let Some(thread) = self.current.lock().as_ref() {
            *thread.state.lock() = ThreadState::Blocked;
            self.need_resched.store(true, Ordering::Release);
        }
    }
    
    /// Unblock a thread
    pub fn unblock(&self, thread: Arc<SchedThread>) {
        *thread.state.lock() = ThreadState::Ready;
        self.add_thread(thread);
    }
    
    /// Yield current thread
    pub fn yield_current(&self) {
        self.need_resched.store(true, Ordering::Release);
        self.schedule();
    }
    
    /// Main scheduling decision
    pub fn schedule(&self) {
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        
        let mut current_lock = self.current.lock();
        let mut queues = self.run_queues.lock();
        
        // Save current thread if still ready
        if let Some(ref current) = *current_lock {
            if *current.state.lock() == ThreadState::Ready {
                // Put back in queue if quantum not expired
                if current.quantum.load(Ordering::Relaxed) > 0 {
                    // Keep running
                    return;
                }
                // Quantum expired, put back in queue
                current.reset_quantum();
                queues[current.priority].push(current.clone());
            }
        }
        
        // Find highest priority thread to run
        let next = queues.iter_mut()
            .rev() // Start from highest priority
            .find_map(|queue| queue.pop())
            .or_else(|| self.idle_thread.clone());
        
        if let Some(next_thread) = next {
            // Perform context switch
            if let Some(ref current) = *current_lock {
                if current.thread_id != next_thread.thread_id {
                    self.context_switch(current, &next_thread);
                }
            }
            
            *current_lock = Some(next_thread);
            self.stats.context_switches.fetch_add(1, Ordering::Relaxed);
        }
        
        self.need_resched.store(false, Ordering::Release);
    }
    
    /// Perform context switch
    fn context_switch(&self, _from: &SchedThread, _to: &SchedThread) {
        // In real implementation:
        // 1. Save current thread's registers
        // 2. Switch page tables if different task
        // 3. Load new thread's registers
        // 4. Return to new thread's execution
        
        // This would involve assembly code
    }
    
    /// Timer tick handler
    pub fn tick(&self) {
        self.stats.ticks.fetch_add(1, Ordering::Relaxed);
        
        // Decrease current thread's quantum
        if let Some(ref current) = *self.current.lock() {
            if current.tick() {
                // Quantum expired
                self.need_resched.store(true, Ordering::Release);
            }
            
            if current.thread_id == self.idle_thread.as_ref().unwrap().thread_id {
                self.stats.idle_ticks.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    
    /// Check if reschedule is needed
    pub fn should_reschedule(&self) -> bool {
        self.need_resched.load(Ordering::Acquire)
    }
    
    /// Get current thread
    pub fn current_thread(&self) -> Option<Arc<SchedThread>> {
        self.current.lock().clone()
    }
    
    /// Get scheduler statistics
    pub fn stats(&self) -> &SchedStats {
        &self.stats
    }
}

/// Global scheduler instance (simplified for now)
static mut SCHEDULER_INITIALIZED: bool = false;

/// Initialize the scheduler
pub fn init() {
    unsafe {
        SCHEDULER_INITIALIZED = true;
    }
}

/// Add thread to scheduler
pub fn add_thread(_thread: Arc<SchedThread>) {
    // Simplified for compilation
}

/// Remove thread from scheduler
pub fn remove_thread(_thread_id: ThreadId) {
    // Simplified for compilation
}

/// Schedule next thread
pub fn schedule() {
    // Simplified for compilation
}

/// Timer tick
pub fn tick() {
    // Poll servers (name, vm, pager) once per tick to service IPC
    crate::servers::poll_once();
}

/// Check if should reschedule
pub fn should_reschedule() -> bool {
    false // Simplified for compilation
}

/// Yield CPU
pub fn yield_cpu() {
    // Simplified for compilation
}

/// Block current thread
pub fn block() {
    // Simplified for compilation
}

/// Unblock thread
pub fn unblock(_thread: Arc<SchedThread>) {
    // Simplified for compilation
}

/// Get current thread
pub fn current_thread() -> Option<Arc<SchedThread>> {
    None // Simplified for compilation
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sched_thread_creation() {
        let thread = SchedThread::new(ThreadId(1), TaskId(1), 16);
        assert_eq!(thread.priority, 16);
        assert_eq!(thread.quantum.load(Ordering::Relaxed), TIME_QUANTUM_MS);
    }
    
    #[test]
    fn test_run_queue() {
        let mut queue = RunQueue::new();
        assert!(queue.is_empty());
        
        let thread = SchedThread::new(ThreadId(1), TaskId(1), 16);
        queue.push(thread.clone());
        assert!(!queue.is_empty());
        
        let popped = queue.pop().unwrap();
        assert_eq!(popped.thread_id, ThreadId(1));
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_quantum_tick() {
        let thread = SchedThread::new(ThreadId(1), TaskId(1), 16);
        thread.quantum.store(2, Ordering::Relaxed);
        
        assert!(!thread.tick()); // 2 -> 1
        assert!(thread.tick());  // 1 -> 0, returns true
    }
}
