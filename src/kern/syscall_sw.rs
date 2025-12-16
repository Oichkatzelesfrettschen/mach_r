//! Mach Trap Table - System Call Switch
//!
//! Based on Mach4 kern/syscall_sw.h/c by CMU (1987) and Utah CSL (1993-1994)
//!
//! This module defines the Mach trap table which maps trap numbers to
//! kernel functions. Mach uses negative trap numbers for its system calls
//! (positive numbers are reserved for Unix).
//!
//! ## Architecture
//!
//! - Traps 0-9: Reserved for Unix
//! - Trap 25: mach_msg_trap (primary IPC)
//! - Traps 26-29: Self ports (reply, thread, task, host)
//! - Traps 59-61: Scheduler (swtch_pri, swtch, thread_switch)
//! - Traps 64-76: VM and port syscalls

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ipc::PortName;

// ============================================================================
// Trap Numbers (Constants)
// ============================================================================

/// Mach message trap number
pub const MACH_MSG_TRAP: i32 = 25;
/// Reply port trap
pub const MACH_REPLY_PORT: i32 = 26;
/// Thread self trap
pub const MACH_THREAD_SELF: i32 = 27;
/// Task self trap
pub const MACH_TASK_SELF: i32 = 28;
/// Host self trap
pub const MACH_HOST_SELF: i32 = 29;

/// Switch with priority trap
pub const SWTCH_PRI: i32 = 59;
/// Switch trap
pub const SWTCH: i32 = 60;
/// Thread switch trap
pub const THREAD_SWITCH: i32 = 61;

/// VM map trap
pub const SYSCALL_VM_MAP: i32 = 64;
/// VM allocate trap
pub const SYSCALL_VM_ALLOCATE: i32 = 65;
/// VM deallocate trap
pub const SYSCALL_VM_DEALLOCATE: i32 = 66;

/// Task create trap
pub const SYSCALL_TASK_CREATE: i32 = 68;
/// Task terminate trap
pub const SYSCALL_TASK_TERMINATE: i32 = 69;
/// Task suspend trap
pub const SYSCALL_TASK_SUSPEND: i32 = 70;
/// Task set special port trap
pub const SYSCALL_TASK_SET_SPECIAL_PORT: i32 = 71;

/// Port allocate trap
pub const SYSCALL_MACH_PORT_ALLOCATE: i32 = 72;
/// Port deallocate trap
pub const SYSCALL_MACH_PORT_DEALLOCATE: i32 = 73;
/// Port insert right trap
pub const SYSCALL_MACH_PORT_INSERT_RIGHT: i32 = 74;
/// Port allocate name trap
pub const SYSCALL_MACH_PORT_ALLOCATE_NAME: i32 = 75;
/// Thread depress abort trap
pub const SYSCALL_THREAD_DEPRESS_ABORT: i32 = 76;

/// Maximum trap number
pub const MACH_TRAP_COUNT: usize = 130;

// ============================================================================
// Return Codes
// ============================================================================

/// Kernel return type (Mach kern_return_t)
pub type KernReturn = i32;

/// Success
pub const KERN_SUCCESS: KernReturn = 0;
/// Invalid argument
pub const KERN_INVALID_ARGUMENT: KernReturn = 4;
/// Invalid task
pub const KERN_INVALID_TASK: KernReturn = 5;
/// Invalid right
pub const KERN_INVALID_RIGHT: KernReturn = 17;
/// No space
pub const KERN_NO_SPACE: KernReturn = 3;
/// Resource shortage
pub const KERN_RESOURCE_SHORTAGE: KernReturn = 6;
/// Operation not supported
pub const KERN_NOT_SUPPORTED: KernReturn = 46;

// ============================================================================
// Trap Flags
// ============================================================================

/// Trap may discard its kernel stack
/// Some architectures need to save more state in the PCB for these traps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapFlags(u32);

