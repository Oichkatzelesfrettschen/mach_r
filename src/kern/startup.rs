//! Kernel Startup - Bootstrap sequence
//!
//! Based on Mach4 kern/startup.c by CMU (1988-1991)
//!
//! This module handles the kernel startup sequence after the bootloader
//! hands control to the kernel. It initializes all subsystems in the correct
//! order and creates the initial kernel threads.
//!
//! ## Startup Sequence
//!
//! 1. Early init (panic, printf, console)
//! 2. Scheduler init
//! 3. VM bootstrap (physical pages, kernel heap)
//! 4. IPC bootstrap
//! 5. Full VM init
//! 6. Full IPC init
//! 7. Timer init
//! 8. Machine-specific init
//! 9. Task/Thread init
//! 10. Create kernel threads (idle, reaper, pageout, etc.)
//! 11. Start bootstrap task

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Boot State
// ============================================================================

/// Kernel startup state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum StartupPhase {
    /// Not yet started
    NotStarted = 0,
    /// Early console/panic init
    EarlyInit = 1,
    /// Scheduler structures initialized
    SchedInit = 2,
    /// VM bootstrap complete (physical memory)
    VmBootstrap = 3,
    /// IPC bootstrap complete
    IpcBootstrap = 4,
    /// Full VM initialized
    VmInit = 5,
    /// Full IPC initialized
    IpcInit = 6,
    /// Timers initialized
    TimerInit = 7,
    /// Machine-specific init done
    MachineInit = 8,
    /// Tasks/Threads initialized
    TaskInit = 9,
    /// Kernel threads created
    ThreadsCreated = 10,
    /// Bootstrap task started
    BootstrapStarted = 11,
    /// Fully running
    Running = 12,
}

impl StartupPhase {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => Self::NotStarted,
            1 => Self::EarlyInit,
            2 => Self::SchedInit,
            3 => Self::VmBootstrap,
            4 => Self::IpcBootstrap,
            5 => Self::VmInit,
            6 => Self::IpcInit,
            7 => Self::TimerInit,
            8 => Self::MachineInit,
            9 => Self::TaskInit,
            10 => Self::ThreadsCreated,
            11 => Self::BootstrapStarted,
            12 => Self::Running,
            _ => Self::NotStarted,
        }
    }
}

/// Global startup phase
static STARTUP_PHASE: AtomicU32 = AtomicU32::new(0);

/// Whether kernel has completed startup
static KERNEL_READY: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Machine Info
// ============================================================================

/// Machine configuration info (like Mach's machine_info structure)
#[derive(Debug, Clone)]
pub struct MachineInfo {
    /// Maximum number of CPUs supported
    pub max_cpus: u32,
    /// Number of available (online) CPUs
    pub avail_cpus: u32,
    /// Total physical memory size in bytes
    pub memory_size: u64,
    /// Available physical memory in bytes
    pub avail_memory: u64,
    /// Kernel major version
    pub major_version: u32,
    /// Kernel minor version
    pub minor_version: u32,
}

impl MachineInfo {
    pub const fn new() -> Self {
        Self {
            max_cpus: 1,
            avail_cpus: 0,
            memory_size: 0,
            avail_memory: 0,
            major_version: 0,
            minor_version: 1,
        }
    }
}

impl Default for MachineInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Global machine info
static MACHINE_INFO: spin::Once<spin::Mutex<MachineInfo>> = spin::Once::new();

fn machine_info() -> &'static spin::Mutex<MachineInfo> {
    MACHINE_INFO.call_once(|| spin::Mutex::new(MachineInfo::new()))
}

// ============================================================================
// CPU Slot Info
// ============================================================================

/// Per-CPU slot information (like Mach's machine_slot)
#[derive(Debug, Clone)]
pub struct CpuSlot {
    /// Is this slot a CPU?
    pub is_cpu: bool,
    /// Is CPU running?
    pub running: bool,
    /// CPU type
    pub cpu_type: i32,
    /// CPU subtype
    pub cpu_subtype: i32,
    /// Clock frequency in Hz
    pub clock_freq: u64,
}

impl CpuSlot {
    pub const fn new() -> Self {
        Self {
            is_cpu: false,
            running: false,
            cpu_type: 0,
            cpu_subtype: 0,
            clock_freq: 0,
        }
    }

    pub fn init_as_cpu(&mut self, cpu_type: i32, cpu_subtype: i32) {
        self.is_cpu = true;
        self.running = false;
        self.cpu_type = cpu_type;
        self.cpu_subtype = cpu_subtype;
    }

    pub fn mark_running(&mut self) {
        self.running = true;
    }
}

impl Default for CpuSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum CPUs supported
pub const NCPUS: usize = 32;

