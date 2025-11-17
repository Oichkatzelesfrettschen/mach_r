//! Preemptive round-robin scheduler with priority support

use crate::task::{Thread, ThreadId, ThreadState};
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::arch::asm;

/// Number of priority levels
const PRIORITY_LEVELS: usize = 128;

/// Time quantum in milliseconds
const TIME_QUANTUM_MS: u32 = 10;

/// Scheduler state
pub struct Scheduler {
    /// Ready queues for each priority level
    ready_queues: [VecDeque<ThreadId>; PRIORITY_LEVELS],
    /// All threads in the system
    threads: Vec<Option<Box<Thread>>>,
    /// Currently running thread
    current: Option<ThreadId>,
    /// Idle thread ID
    idle_thread: Option<ThreadId>,
}

static mut SCHEDULER: Option<Scheduler> = None;

impl Scheduler {
    pub fn new() -> Self {
        const EMPTY: VecDeque<ThreadId> = VecDeque::new();
        Self {
            ready_queues: [EMPTY; PRIORITY_LEVELS],
            threads: Vec::new(),
            current: None,
            idle_thread: None,
        }
    }
    
    /// Add a thread to the scheduler
    pub fn add_thread(&mut self, thread: Thread) {
        let id = thread.id;
        let priority = thread.priority.0 as usize;
        
        // Store thread
        self.threads.push(Some(Box::new(thread)));
        
        // Add to ready queue if ready
        if self.get_thread(id).map(|t| t.state) == Some(ThreadState::Ready) {
            self.ready_queues[priority].push_back(id);
        }
    }
    
    /// Get a thread by ID
    fn get_thread(&self, id: ThreadId) -> Option<&Thread> {
        for slot in &self.threads {
            if let Some(thread) = slot {
                if thread.id == id {
                    return Some(thread);
                }
            }
        }
        None
    }
    
    /// Get a mutable thread by ID
    fn get_thread_mut(&mut self, id: ThreadId) -> Option<&mut Thread> {
        for slot in &mut self.threads {
            if let Some(thread) = slot {
                if thread.id == id {
                    return Some(thread);
                }
            }
        }
        None
    }
    
    /// Find next thread to run
    fn find_next(&mut self) -> Option<ThreadId> {
        // Search from highest to lowest priority
        for priority in (0..PRIORITY_LEVELS).rev() {
            if let Some(thread_id) = self.ready_queues[priority].pop_front() {
                return Some(thread_id);
            }
        }
        
        // No ready threads, use idle
        self.idle_thread
    }
    
    /// Schedule next thread
    pub fn schedule(&mut self) {
        // Save current thread context if running
        if let Some(current) = self.current {
            if let Some(thread) = self.get_thread_mut(current) {
                if thread.state == ThreadState::Running {
                    thread.state = ThreadState::Ready;
                    let priority = thread.priority.0 as usize;
                    self.ready_queues[priority].push_back(current);
                }
            }
        }
        
        // Find next thread
        let next = self.find_next();
        if let Some(next_id) = next {
            if let Some(thread) = self.get_thread_mut(next_id) {
                thread.state = ThreadState::Running;
                let ctx = thread.context.clone();
                self.current = Some(next_id);
                
                // Context switch
                unsafe {
                    switch_context(&ctx);
                }
            }
        }
    }
    
    /// Block a thread
    pub fn block_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.get_thread_mut(id) {
            thread.block();
        }
    }
    
    /// Wake a thread
    pub fn wake_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.get_thread_mut(id) {
            thread.unblock();
            if thread.state == ThreadState::Ready {
                let priority = thread.priority.0 as usize;
                self.ready_queues[priority].push_back(id);
            }
        }
    }
    
    /// Suspend a thread
    pub fn suspend_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.get_thread_mut(id) {
            thread.suspend();
        }
    }
    
    /// Resume a thread
    pub fn resume_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.get_thread_mut(id) {
            thread.resume();
            if thread.state == ThreadState::Ready {
                let priority = thread.priority.0 as usize;
                self.ready_queues[priority].push_back(id);
            }
        }
    }
    
    /// Terminate a thread
    pub fn terminate_thread(&mut self, id: ThreadId) {
        // Find and remove thread
        for (i, slot) in self.threads.iter_mut().enumerate() {
            if let Some(thread) = slot {
                if thread.id == id {
                    // Free resources
                    crate::memory::free_stack(thread.kernel_stack);
                    
                    // Remove thread
                    self.threads[i] = None;
                    
                    // If current, schedule new one
                    if self.current == Some(id) {
                        self.current = None;
                        self.schedule();
                    }
                    return;
                }
            }
        }
    }
    
    /// Yield current thread
    pub fn yield_current(&mut self) {
        self.schedule();
    }
}

/// Initialize scheduler
pub fn init() {
    unsafe {
        SCHEDULER = Some(Scheduler::new());
        
        // Create idle thread
        let idle = Thread::new(
            crate::task::TaskId(0),
            idle_thread as usize,
            crate::memory::alloc_stack(4096),
        );
        
        if let Some(sched) = &mut SCHEDULER {
            sched.idle_thread = Some(idle.id);
            sched.add_thread(idle);
        }
    }
}

/// Idle thread function
extern "C" fn idle_thread() -> ! {
    loop {
        unsafe {
            asm!("wfi");  // Wait for interrupt
        }
    }
}

/// Context switch (platform specific)
#[cfg(target_arch = "aarch64")]
unsafe fn switch_context(context: &crate::task::Context) {
    asm!(
        "
        // Load new context
        ldp x0, x1, [{ctx}, #0]
        ldp x2, x3, [{ctx}, #16]
        ldp x4, x5, [{ctx}, #32]
        ldp x6, x7, [{ctx}, #48]
        ldp x8, x9, [{ctx}, #64]
        ldp x10, x11, [{ctx}, #80]
        ldp x12, x13, [{ctx}, #96]
        ldp x14, x15, [{ctx}, #112]
        ldp x16, x17, [{ctx}, #128]
        ldp x18, x19, [{ctx}, #144]
        ldp x20, x21, [{ctx}, #160]
        ldp x22, x23, [{ctx}, #176]
        ldp x24, x25, [{ctx}, #192]
        ldp x26, x27, [{ctx}, #208]
        ldp x28, x29, [{ctx}, #224]
        ldr x30, [{ctx}, #240]
        
        // Load SP and PC
        ldr x0, [{ctx}, #248]
        mov sp, x0
        ldr x0, [{ctx}, #256]
        
        // Jump to new PC
        br x0
        ",
        ctx = in(reg) context,
        options(noreturn)
    );
}

#[cfg(not(target_arch = "aarch64"))]
unsafe fn switch_context(_context: &crate::task::Context) {
    // Stub for other architectures
}

/// Timer interrupt handler
pub fn timer_tick() {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.schedule();
        }
    }
}

/// Public scheduler interface
pub fn add_thread(thread: Thread) {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.add_thread(thread);
        }
    }
}

pub fn wake_thread(id: ThreadId) {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.wake_thread(id);
        }
    }
}

pub fn block_current() {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            if let Some(current) = sched.current {
                sched.block_thread(current);
                sched.schedule();
            }
        }
    }
}

pub fn suspend_thread(id: ThreadId) {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.suspend_thread(id);
        }
    }
}

pub fn resume_thread(id: ThreadId) {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.resume_thread(id);
        }
    }
}

pub fn terminate_thread(id: ThreadId) {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.terminate_thread(id);
        }
    }
}

pub fn yield_current() {
    unsafe {
        if let Some(sched) = &mut SCHEDULER {
            sched.yield_current();
        }
    }
}