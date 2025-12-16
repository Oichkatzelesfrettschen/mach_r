//! Task scheduler for Mach_R
//!
//! Implements preemptive round-robin scheduling with priority support.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use spin::Once;

use crate::task::{Context, ThreadState};
use crate::types::{TaskId, ThreadId}; // Add Context here

/// Number of priority levels (reduced in test mode to avoid stack overflow)
#[cfg(not(test))]
pub const PRIORITY_LEVELS: usize = 128;
#[cfg(test)]
pub const PRIORITY_LEVELS: usize = 8;

/// Default priority (adjusted for test mode's reduced levels)
#[cfg(not(test))]
pub const DEFAULT_PRIORITY: usize = 16;
#[cfg(test)]
pub const DEFAULT_PRIORITY: usize = 4;

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
    /// CPU context for thread switching
    pub context: Mutex<Context>,
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
            context: Mutex::new(Context::new()),
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
    pub fn unblock(&self, thread_id: ThreadId) {
        let mut queues = self.run_queues.lock();
        // Find the thread in any queue or current, change its state and re-add it.
        // This is a simplified approach, a more robust solution would involve a global thread table.
        for queue in queues.iter_mut() {
            if let Some(pos) = queue.threads.iter().position(|t| t.thread_id == thread_id) {
                let thread = queue.threads.remove(pos).unwrap();
                *thread.state.lock() = ThreadState::Ready;
                queue.threads.push_back(thread); // Re-add to the end of its priority queue
                return;
            }
        }
        // If the thread is the current running one, unblock it.
        if let Some(ref current) = *self.current.lock() {
            if current.thread_id == thread_id {
                *current.state.lock() = ThreadState::Ready;
            }
        }
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
            // If current is running and its quantum expired, put it back in queue
            if *current.state.lock() == ThreadState::Running {
                if current.quantum.load(Ordering::Relaxed) == 0 {
                    current.reset_quantum();
                    *current.state.lock() = ThreadState::Ready;
                    queues[current.priority].push(current.clone());
                } else {
                    // Quantum not expired, keep running
                    return;
                }
            } else if *current.state.lock() == ThreadState::Ready {
                // Current thread is ready, put it back in queue (e.g., if yielded)
                queues[current.priority].push(current.clone());
            }
        }

        // Find highest priority thread to run
        let next = queues
            .iter_mut()
            .rev() // Start from highest priority
            .find_map(|queue| queue.pop())
            .or_else(|| self.idle_thread.clone()); // Fallback to idle thread

        if let Some(next_thread) = next {
            // Perform context switch if different thread
            if let Some(ref current) = *current_lock {
                if current.thread_id != next_thread.thread_id {
                    *next_thread.state.lock() = ThreadState::Running; // Next becomes Running
                                                                      // SAFETY: context_switch performs low-level register manipulation
                    unsafe {
                        self.context_switch(current, &next_thread);
                    }
                }
            } else {
                // No current thread (e.g., initial boot)
                *next_thread.state.lock() = ThreadState::Running;
            }

            *current_lock = Some(next_thread);
            self.stats.context_switches.fetch_add(1, Ordering::Relaxed);
        }

        self.need_resched.store(false, Ordering::Release);
    }

    /// Perform context switch
    #[cfg(target_arch = "aarch64")]
    unsafe fn context_switch(&self, from: &Arc<SchedThread>, to: &Arc<SchedThread>) {
        // Need to get the actual Context pointers from the SchedThread's Mutex<Context>
        // and safely dereference them.
        let from_ctx_ptr: *mut Context = &mut *from.context.lock();
        let to_ctx_ptr: *mut Context = &mut *to.context.lock();

        asm!(
            "
            // Save current context (from->context)
            stp x0, x1, [x8, #0]
            stp x2, x3, [x8, #16]
            stp x4, x5, [x8, #32]
            stp x6, x7, [x8, #48]
            stp x8, x9, [x8, #64]
            stp x10, x11, [x8, #80]
            stp x12, x13, [x8, #96]
            stp x14, x15, [x8, #112]
            stp x16, x17, [x8, #128]
            stp x18, x19, [x8, #144]
            stp x20, x21, [x8, #160]
            stp x22, x23, [x8, #176]
            stp x24, x25, [x8, #192]
            stp x26, x27, [x8, #208]
            stp x28, x29, [x8, #224]
            str x30, [x8, #240]
            mrs x1, sp_el0
            str x1, [x8, #248] // Save SP
            adr x1, .
            str x1, [x8, #256] // Save PC (address of next instruction)
            mrs x1, spsr_el1
            str x1, [x8, #264] // Save PSTATE

            // Load new context (to->context)
            ldp x0, x1, [x9, #0]
            ldp x2, x3, [x9, #16]
            ldp x4, x5, [x9, #32]
            ldp x6, x7, [x9, #48]
            ldp x8, x9, [x9, #64]
            ldp x10, x11, [x9, #80]
            ldp x12, x13, [x9, #96]
            ldp x14, x15, [x9, #112]
            ldp x16, x17, [x9, #128]
            ldp x18, x19, [x9, #144]
            ldp x20, x21, [x9, #160]
            ldp x22, x23, [x9, #176]
            ldp x24, x25, [x9, #192]
            ldp x26, x27, [x9, #208]
            ldp x28, x29, [x9, #224]
            ldr x30, [x9, #240]
            
            ldr x0, [x9, #248]
            msr sp_el0, x0 // Load SP
            ldr x0, [x9, #264]
            msr spsr_el1, x0 // Load PSTATE
            ldr x0, [x9, #256] // Load PC

            // Jump to new PC
            br x0
            /* {x8} {x9} */ // Mark as used
            ",
            x8 = in(reg) from_ctx_ptr,
            x9 = in(reg) to_ctx_ptr,
            options(noreturn)
        );
    }

    /// Perform context switch - x86_64 variant
    #[cfg(target_arch = "x86_64")]
    unsafe fn context_switch(&self, from: &Arc<SchedThread>, to: &Arc<SchedThread>) {
        // Get pointers to the Context structures
        let from_ctx_ptr: *mut Context = &mut *from.context.lock();
        let to_ctx_ptr: *mut Context = &mut *to.context.lock();

        // x86_64 context switch saves callee-saved registers per System V ABI
        // rbx, rbp, r12-r15 are callee-saved
        // rsp is the stack pointer
        // rip (return address) is pushed on stack by call instruction
        asm!(
            "
            // Save current context (callee-saved registers)
            mov [rdi + 0], rbx
            mov [rdi + 8], rbp
            mov [rdi + 16], r12
            mov [rdi + 24], r13
            mov [rdi + 32], r14
            mov [rdi + 40], r15
            mov [rdi + 48], rsp
            // Save return address (where we'll resume)
            lea rax, [rip + 2f]
            mov [rdi + 56], rax
            // Save flags
            pushfq
            pop rax
            mov [rdi + 64], rax

            // Load new context (callee-saved registers)
            mov rbx, [rsi + 0]
            mov rbp, [rsi + 8]
            mov r12, [rsi + 16]
            mov r13, [rsi + 24]
            mov r14, [rsi + 32]
            mov r15, [rsi + 40]
            mov rsp, [rsi + 48]
            // Load flags
            mov rax, [rsi + 64]
            push rax
            popfq
            // Jump to saved return address
            mov rax, [rsi + 56]
            jmp rax

            2:
            // We return here when switched back
            ",
            in("rdi") from_ctx_ptr,
            in("rsi") to_ctx_ptr,
            out("rax") _,
            clobber_abi("C"),
        );
    }

    /// Perform context switch - stub for unsupported architectures
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    fn context_switch(&self, _from: &Arc<SchedThread>, _to: &Arc<SchedThread>) {
        // Stub for unsupported architectures
        // In a real implementation, this would panic or use a software fallback
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

    /// Suspend a thread
    pub fn suspend_thread(&self, thread_id: ThreadId) {
        // TODO: Implement suspending a thread.
        let _ = thread_id; // Suppress unused warning
        todo!();
    }

    /// Resume a thread
    pub fn resume_thread(&self, thread_id: ThreadId) {
        // TODO: Implement resuming a thread.
        let _ = thread_id; // Suppress unused warning
        todo!();
    }

    /// Terminate a thread
    pub fn terminate_thread(&self, thread_id: ThreadId) {
        // TODO: Implement terminating a thread.
        let _ = thread_id; // Suppress unused warning
        todo!();
    }
}

// ... (rest of the file until the global functions) ...

/// Global scheduler instance
static SCHEDULER: Once<Scheduler> = Once::new();

/// Initialize the scheduler
pub fn init(idle_thread_entry: unsafe extern "C" fn() -> !) {
    SCHEDULER.call_once(|| {
        let mut scheduler = Scheduler::new();

        // Create idle thread SchedThread
        let idle_thread_arc = Arc::new(SchedThread {
            thread_id: ThreadId(0), // Special ID for idle thread
            task_id: TaskId(0),     // Idle thread belongs to kernel task
            priority: 0,            // Lowest priority
            quantum: AtomicU64::new(TIME_QUANTUM_MS),
            state: Mutex::new(ThreadState::Ready),
            affinity: AtomicUsize::new(usize::MAX),
            context: Mutex::new({
                let mut ctx = Context::new();
                ctx.pc = idle_thread_entry as usize as u64; // Set PC to idle_thread_entry
                                                            // Stack will be set up by alloc_stack and passed during creation of the SchedThread
                                                            // This is a placeholder for now, actual stack setup happens elsewhere for SchedThread
                ctx
            }),
        });

        scheduler.init(idle_thread_arc); // Pass the Arc clone
        scheduler
    });
}

/// Get the global scheduler instance
pub fn global_scheduler() -> &'static Scheduler {
    SCHEDULER.get().expect("Scheduler not initialized")
}

/// Idle thread function
#[cfg(not(test))]
pub extern "C" fn idle_thread_entry() -> ! {
    loop {
        // Here we would perform low-power operations or simply wait for interrupts
        // using WFI/WFE (Wait For Interrupt/Event) instructions.
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("wfi");
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt");
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            // Spin for other architectures
            core::hint::spin_loop();
        }
    }
}

/// Idle thread function (test mode - no privileged instructions)
#[cfg(test)]
pub extern "C" fn idle_thread_entry() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// Public scheduler interface functions (adapted from real_os/kernel/src/scheduler)

pub fn add_thread(thread: Arc<SchedThread>) {
    global_scheduler().add_thread(thread);
}

pub fn remove_thread(thread_id: ThreadId) {
    global_scheduler().remove_thread(thread_id);
}

pub fn schedule() {
    global_scheduler().schedule();
}

pub fn timer_tick() {
    global_scheduler().tick();
}

pub fn should_reschedule() -> bool {
    global_scheduler().should_reschedule()
}

pub fn yield_cpu() {
    global_scheduler().yield_current();
}

pub fn block_current() {
    global_scheduler().block_current();
}

pub fn unblock_thread(thread_id: ThreadId) {
    global_scheduler().unblock(thread_id);
}

pub fn wake_thread(thread_id: ThreadId) {
    global_scheduler().unblock(thread_id);
}

pub fn current_thread() -> Option<Arc<SchedThread>> {
    global_scheduler().current_thread()
}

pub fn suspend_thread(thread_id: ThreadId) {
    global_scheduler().suspend_thread(thread_id);
}

pub fn resume_thread(thread_id: ThreadId) {
    global_scheduler().resume_thread(thread_id);
}

pub fn terminate_thread(thread_id: ThreadId) {
    global_scheduler().terminate_thread(thread_id);
}

// ============================================================================
// Timer Interrupt Integration
// ============================================================================

/// ARM Generic Timer IRQ numbers
#[cfg(target_arch = "aarch64")]
pub const TIMER_IRQ: u32 = 27; // Virtual timer IRQ (EL1)

/// x86_64 APIC timer IRQ (placeholder - typically uses local APIC timer)
#[cfg(target_arch = "x86_64")]
pub const TIMER_IRQ: u32 = 32; // IRQ 0 mapped to vector 32 after PIC remapping

/// Generic fallback timer IRQ
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
pub const TIMER_IRQ: u32 = 0;

/// Timer interrupt handler
///
/// Called by the interrupt controller when the timer fires.
/// This is the main entry point for preemptive scheduling.
pub fn timer_interrupt_handler(_irq: u32) {
    // Update scheduler tick and check for quantum expiration
    timer_tick();

    // Reprogram the timer for the next tick
    #[cfg(target_arch = "aarch64")]
    reprogram_timer_arm64();

    // Check if we need to reschedule
    if should_reschedule() {
        // In a real implementation, we would set a flag for deferred scheduling
        // or directly reschedule if safe to do so
        // For now, just call schedule directly (unsafe in interrupt context)
        // schedule(); // Commented out: scheduling in interrupt context is complex
    }
}

/// Reprogram ARM64 timer for next tick
#[cfg(target_arch = "aarch64")]
fn reprogram_timer_arm64() {
    use core::arch::asm;

    // Get current counter value
    let current: u64;
    unsafe {
        asm!("mrs {}, cntvct_el0", out(reg) current);
    }

    // Get frequency for interval calculation (assume ~1MHz for 10ms quantum)
    let freq: u64;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }

    // Calculate interval for TIME_QUANTUM_MS milliseconds
    let interval = (freq * TIME_QUANTUM_MS) / 1000;

    // Set compare value for next interrupt
    let next_compare = current + interval;
    unsafe {
        asm!("msr cntv_cval_el0, {}", in(reg) next_compare);
    }
}

/// Initialize timer interrupt for scheduling
///
/// Sets up periodic timer interrupts and connects them to the scheduler.
/// Must be called after both scheduler and interrupt controller are initialized.
#[cfg(not(test))]
pub fn init_timer_interrupt() -> Result<(), &'static str> {
    #[cfg(target_arch = "aarch64")]
    {
        // Register timer interrupt handler with the interrupt controller
        if let Err(_) = crate::drivers::interrupt::register_system_handler(
            TIMER_IRQ,
            timer_interrupt_handler,
        ) {
            return Err("Failed to register timer interrupt handler");
        }

        // Set up periodic timer
        init_arm64_timer()?;
    }

    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 timer setup would go here (PIT, APIC timer, or HPET)
        // For now, this is a stub
        // Would typically use Local APIC timer for per-CPU scheduling
    }

    Ok(())
}

/// Initialize ARM64 Generic Timer for scheduler
#[cfg(all(target_arch = "aarch64", not(test)))]
fn init_arm64_timer() -> Result<(), &'static str> {
    use core::arch::asm;

    // Get frequency
    let freq: u64;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }

    if freq == 0 {
        return Err("Timer frequency is zero");
    }

    // Get current counter
    let current: u64;
    unsafe {
        asm!("mrs {}, cntvct_el0", out(reg) current);
    }

    // Calculate interval for first tick
    let interval = (freq * TIME_QUANTUM_MS) / 1000;

    // Set compare value
    let compare = current + interval;
    unsafe {
        asm!("msr cntv_cval_el0, {}", in(reg) compare);
    }

    // Enable virtual timer (ENABLE bit = 1, IMASK bit = 0)
    let ctl: u64 = 1;
    unsafe {
        asm!("msr cntv_ctl_el0, {}", in(reg) ctl);
    }

    Ok(())
}

/// Test stub for timer interrupt initialization
#[cfg(test)]
pub fn init_timer_interrupt() -> Result<(), &'static str> {
    // No-op in test mode
    Ok(())
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
        assert!(thread.tick()); // 1 -> 0, returns true
    }
}
