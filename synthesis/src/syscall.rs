//! System call interface for Mach_R
//!
//! Implements the Mach system call interface including port operations,
//! task/thread management, and memory operations.

use crate::types::{PortId, TaskId};
use crate::message::Message;
use crate::init::ServiceState;

/// System call numbers (Mach-style)
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum SyscallNumber {
    // Port operations
    PortAllocate = 1,
    PortDeallocate = 2,
    PortInsertRight = 3,
    PortExtractRight = 4,
    
    // Message operations
    MsgSend = 10,
    MsgReceive = 11,
    MsgRpc = 12,
    
    // Task operations
    TaskCreate = 20,
    TaskTerminate = 21,
    TaskSuspend = 22,
    TaskResume = 23,
    TaskGetSpecialPort = 24,
    TaskSetSpecialPort = 25,
    
    // Thread operations
    ThreadCreate = 30,
    ThreadTerminate = 31,
    ThreadGetState = 32,
    ThreadSetState = 33,
    ThreadSuspend = 34,
    ThreadResume = 35,
    
    // VM operations
    VmAllocate = 40,
    VmDeallocate = 41,
    VmProtect = 42,
    VmInherit = 43,
    VmRead = 44,
    VmWrite = 45,
    VmCopy = 46,
    VmMap = 47,
    
    // Host operations
    HostInfo = 50,
    HostKernelVersion = 51,
    
    // Clock operations
    ClockGetTime = 60,
    ClockSleep = 61,
    
    // Misc
    ThreadSwitch = 70,
    TaskSelf = 71,
    ThreadSelf = 72,
    
    // Enhanced system calls for Mach_R components
    // Device driver operations
    DeviceOpen = 100,
    DeviceClose = 101,
    DeviceRead = 102,
    DeviceWrite = 103,
    DeviceControl = 104,
    DeviceList = 105,
    
    // Service management operations
    ServiceStart = 110,
    ServiceStop = 111,
    ServiceRestart = 112,
    ServiceStatus = 113,
    ServiceList = 114,
    ServiceCreate = 115,
    ServiceDestroy = 116,
    
    // Boot/System operations
    SystemInfo = 120,
    SystemReboot = 121,
    SystemShutdown = 122,
    SystemUptime = 123,
    
    // Timer operations
    TimerCreate = 130,
    TimerDestroy = 131,
    TimerSetTimeout = 132,
    TimerGetTime = 133,
    
    // Console/Debug operations
    ConsoleWrite = 140,
    ConsoleRead = 141,
    DebugPrint = 142,
}

/// System call return codes
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyscallResult {
    Success = 0,
    InvalidArgument = -1,
    NoMemory = -2,
    NoAccess = -3,
    InvalidPort = -4,
    InvalidTask = -5,
    InvalidThread = -6,
    InvalidAddress = -7,
    PortDead = -8,
    NameExists = -9,
    NotSupported = -10,
    ResourceShortage = -11,
    Interrupted = -12,
    DeviceError = -13,
    ServiceError = -14,
    SystemError = -15,
    TimerError = -16,
}

/// Port right types for syscalls
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum MachPortRight {
    Send = 1,
    Receive = 2,
    SendOnce = 3,
    PortSet = 4,
    DeadName = 5,
}

/// Message options for msg_send/receive
#[derive(Debug, Clone, Copy)]
pub struct MsgOption(u64);

impl MsgOption {
    pub const NONE: Self = MsgOption(0);
    pub const SEND_MSG: Self = MsgOption(1);
    pub const RCV_MSG: Self = MsgOption(2);
    pub const SEND_TIMEOUT: Self = MsgOption(0x10);
    pub const RCV_TIMEOUT: Self = MsgOption(0x100);
    pub const SEND_INTERRUPT: Self = MsgOption(0x40);
    pub const RCV_INTERRUPT: Self = MsgOption(0x400);
}