impl TrapFlags {
    /// No special flags
    pub const NONE: Self = Self(0);
    /// Trap may discard kernel stack
    pub const STACK: Self = Self(1);

    pub fn may_discard_stack(&self) -> bool {
        self.0 & 1 != 0
    }
}

// ============================================================================
// Trap Handler Function Type
// ============================================================================

/// Trap handler function signature
/// Takes trap arguments and returns a kernel return code
pub type TrapHandler = fn(&TrapArgs) -> KernReturn;

/// Trap arguments passed to handler
#[derive(Debug, Clone)]
pub struct TrapArgs {
    /// Argument registers (up to 7 for Mach traps)
    pub args: [usize; 7],
    /// Number of valid arguments
    pub arg_count: usize,
}

impl TrapArgs {
    pub fn new() -> Self {
        Self {
            args: [0; 7],
            arg_count: 0,
        }
    }

    pub fn with_args(args: &[usize]) -> Self {
        let mut trap_args = Self::new();
        let count = args.len().min(7);
        trap_args.args[..count].copy_from_slice(&args[..count]);
        trap_args.arg_count = count;
        trap_args
    }

    pub fn arg(&self, index: usize) -> usize {
        if index < self.arg_count {
            self.args[index]
        } else {
            0
        }
    }

    pub fn arg_u32(&self, index: usize) -> u32 {
        self.arg(index) as u32
    }

    pub fn arg_port(&self, index: usize) -> PortName {
        PortName(self.arg_u32(index))
    }
}

impl Default for TrapArgs {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Trap Table Entry
// ============================================================================

/// Mach trap table entry (mach_trap_t equivalent)
#[derive(Clone)]
pub struct MachTrap {
    /// Number of arguments
    pub arg_count: u8,
    /// Trap flags
    pub flags: TrapFlags,
    /// Handler function
    pub handler: TrapHandler,
    /// Trap name for debugging
    pub name: &'static str,
}

impl MachTrap {
    /// Create a new trap entry
    pub const fn new(name: &'static str, arg_count: u8, handler: TrapHandler) -> Self {
        Self {
            arg_count,
            flags: TrapFlags::NONE,
            handler,
            name,
        }
    }

    /// Create a trap that may discard stack
    pub const fn with_stack(name: &'static str, arg_count: u8, handler: TrapHandler) -> Self {
        Self {
            arg_count,
            flags: TrapFlags::STACK,
            handler,
            name,
        }
    }

    /// Execute this trap
    pub fn execute(&self, args: &TrapArgs) -> KernReturn {
        (self.handler)(args)
    }
}

impl core::fmt::Debug for MachTrap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MachTrap")
            .field("name", &self.name)
            .field("arg_count", &self.arg_count)
            .field("flags", &self.flags)
            .finish()
    }
}

// ============================================================================
// Default Trap Handlers
// ============================================================================

/// Invalid trap - returns null port
fn null_port(_args: &TrapArgs) -> KernReturn {
    // Returns MACH_PORT_NULL (0)
    0
}

/// Invalid trap - returns error
fn kern_invalid(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.invalid_calls.fetch_add(1, Ordering::Relaxed);
    KERN_INVALID_ARGUMENT
}

// ============================================================================
// Mach Trap Implementations
// ============================================================================