/// CPU slots
static CPU_SLOTS: spin::Once<spin::Mutex<Vec<CpuSlot>>> = spin::Once::new();

fn cpu_slots() -> &'static spin::Mutex<Vec<CpuSlot>> {
    CPU_SLOTS.call_once(|| {
        let mut slots = Vec::with_capacity(NCPUS);
        for _ in 0..NCPUS {
            slots.push(CpuSlot::new());
        }
        spin::Mutex::new(slots)
    })
}

// ============================================================================
// Startup Timing
// ============================================================================

/// Startup timing statistics
#[derive(Debug, Clone, Default)]
pub struct StartupTiming {
    /// Time for each phase (in arbitrary units)
    pub phase_times: [u64; 13],
    /// Total startup time
    pub total_time: u64,
}

static STARTUP_TIMING: spin::Once<spin::Mutex<StartupTiming>> = spin::Once::new();

fn startup_timing() -> &'static spin::Mutex<StartupTiming> {
    STARTUP_TIMING.call_once(|| spin::Mutex::new(StartupTiming::default()))
}

/// Simple tick counter for timing (would use actual timer in real implementation)
static TICK_COUNTER: AtomicU64 = AtomicU64::new(0);

fn get_ticks() -> u64 {
    TICK_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// ============================================================================
// Startup Functions
// ============================================================================

/// Get current startup phase
pub fn get_startup_phase() -> StartupPhase {
    StartupPhase::from_u32(STARTUP_PHASE.load(Ordering::SeqCst))
}

/// Set startup phase
fn set_startup_phase(phase: StartupPhase) {
    let start = get_ticks();
    STARTUP_PHASE.store(phase as u32, Ordering::SeqCst);

    // Record timing
    let mut timing = startup_timing().lock();
    if (phase as usize) < timing.phase_times.len() {
        timing.phase_times[phase as usize] = start;
    }
}

/// Check if kernel is fully started
pub fn kernel_ready() -> bool {
    KERNEL_READY.load(Ordering::SeqCst)
}

/// Early initialization - panic and console
fn early_init() {
    set_startup_phase(StartupPhase::EarlyInit);
    // panic_init() - handled by Rust's panic handler
    // printf_init() - handled by console module
}

/// Scheduler initialization
fn sched_init() {
    set_startup_phase(StartupPhase::SchedInit);
    // Initialize scheduler structures
    // In our implementation, this is handled by kern::sched_prim
}

/// VM bootstrap - physical memory and early heap
fn vm_bootstrap() {
    set_startup_phase(StartupPhase::VmBootstrap);
    // Initialize physical page management
    // Set up kernel virtual address space
}

/// IPC bootstrap - basic port/message structures
fn ipc_bootstrap() {
    set_startup_phase(StartupPhase::IpcBootstrap);
    // Initialize IPC zones
    // Set up kernel IPC space
}

/// Full VM initialization
fn vm_init() {
    set_startup_phase(StartupPhase::VmInit);
    // Complete VM subsystem initialization
    // Start page fault handling
}

/// Full IPC initialization
fn ipc_init() {
    set_startup_phase(StartupPhase::IpcInit);
    // Complete IPC subsystem
    // Initialize notification system
}

/// Timer initialization
fn timer_init() {
    set_startup_phase(StartupPhase::TimerInit);
    // Initialize kernel timers
    // Start timeout handling
}

/// Machine-specific initialization
fn machine_init() {
    set_startup_phase(StartupPhase::MachineInit);

    // Detect CPU configuration
    #[cfg(target_arch = "x86_64")]
    {
        let mut info = machine_info().lock();
        info.max_cpus = 1; // Would detect actual CPUs
        info.major_version = 0;
        info.minor_version = 1;
    }

    #[cfg(target_arch = "aarch64")]
    {
        let mut info = machine_info().lock();
        info.max_cpus = 1;
        info.major_version = 0;
        info.minor_version = 1;
    }
}

/// Task and thread initialization
fn task_thread_init() {
    set_startup_phase(StartupPhase::TaskInit);
    // Initialize task subsystem
    // Initialize thread subsystem
    // Create kernel task
}

/// Create kernel threads
fn create_kernel_threads() {
    set_startup_phase(StartupPhase::ThreadsCreated);

    // In real implementation:
    // - Create idle thread per CPU
    // - Create reaper thread
    // - Create swapin thread
    // - Create scheduler thread
    // - Create pageout daemon
}

/// Start bootstrap task
fn start_bootstrap() {
    set_startup_phase(StartupPhase::BootstrapStarted);
    // Load and start the bootstrap server
    // This starts the first user-space process
}

// ============================================================================
// First User Process Creation
// ============================================================================

use alloc::sync::Arc;
use crate::kern::thread::{Thread, ThreadId, ThreadState};
use crate::mach_vm::{pmap, vm_map};

/// Information about the created init process
#[derive(Debug, Clone)]
pub struct InitProcessInfo {
    /// Init task
    pub task_id: crate::kern::thread::TaskId,
    /// Init thread
    pub thread_id: ThreadId,
    /// Entry point address
    pub entry_point: u64,
    /// Stack pointer
    pub stack_pointer: u64,
}

/// Create the first user process (init)
///
/// This sets up the initial user-space task that bootstraps the system.
/// In a full Mach system, this would be the bootstrap server.
pub fn create_init_process(entry_point: u64, stack_pointer: u64) -> Option<InitProcessInfo> {
    use crate::kern::task::task_create;
    use crate::kern::thread::thread_create;

    // Create init task
    let task = task_create(None);
    let task_id = task.id;

    // Create VM map for init using the map manager
    let user_space_min = 0x1000u64; // Start after NULL page
    let user_space_max = 0x0000_7FFF_FFFF_FFFFu64; // User space limit
    let map = vm_map::create(user_space_min, user_space_max);

    // Create pmap for hardware page tables
    let pmap_ref = pmap::pmap_create();
    *map.pmap_id.lock() = Some(pmap_ref.id);

    // Set task's map
    task.set_map(map.id);

    // Create IPC space for task
    let space = crate::ipc::space::create_space();
    task.set_ipc_space(space.id());

    // Create init thread
    let thread = thread_create(task_id);
    let thread_id = thread.id;

    // Set up thread's initial register state
    setup_thread_state(&thread, entry_point, stack_pointer);

    // Mark thread as runnable
    thread.set_state(ThreadState::RUN);

    Some(InitProcessInfo {
        task_id,
        thread_id,
        entry_point,
        stack_pointer,
    })
}

/// Set up initial thread state for user mode entry
fn setup_thread_state(thread: &Arc<Thread>, entry_point: u64, stack_pointer: u64) {
    // Set program counter (entry point)
    thread.set_pc(entry_point);

    // Set stack pointer
    thread.set_sp(stack_pointer);

    // Architecture-specific setup
    #[cfg(target_arch = "x86_64")]
    {
        // For x86_64, we need to set up:
        // - CS selector for user code (ring 3)
        // - SS selector for user stack
        // - RFLAGS with interrupts enabled
        // This is handled in the thread's saved state
    }

    #[cfg(target_arch = "aarch64")]
    {
        // For ARM64, we need to set up:
        // - SPSR_EL1 for return to EL0
        // - ELR_EL1 with entry point
        // This is handled in the thread's saved state
    }
}

/// Load init binary and create init process
///
/// This is the main entry point for creating the first user process.
pub fn bootstrap_init_process(init_binary: Option<&[u8]>) -> Option<InitProcessInfo> {
    use crate::kern::elf_loader::ElfLoader;

    // If we have a binary, try to load it
    if let Some(binary) = init_binary {
        // Parse and validate ELF
        let header = match ElfLoader::parse_header(binary) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let entry_point = header.entry_point();

        // Default stack setup
        let stack_top = 0x7FFF_FFFF_F000u64;
        let stack_pointer = stack_top - 8;

        return create_init_process(entry_point, stack_pointer);
    }

    // No binary - create a minimal init with default values
    // This is useful for testing kernel boot without a real init
    let default_entry = 0x400000u64; // Traditional ELF entry
    let default_stack = 0x7FFF_FFFF_FFF0u64;

    create_init_process(default_entry, default_stack)
}

/// Mark kernel as running
fn mark_running() {
    set_startup_phase(StartupPhase::Running);
    KERNEL_READY.store(true, Ordering::SeqCst);

    // Calculate total startup time
    let mut timing = startup_timing().lock();
    timing.total_time = get_ticks();
}

// ============================================================================
// Main Startup Entry Point
// ============================================================================

/// Main kernel startup sequence
///
/// Called from architecture-specific boot code after basic initialization.
/// Does not return - dispatches the first thread.
pub fn setup_main() {
    // Phase 1: Early init
    early_init();

    // Phase 2: Scheduler
    sched_init();

    // Phase 3: VM bootstrap
    vm_bootstrap();

    // Phase 4: IPC bootstrap
    ipc_bootstrap();

    // Phase 5: Full VM
    vm_init();

    // Phase 6: Full IPC
    ipc_init();

    // Phase 7: Timers
    timer_init();

    // Phase 8: Machine-specific
    machine_init();

    // Phase 9: Tasks/Threads
    task_thread_init();

    // Phase 10: Kernel threads
    create_kernel_threads();

    // Phase 11: Bootstrap task
    start_bootstrap();

    // Phase 12: Running
    mark_running();

    // In real implementation: cpu_launch_first_thread()
    // This function would not return
}

/// Secondary CPU startup
pub fn slave_main(cpu_num: usize) {
    // Mark CPU slot as running
    {
        let mut slots = cpu_slots().lock();
        if cpu_num < slots.len() {
            slots[cpu_num].mark_running();
        }
    }

    // Update available CPU count
    {
        let mut info = machine_info().lock();
        info.avail_cpus += 1;
    }

    // In real implementation: cpu_launch_first_thread(THREAD_NULL)
}

/// Mark a CPU as up and running
pub fn cpu_up(cpu_num: usize) {
    {
        let mut slots = cpu_slots().lock();
        if cpu_num < slots.len() {
            slots[cpu_num].mark_running();
        }
    }

    {
        let mut info = machine_info().lock();
        info.avail_cpus += 1;
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Get machine information
pub fn get_machine_info() -> MachineInfo {
    machine_info().lock().clone()
}

/// Set memory size (called during VM init)
pub fn set_memory_size(total: u64, available: u64) {
    let mut info = machine_info().lock();
    info.memory_size = total;
    info.avail_memory = available;
}

/// Initialize a CPU slot
pub fn init_cpu_slot(cpu_num: usize, cpu_type: i32, cpu_subtype: i32) {
    let mut slots = cpu_slots().lock();
    if cpu_num < slots.len() {
        slots[cpu_num].init_as_cpu(cpu_type, cpu_subtype);
    }
}

/// Get CPU slot info
pub fn get_cpu_slot(cpu_num: usize) -> Option<CpuSlot> {
    let slots = cpu_slots().lock();
    slots.get(cpu_num).cloned()
}

/// Get startup timing info
pub fn get_startup_timing() -> StartupTiming {
    startup_timing().lock().clone()
}

/// Initialize the startup subsystem (for module init)
pub fn init() {
    let _ = machine_info();
    let _ = cpu_slots();
    let _ = startup_timing();
}

// ============================================================================
// Kernel Thread Types
// ============================================================================

/// Types of kernel threads created during startup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelThreadType {
    /// Idle thread (one per CPU)
    Idle,
    /// Thread reaper (cleans up dead threads)
    Reaper,
    /// Swap-in thread
    Swapin,
    /// Scheduler thread
    Scheduler,
    /// Pageout daemon
    Pageout,
    /// Action thread (SMP shutdown coordination)
    Action,
}

impl KernelThreadType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Reaper => "reaper",
            Self::Swapin => "swapin",
            Self::Scheduler => "scheduler",
            Self::Pageout => "pageout",
            Self::Action => "action",
        }
    }
}