/// Main system call dispatcher
pub fn dispatch(syscall_num: u64, args: &[u64]) -> i64 {
    // Convert to enum
    let syscall = match syscall_num {
        1 => SyscallNumber::PortAllocate,
        2 => SyscallNumber::PortDeallocate,
        10 => SyscallNumber::MsgSend,
        11 => SyscallNumber::MsgReceive,
        20 => SyscallNumber::TaskCreate,
        21 => SyscallNumber::TaskTerminate,
        30 => SyscallNumber::ThreadCreate,
        40 => SyscallNumber::VmAllocate,
        71 => SyscallNumber::TaskSelf,
        72 => SyscallNumber::ThreadSelf,
        // Enhanced syscalls
        100 => SyscallNumber::DeviceOpen,
        101 => SyscallNumber::DeviceClose,
        102 => SyscallNumber::DeviceRead,
        103 => SyscallNumber::DeviceWrite,
        104 => SyscallNumber::DeviceControl,
        105 => SyscallNumber::DeviceList,
        110 => SyscallNumber::ServiceStart,
        111 => SyscallNumber::ServiceStop,
        112 => SyscallNumber::ServiceRestart,
        113 => SyscallNumber::ServiceStatus,
        114 => SyscallNumber::ServiceList,
        120 => SyscallNumber::SystemInfo,
        123 => SyscallNumber::SystemUptime,
        130 => SyscallNumber::TimerCreate,
        133 => SyscallNumber::TimerGetTime,
        140 => SyscallNumber::ConsoleWrite,
        141 => SyscallNumber::ConsoleRead,
        142 => SyscallNumber::DebugPrint,
        _ => return SyscallResult::NotSupported as i64,
    };
    
    // Dispatch to handler
    match syscall {
        SyscallNumber::PortAllocate => sys_port_allocate(args),
        SyscallNumber::PortDeallocate => sys_port_deallocate(args),
        SyscallNumber::MsgSend => sys_msg_send(args),
        SyscallNumber::MsgReceive => sys_msg_receive(args),
        SyscallNumber::TaskCreate => sys_task_create(args),
        SyscallNumber::TaskTerminate => sys_task_terminate(args),
        SyscallNumber::ThreadCreate => sys_thread_create(args),
        SyscallNumber::VmAllocate => sys_vm_allocate(args),
        SyscallNumber::TaskSelf => sys_task_self(args),
        SyscallNumber::ThreadSelf => sys_thread_self(args),
        // Enhanced syscall handlers
        SyscallNumber::DeviceOpen => sys_device_open(args),
        SyscallNumber::DeviceClose => sys_device_close(args),
        SyscallNumber::DeviceRead => sys_device_read(args),
        SyscallNumber::DeviceWrite => sys_device_write(args),
        SyscallNumber::DeviceControl => sys_device_control(args),
        SyscallNumber::DeviceList => sys_device_list(args),
        SyscallNumber::ServiceStart => sys_service_start(args),
        SyscallNumber::ServiceStop => sys_service_stop(args),
        SyscallNumber::ServiceRestart => sys_service_restart(args),
        SyscallNumber::ServiceStatus => sys_service_status(args),
        SyscallNumber::ServiceList => sys_service_list(args),
        SyscallNumber::SystemInfo => sys_system_info(args),
        SyscallNumber::SystemUptime => sys_system_uptime(args),
        SyscallNumber::TimerCreate => sys_timer_create(args),
        SyscallNumber::TimerGetTime => sys_timer_get_time(args),
        SyscallNumber::ConsoleWrite => sys_console_write(args),
        SyscallNumber::ConsoleRead => sys_console_read(args),
        SyscallNumber::DebugPrint => sys_debug_print(args),
        _ => SyscallResult::NotSupported as i64,
    }
}

/// Allocate a new port
fn sys_port_allocate(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let task_id = TaskId(args[0]);
    let right_type = args[1];
    
    // Get task
    let task_manager = crate::task::manager();
    let task = match task_manager.get_task(task_id) {
        Some(t) => t,
        None => return SyscallResult::InvalidTask as i64,
    };
    
    // Allocate port
    let port = task.allocate_port();
    
    // Add appropriate rights
    match right_type {
        1 => port.add_send_right(),
        2 => {}, // Receive right is implicit
        _ => return SyscallResult::InvalidArgument as i64,
    }
    
    // Return port ID (in real implementation, would return port name)
    port.id().0 as i64
}