/// mach_msg_trap - Primary IPC mechanism
///
/// Arguments:
///   arg0: msg - pointer to message buffer
///   arg1: option - send/receive/timeout options
///   arg2: send_size - size of message to send
///   arg3: rcv_size - size of receive buffer
///   arg4: rcv_name - port name for receive
///   arg5: timeout - timeout in milliseconds
///   arg6: notify - notification port
fn mach_msg_trap_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.msg_calls.fetch_add(1, Ordering::Relaxed);

    let msg_ptr = args.arg(0) as *mut u8;
    let option = args.arg(1) as u32;
    let send_size = args.arg(2) as u32;
    let rcv_size = args.arg(3) as u32;
    let _rcv_name = args.arg_port(4);
    let _timeout = args.arg(5) as u32;
    let _notify = args.arg_port(6);

    // Validate user pointer
    if !super::copyio::is_user_range(msg_ptr as usize, send_size.max(rcv_size) as usize) {
        return KERN_INVALID_ARGUMENT;
    }

    // Build options for IPC subsystem
    let _do_send = (option & 0x1) != 0;
    let _do_receive = (option & 0x2) != 0;

    // Get current thread's task
    let _current_task_id = if let Some(thread) = crate::scheduler::current_thread() {
        thread.task_id
    } else {
        return KERN_INVALID_TASK;
    };

    // TODO: Call into IPC subsystem when API is stabilized
    // For now, return success as a placeholder
    // The actual implementation would call:
    // - crate::ipc::mach_msg::mach_msg() with appropriate options
    KERN_SUCCESS
}

/// mach_reply_port - Allocate a reply port for the current thread
///
/// Returns: Port name of new reply port
fn mach_reply_port_impl(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.self_calls.fetch_add(1, Ordering::Relaxed);

    // Get current task
    let current_task_id = if let Some(thread) = crate::scheduler::current_thread() {
        thread.task_id
    } else {
        return KERN_INVALID_TASK;
    };

    // Get task's IPC space
    let task_mgr = crate::task::manager();
    let task = match task_mgr.get_task(current_task_id) {
        Some(t) => t,
        None => return KERN_INVALID_TASK,
    };

    // Allocate a new port with receive right
    let port_name = task.allocate_port();

    // Return port name as result (in i32)
    port_name.0 as KernReturn
}

/// mach_thread_self - Get current thread's port
///
/// Returns: Port name representing the current thread
fn mach_thread_self_impl(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.self_calls.fetch_add(1, Ordering::Relaxed);

    // Get current thread
    let thread = match crate::scheduler::current_thread() {
        Some(t) => t,
        None => return KERN_INVALID_ARGUMENT,
    };

    // Return thread ID as port name
    // In a full implementation, this would look up the thread's kernel object port
    thread.thread_id.0 as KernReturn
}

/// mach_task_self - Get current task's port
///
/// Returns: Port name representing the current task
fn mach_task_self_impl(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.self_calls.fetch_add(1, Ordering::Relaxed);

    // Get current task
    let task_id = if let Some(thread) = crate::scheduler::current_thread() {
        thread.task_id
    } else {
        return KERN_INVALID_TASK;
    };

    // Return task ID as port name
    // In a full implementation, this would look up the task's kernel object port
    task_id.0 as KernReturn
}

/// mach_host_self - Get host port
///
/// Returns: Port name representing the host
fn mach_host_self_impl(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.self_calls.fetch_add(1, Ordering::Relaxed);

    // Return the host port
    // In a full implementation, this checks if caller is privileged
    if let Some(port_name) = super::host::host_self() {
        port_name.0 as KernReturn
    } else {
        // Return a well-known port ID for the host
        1 // MACH_PORT_HOST
    }
}

/// swtch_pri - Yield processor with priority hint
///
/// Arguments:
///   arg0: pri - priority hint for scheduling
fn swtch_pri_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.sched_calls.fetch_add(1, Ordering::Relaxed);

    let _pri = args.arg(0);

    // Yield the current CPU
    crate::scheduler::yield_cpu();

    KERN_SUCCESS
}

/// swtch - Yield processor
fn swtch_impl(_args: &TrapArgs) -> KernReturn {
    TRAP_STATS.sched_calls.fetch_add(1, Ordering::Relaxed);

    // Yield the current CPU
    crate::scheduler::yield_cpu();

    KERN_SUCCESS
}