// ============================================================================
// Startup Callbacks
// ============================================================================

/// Callback function type for startup hooks
pub type StartupCallback = fn();

/// Registered startup callbacks
static STARTUP_CALLBACKS: spin::Once<spin::Mutex<Vec<StartupCallback>>> = spin::Once::new();

fn startup_callbacks() -> &'static spin::Mutex<Vec<StartupCallback>> {
    STARTUP_CALLBACKS.call_once(|| spin::Mutex::new(Vec::new()))
}

/// Register a callback to be called during startup
pub fn register_startup_callback(callback: StartupCallback) {
    startup_callbacks().lock().push(callback);
}

/// Run all registered startup callbacks
pub fn run_startup_callbacks() {
    let callbacks = startup_callbacks().lock();
    for callback in callbacks.iter() {
        callback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_phase() {
        assert_eq!(StartupPhase::from_u32(0), StartupPhase::NotStarted);
        assert_eq!(StartupPhase::from_u32(12), StartupPhase::Running);
        assert_eq!(StartupPhase::from_u32(99), StartupPhase::NotStarted);
    }

    #[test]
    fn test_machine_info() {
        let info = MachineInfo::new();
        assert_eq!(info.max_cpus, 1);
        assert_eq!(info.avail_cpus, 0);
    }

    #[test]
    fn test_cpu_slot() {
        let mut slot = CpuSlot::new();
        assert!(!slot.is_cpu);
        assert!(!slot.running);

        slot.init_as_cpu(7, 3);
        assert!(slot.is_cpu);
        assert!(!slot.running);

        slot.mark_running();
        assert!(slot.running);
    }

    #[test]
    fn test_kernel_thread_type() {
        assert_eq!(KernelThreadType::Idle.name(), "idle");
        assert_eq!(KernelThreadType::Pageout.name(), "pageout");
    }
}