/// Deallocate a port
fn sys_port_deallocate(args: &[u64]) -> i64 {
    if args.is_empty() {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let _port_id = PortId(args[0]);
    
    // In real implementation:
    // 1. Verify caller has rights
    // 2. Remove from task's port namespace
    // 3. Deallocate if no more references
    
    SyscallResult::Success as i64
}

/// Send a message
fn sys_msg_send(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let port_id = PortId(args[0]);
    let msg_addr = args[1] as *const u8;
    let msg_size = args[2] as usize;
    
    // In real implementation:
    // 1. Copy message from user space
    // 2. Validate message format
    // 3. Check send rights
    // 4. Queue message
    
    unsafe {
        // This is unsafe and simplified
        let msg_data = core::slice::from_raw_parts(msg_addr, msg_size.min(256));
        
        match Message::new_inline(port_id, msg_data) {
            Ok(msg) => {
                // Would need to look up actual port
                // For now, return success
                SyscallResult::Success as i64
            }
            Err(_) => SyscallResult::InvalidArgument as i64,
        }
    }
}

/// Receive a message
fn sys_msg_receive(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let port_id = PortId(args[0]);
    let msg_addr = args[1] as *mut u8;
    let msg_size = args[2] as usize;
    
    // In real implementation:
    // 1. Check receive rights
    // 2. Dequeue message or block
    // 3. Copy to user space
    
    SyscallResult::Success as i64
}

/// Create a new task
fn sys_task_create(args: &[u64]) -> i64 {
    let _parent_task = if !args.is_empty() {
        TaskId(args[0])
    } else {
        // Use kernel task as parent
        TaskId(0)
    };
    
    // Create new task
    let task_manager = crate::task::manager();
    let new_task = task_manager.create_task();
    
    // Return task ID
    new_task.id().0 as i64
}

/// Terminate a task
fn sys_task_terminate(args: &[u64]) -> i64 {
    if args.is_empty() {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let task_id = TaskId(args[0]);
    
    // Get task
    let task_manager = crate::task::manager();
    match task_manager.get_task(task_id) {
        Some(task) => {
            task.terminate();
            SyscallResult::Success as i64
        }
        None => SyscallResult::InvalidTask as i64,
    }
}

/// Create a new thread
fn sys_thread_create(args: &[u64]) -> i64 {
    if args.is_empty() {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let task_id = TaskId(args[0]);
    
    // Get task
    let task_manager = crate::task::manager();
    match task_manager.get_task(task_id) {
        Some(task) => {
            let thread_id = task.create_thread();
            thread_id.0 as i64
        }
        None => SyscallResult::InvalidTask as i64,
    }
}

/// Allocate virtual memory
fn sys_vm_allocate(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let _task_id = TaskId(args[0]);
    let _address = args[1] as usize;
    let _size = args[2] as usize;
    
    // In real implementation:
    // 1. Get task's VM map
    // 2. Find free region if address is 0
    // 3. Allocate pages
    // 4. Update page tables
    
    // For now, return the requested address
    _address as i64
}

/// Get current task
fn sys_task_self(_args: &[u64]) -> i64 {
    // In real implementation, would return current task's ID
    // For now, return kernel task
    0
}

/// Get current thread
fn sys_thread_self(_args: &[u64]) -> i64 {
    // In real implementation, would return current thread's ID
    if let Some(thread) = crate::scheduler::current_thread() {
        thread.thread_id.0 as i64
    } else {
        0
    }
}

/// Convert user-space message to kernel message
pub fn user_to_kernel_msg(_user_msg: *const u8, _size: usize) -> Result<Message, SyscallResult> {
    // In real implementation:
    // 1. Validate user pointer
    // 2. Copy from user space safely
    // 3. Parse message format
    // 4. Validate port rights
    
    Err(SyscallResult::NotSupported)
}

/// Copy kernel message to user space
pub fn kernel_to_user_msg(_msg: &Message, _user_msg: *mut u8, _size: usize) -> Result<(), SyscallResult> {
    // In real implementation:
    // 1. Validate user pointer
    // 2. Check size is sufficient
    // 3. Copy to user space safely
    // 4. Transfer port rights
    
    Err(SyscallResult::NotSupported)
}

// Enhanced syscall implementations

/// Open a device by name
fn sys_device_open(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let device_name_ptr = args[0] as *const u8;
    let device_name_len = args[1] as usize;
    
    // TODO: Validate user pointer and copy name safely
    // For now, return mock device handle
    1 // Mock device handle
}

/// Close a device handle
fn sys_device_close(args: &[u64]) -> i64 {
    if args.len() < 1 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let device_handle = args[0];
    
    // TODO: Close actual device handle
    SyscallResult::Success as i64
}

/// Read from device
fn sys_device_read(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let device_handle = args[0];
    let buffer_ptr = args[1] as *mut u8;
    let buffer_size = args[2] as usize;
    
    // TODO: Validate buffer and read from actual device
    0 // Bytes read
}

/// Write to device
fn sys_device_write(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let device_handle = args[0];
    let buffer_ptr = args[1] as *const u8;
    let buffer_size = args[2] as usize;
    
    // TODO: Validate buffer and write to actual device
    buffer_size as i64 // Bytes written
}

/// Control device (ioctl-like)
fn sys_device_control(args: &[u64]) -> i64 {
    if args.len() < 3 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let device_handle = args[0];
    let command = args[1] as u32;
    let arg = args[2];
    
    // TODO: Call actual device control
    SyscallResult::Success as i64
}

/// List available devices
fn sys_device_list(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *mut u8;
    let buffer_size = args[1] as usize;
    
    // TODO: Fill buffer with device list
    0 // Number of devices
}

/// Start a service
fn sys_service_start(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let service_name_ptr = args[0] as *const u8;
    let service_name_len = args[1] as usize;
    
    // TODO: Get service name safely from user space
    // TODO: Start actual service via init system
    SyscallResult::Success as i64
}

/// Stop a service
fn sys_service_stop(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let service_name_ptr = args[0] as *const u8;
    let service_name_len = args[1] as usize;
    
    // TODO: Stop actual service via init system
    SyscallResult::Success as i64
}

/// Restart a service
fn sys_service_restart(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let service_name_ptr = args[0] as *const u8;
    let service_name_len = args[1] as usize;
    
    // TODO: Restart actual service via init system
    SyscallResult::Success as i64
}

/// Get service status
fn sys_service_status(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let service_name_ptr = args[0] as *const u8;
    let service_name_len = args[1] as usize;
    
    // TODO: Get actual service status
    ServiceState::Running as i64 // Mock status
}

/// List services
fn sys_service_list(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *mut u8;
    let buffer_size = args[1] as usize;
    
    // TODO: Fill buffer with service list
    0 // Number of services
}

/// Get system information
fn sys_system_info(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *mut u8;
    let buffer_size = args[1] as usize;
    
    // TODO: Fill buffer with system info (kernel version, memory, etc.)
    SyscallResult::Success as i64
}

/// Get system uptime
fn sys_system_uptime(args: &[u64]) -> i64 {
    // TODO: Get actual system uptime from boot
    0 // Uptime in milliseconds
}

/// Create a timer
fn sys_timer_create(args: &[u64]) -> i64 {
    if args.len() < 1 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let timeout_ms = args[0];
    
    // TODO: Create actual timer using timer driver
    1 // Mock timer handle
}

/// Get current time
fn sys_timer_get_time(args: &[u64]) -> i64 {
    // TODO: Get actual time from timer driver
    if let Some(_) = crate::drivers::device_manager() {
        // Mock timestamp in microseconds
        123456789
    } else {
        SyscallResult::SystemError as i64
    }
}

/// Write to console
fn sys_console_write(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *const u8;
    let buffer_size = args[1] as usize;
    
    // TODO: Validate buffer and write to console via UART driver
    // For now, try to write via debug output
    unsafe {
        let slice = core::slice::from_raw_parts(buffer_ptr, buffer_size);
        if let Ok(s) = core::str::from_utf8(slice) {
            crate::boot::serial_println(s);
        }
    }
    
    buffer_size as i64
}

/// Read from console
fn sys_console_read(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *mut u8;
    let buffer_size = args[1] as usize;
    
    // TODO: Read from console via UART driver
    0 // Bytes read
}

/// Debug print (for kernel debugging)
fn sys_debug_print(args: &[u64]) -> i64 {
    if args.len() < 2 {
        return SyscallResult::InvalidArgument as i64;
    }
    
    let buffer_ptr = args[0] as *const u8;
    let buffer_size = args[1] as usize;
    
    unsafe {
        let slice = core::slice::from_raw_parts(buffer_ptr, buffer_size);
        if let Ok(s) = core::str::from_utf8(slice) {
            crate::boot::serial_println(s);
        }
    }
    
    buffer_size as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_syscall_dispatch() {
        // Test invalid syscall
        let result = dispatch(999, &[]);
        assert_eq!(result, SyscallResult::NotSupported as i64);
        
        // Test task_self
        let result = dispatch(71, &[]);
        assert_eq!(result, 0); // Kernel task
    }
    
    #[test]
    fn test_port_allocate() {
        // Would need proper task setup
        let args = [0, 1]; // Task 0, send right
        let result = sys_port_allocate(&args);
        // Should fail with invalid task in test environment
        assert!(result < 0);
    }
}