/// thread_switch - Directed context switch
///
/// Arguments:
///   arg0: thread_name - target thread port (or 0 for any)
///   arg1: option - switch behavior flags
///   arg2: time - time hint in milliseconds
fn thread_switch_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.sched_calls.fetch_add(1, Ordering::Relaxed);

    let thread_name = args.arg_port(0);
    let option = args.arg(1) as u32;
    let time = args.arg(2) as u32;

    // Thread switch options
    const SWITCH_OPTION_NONE: u32 = 0;
    const SWITCH_OPTION_DEPRESS: u32 = 1;
    const SWITCH_OPTION_WAIT: u32 = 2;

    match option {
        SWITCH_OPTION_NONE => {
            // Simple yield
            crate::scheduler::yield_cpu();
        }
        SWITCH_OPTION_DEPRESS => {
            // Depress priority temporarily
            // TODO: Implement priority depression
            crate::scheduler::yield_cpu();
        }
        SWITCH_OPTION_WAIT => {
            // Wait for specified time
            if time > 0 {
                // TODO: Implement timed wait
                crate::scheduler::yield_cpu();
            }
        }
        _ => return KERN_INVALID_ARGUMENT,
    }

    // If specific thread requested, would handoff to it
    if thread_name.0 != 0 {
        // TODO: Look up thread by port name and switch to it
        let _ = thread_name;
    }

    KERN_SUCCESS
}

/// syscall_vm_map - Map memory into task's address space
fn syscall_vm_map_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.vm_calls.fetch_add(1, Ordering::Relaxed);

    let _target_task = args.arg_port(0);
    let _address = args.arg(1);
    let _size = args.arg(2);
    let _mask = args.arg(3);
    let _anywhere = args.arg(4);
    let _memory_object = args.arg(5);
    let _offset = args.arg(6);

    // TODO: Call into VM subsystem
    // Would call crate::mach_vm::vm_map()
    KERN_SUCCESS
}

/// syscall_vm_allocate - Allocate anonymous memory
fn syscall_vm_allocate_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.vm_calls.fetch_add(1, Ordering::Relaxed);

    let _target_task = args.arg_port(0);
    let _address = args.arg(1);
    let _size = args.arg(2);
    let _anywhere = args.arg(3);

    // TODO: Call into VM subsystem
    // Would call crate::mach_vm::vm_allocate()
    KERN_SUCCESS
}

/// syscall_vm_deallocate - Deallocate memory
fn syscall_vm_deallocate_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.vm_calls.fetch_add(1, Ordering::Relaxed);

    let _target_task = args.arg_port(0);
    let _address = args.arg(1);
    let _size = args.arg(2);

    // TODO: Call into VM subsystem
    // Would call crate::mach_vm::vm_deallocate()
    KERN_SUCCESS
}

/// syscall_mach_port_allocate - Allocate a port with specified right
///
/// Arguments:
///   arg0: task - target task port
///   arg1: right - type of right to create (receive, port_set, dead_name)
///   arg2: name_ptr - pointer to store allocated port name
fn syscall_mach_port_allocate_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.port_calls.fetch_add(1, Ordering::Relaxed);

    let task_port = args.arg_port(0);
    let right = args.arg(1) as u32;
    let name_ptr = args.arg(2) as *mut u32;

    // Validate name pointer
    if !super::copyio::is_user_address(name_ptr as usize) {
        return KERN_INVALID_ARGUMENT;
    }

    // Get task from port
    // For now, assume task_port.0 == task_id
    let task_id = crate::types::TaskId(task_port.0 as u64);
    let task_mgr = crate::task::manager();
    let task = match task_mgr.get_task(task_id) {
        Some(t) => t,
        None => return KERN_INVALID_TASK,
    };

    // Allocate port based on right type
    const MACH_PORT_RIGHT_RECEIVE: u32 = 1;
    const MACH_PORT_RIGHT_PORT_SET: u32 = 2;
    const MACH_PORT_RIGHT_DEAD_NAME: u32 = 3;

    let port_name = match right {
        MACH_PORT_RIGHT_RECEIVE => {
            task.allocate_port()
        }
        MACH_PORT_RIGHT_PORT_SET | MACH_PORT_RIGHT_DEAD_NAME => {
            // TODO: Implement port set and dead name allocation
            return KERN_NOT_SUPPORTED;
        }
        _ => return KERN_INVALID_ARGUMENT,
    };

    // Write port name to user space
    if let Err(_) = super::copyio::copyout_value(&port_name.0, name_ptr) {
        return KERN_INVALID_ARGUMENT;
    }

    KERN_SUCCESS
}

/// syscall_mach_port_deallocate - Deallocate a port right
fn syscall_mach_port_deallocate_impl(args: &TrapArgs) -> KernReturn {
    TRAP_STATS.port_calls.fetch_add(1, Ordering::Relaxed);

    let task_port = args.arg_port(0);
    let name = args.arg_port(1);

    // Get task from port
    let task_id = crate::types::TaskId(task_port.0 as u64);
    let task_mgr = crate::task::manager();
    let _task = match task_mgr.get_task(task_id) {
        Some(t) => t,
        None => return KERN_INVALID_TASK,
    };

    // TODO: Implement port deallocation through IPC space
    // For now, we just validate the task exists
    // Would call: crate::ipc::right::dealloc(&space, name)
    let _ = name;

    KERN_SUCCESS
}

// ============================================================================
// Trap Statistics
// ============================================================================

/// Trap execution statistics
pub struct TrapStats {
    /// Total trap calls
    pub total_calls: AtomicU64,
    /// Invalid trap calls
    pub invalid_calls: AtomicU64,
    /// Message trap calls
    pub msg_calls: AtomicU64,
    /// Self port calls
    pub self_calls: AtomicU64,
    /// Scheduler calls
    pub sched_calls: AtomicU64,
    /// VM calls
    pub vm_calls: AtomicU64,
    /// Port calls
    pub port_calls: AtomicU64,
}

impl TrapStats {
    pub const fn new() -> Self {
        Self {
            total_calls: AtomicU64::new(0),
            invalid_calls: AtomicU64::new(0),
            msg_calls: AtomicU64::new(0),
            self_calls: AtomicU64::new(0),
            sched_calls: AtomicU64::new(0),
            vm_calls: AtomicU64::new(0),
            port_calls: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> TrapStatsSnapshot {
        TrapStatsSnapshot {
            total_calls: self.total_calls.load(Ordering::Relaxed),
            invalid_calls: self.invalid_calls.load(Ordering::Relaxed),
            msg_calls: self.msg_calls.load(Ordering::Relaxed),
            self_calls: self.self_calls.load(Ordering::Relaxed),
            sched_calls: self.sched_calls.load(Ordering::Relaxed),
            vm_calls: self.vm_calls.load(Ordering::Relaxed),
            port_calls: self.port_calls.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of trap statistics
#[derive(Debug, Clone, Default)]
pub struct TrapStatsSnapshot {
    pub total_calls: u64,
    pub invalid_calls: u64,
    pub msg_calls: u64,
    pub self_calls: u64,
    pub sched_calls: u64,
    pub vm_calls: u64,
    pub port_calls: u64,
}

static TRAP_STATS: TrapStats = TrapStats::new();

// ============================================================================
// Trap Table
// ============================================================================

/// The Mach trap table
pub struct MachTrapTable {
    /// Trap entries
    traps: Vec<MachTrap>,
}

impl MachTrapTable {
    /// Create a new trap table with default entries
    pub fn new() -> Self {
        let mut traps = Vec::with_capacity(MACH_TRAP_COUNT);

        // Initialize all entries as invalid first
        for _ in 0..MACH_TRAP_COUNT {
            traps.push(MachTrap::new("kern_invalid", 0, kern_invalid));
        }

        // Set up standard Mach traps
        let table = Self { traps };
        table.init_standard_traps()
    }

    fn init_standard_traps(mut self) -> Self {
        // Traps 0-9: Reserved for Unix (kern_invalid)

        // Traps 10-13: Obsolete self ports (null_port for compatibility)
        for i in 10..14 {
            self.traps[i] = MachTrap::new("null_port", 0, null_port);
        }

        // Trap 25: mach_msg_trap (primary IPC mechanism)
        self.traps[MACH_MSG_TRAP as usize] =
            MachTrap::with_stack("mach_msg_trap", 7, mach_msg_trap_impl);

        // Trap 26: mach_reply_port
        self.traps[MACH_REPLY_PORT as usize] =
            MachTrap::new("mach_reply_port", 0, mach_reply_port_impl);

        // Trap 27: mach_thread_self
        self.traps[MACH_THREAD_SELF as usize] =
            MachTrap::new("mach_thread_self", 0, mach_thread_self_impl);

        // Trap 28: mach_task_self
        self.traps[MACH_TASK_SELF as usize] =
            MachTrap::new("mach_task_self", 0, mach_task_self_impl);

        // Trap 29: mach_host_self
        self.traps[MACH_HOST_SELF as usize] =
            MachTrap::new("mach_host_self", 0, mach_host_self_impl);

        // Trap 55: obsolete host_self
        self.traps[55] = MachTrap::new("null_port", 0, null_port);

        // Trap 59: swtch_pri
        self.traps[SWTCH_PRI as usize] = MachTrap::with_stack("swtch_pri", 1, swtch_pri_impl);

        // Trap 60: swtch
        self.traps[SWTCH as usize] = MachTrap::with_stack("swtch", 0, swtch_impl);

        // Trap 61: thread_switch
        self.traps[THREAD_SWITCH as usize] =
            MachTrap::with_stack("thread_switch", 3, thread_switch_impl);

        // Trap 64: syscall_vm_map
        self.traps[SYSCALL_VM_MAP as usize] =
            MachTrap::new("syscall_vm_map", 11, syscall_vm_map_impl);

        // Trap 65: syscall_vm_allocate
        self.traps[SYSCALL_VM_ALLOCATE as usize] =
            MachTrap::new("syscall_vm_allocate", 4, syscall_vm_allocate_impl);

        // Trap 66: syscall_vm_deallocate
        self.traps[SYSCALL_VM_DEALLOCATE as usize] =
            MachTrap::new("syscall_vm_deallocate", 3, syscall_vm_deallocate_impl);

        // Trap 72: syscall_mach_port_allocate
        self.traps[SYSCALL_MACH_PORT_ALLOCATE as usize] = MachTrap::new(
            "syscall_mach_port_allocate",
            3,
            syscall_mach_port_allocate_impl,
        );

        // Trap 73: syscall_mach_port_deallocate
        self.traps[SYSCALL_MACH_PORT_DEALLOCATE as usize] = MachTrap::new(
            "syscall_mach_port_deallocate",
            2,
            syscall_mach_port_deallocate_impl,
        );

        self
    }

    /// Get trap entry by number
    pub fn get(&self, trap_num: usize) -> Option<&MachTrap> {
        self.traps.get(trap_num)
    }

    /// Execute a trap
    pub fn execute(&self, trap_num: usize, args: &TrapArgs) -> KernReturn {
        TRAP_STATS.total_calls.fetch_add(1, Ordering::Relaxed);

        if let Some(trap) = self.get(trap_num) {
            trap.execute(args)
        } else {
            TRAP_STATS.invalid_calls.fetch_add(1, Ordering::Relaxed);
            KERN_INVALID_ARGUMENT
        }
    }

    /// Get trap count
    pub fn count(&self) -> usize {
        self.traps.len()
    }

    /// Register a custom trap handler
    pub fn register(&mut self, trap_num: usize, trap: MachTrap) -> bool {
        if trap_num < self.traps.len() {
            self.traps[trap_num] = trap;
            true
        } else {
            false
        }
    }
}

impl Default for MachTrapTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

use spin::Mutex;

static TRAP_TABLE: spin::Once<Mutex<MachTrapTable>> = spin::Once::new();

fn trap_table() -> &'static Mutex<MachTrapTable> {
    TRAP_TABLE.call_once(|| Mutex::new(MachTrapTable::new()))
}

/// Initialize the syscall switch subsystem
pub fn init() {
    let _ = trap_table();
}

// ============================================================================
// Public API
// ============================================================================

/// Execute a Mach trap
pub fn mach_trap(trap_num: usize, args: &TrapArgs) -> KernReturn {
    trap_table().lock().execute(trap_num, args)
}

/// Get trap statistics
pub fn trap_stats() -> TrapStatsSnapshot {
    TRAP_STATS.snapshot()
}

/// Register a custom trap handler
pub fn register_trap(trap_num: usize, trap: MachTrap) -> bool {
    trap_table().lock().register(trap_num, trap)
}

/// Get trap info for debugging
pub fn trap_info(_trap_num: usize) -> Option<(&'static str, u8)> {
    // Note: This is a simplified version - in real implementation
    // we'd need to handle the borrow properly
    None
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Execute mach_msg trap
pub fn do_mach_msg(
    msg: usize,
    option: usize,
    send_size: usize,
    rcv_size: usize,
    rcv_name: usize,
    timeout: usize,
    notify: usize,
) -> KernReturn {
    let args = TrapArgs::with_args(&[msg, option, send_size, rcv_size, rcv_name, timeout, notify]);
    mach_trap(MACH_MSG_TRAP as usize, &args)
}

/// Execute thread switch trap
pub fn do_thread_switch(thread: PortName, option: u32, time: u32) -> KernReturn {
    let args = TrapArgs::with_args(&[thread.0 as usize, option as usize, time as usize]);
    mach_trap(THREAD_SWITCH as usize, &args)
}

/// Execute swtch trap (yield)
pub fn do_swtch() -> KernReturn {
    let args = TrapArgs::new();
    mach_trap(SWTCH as usize, &args)
}

/// Execute swtch_pri trap (yield with priority)
pub fn do_swtch_pri(pri: i32) -> KernReturn {
    let args = TrapArgs::with_args(&[pri as usize]);
    mach_trap(SWTCH_PRI as usize, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_table_creation() {
        let table = MachTrapTable::new();
        assert_eq!(table.count(), MACH_TRAP_COUNT);
    }

    #[test]
    fn test_trap_args() {
        let args = TrapArgs::with_args(&[1, 2, 3]);
        assert_eq!(args.arg(0), 1);
        assert_eq!(args.arg(1), 2);
        assert_eq!(args.arg(2), 3);
        assert_eq!(args.arg(3), 0); // Out of bounds returns 0
    }

    #[test]
    fn test_trap_execution() {
        let table = MachTrapTable::new();
        let args = TrapArgs::new();

        // Test mach_task_self stub
        let result = table.execute(MACH_TASK_SELF as usize, &args);
        assert_eq!(result, KERN_SUCCESS);

        // Test invalid trap
        let result = table.execute(0, &args);
        assert_eq!(result, KERN_INVALID_ARGUMENT);
    }

    #[test]
    fn test_trap_flags() {
        let table = MachTrapTable::new();

        // mach_msg_trap should have STACK flag
        let msg_trap = table.get(MACH_MSG_TRAP as usize).unwrap();
        assert!(msg_trap.flags.may_discard_stack());

        // mach_task_self should not have STACK flag
        let task_trap = table.get(MACH_TASK_SELF as usize).unwrap();
        assert!(!task_trap.flags.may_discard_stack());
    }

    #[test]
    fn test_custom_trap() {
        let mut table = MachTrapTable::new();

        fn custom_handler(_args: &TrapArgs) -> KernReturn {
            42
        }

        let success = table.register(100, MachTrap::new("custom", 0, custom_handler));
        assert!(success);

        let result = table.execute(100, &TrapArgs::new());
        assert_eq!(result, 42);
    }
}
