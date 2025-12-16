//! Kern subsystem - Core kernel primitives
//!
//! Based on Mach4 kern/ directory
//! Contains processor management, scheduling primitives, and kernel services.

pub mod activation;
pub mod ast;
pub mod continuation;
pub mod copyio;
pub mod counters;
pub mod elf_loader;
pub mod exception;
pub mod host;
pub mod ipc_kobject;
pub mod kalloc;
pub mod lock;
pub mod priority;
pub mod processor;
pub mod queue;
pub mod runq;
pub mod sched_prim;
pub mod startup;
pub mod strings;
pub mod syscall_sw;
pub mod task;
pub mod thread;
pub mod thread_swap;
pub mod timer;
pub mod zalloc;

pub use activation::{act_attach, act_create, act_detach, Activation, ActivationId, Shuttle};
pub use copyio::{copyin, copyinstr, copyout, copyoutstr, CopyError, CopyResult};
pub use counters::{context_switch, thread_created, thread_destroyed, CounterSnapshot};
pub use elf_loader::{load_elf, validate_elf, ElfError, ElfLoader, LoadedBinary};
pub use exception::{exception_raise, ExceptionMask, ExceptionType};
pub use host::{host_priv_self, host_self, CpuSubtype, CpuType, Host};
pub use ipc_kobject::{ipc_kobject_destroy, ipc_kobject_set, Kobject, KobjectType};
pub use kalloc::{kalloc, kalloc_init, kalloc_zeroed, kfree, kget};
pub use lock::{RwLock, SimpleLock, SpinLock};
pub use processor::{Processor, ProcessorSet, ProcessorState};
pub use queue::{MpQueueHead, QueueChain, QueueHead};
pub use sched_prim::{WaitEvent, WaitResult};
pub use startup::{get_startup_phase, kernel_ready, setup_main, MachineInfo, StartupPhase};
pub use syscall_sw::{mach_trap, trap_stats, KernReturn, MachTrap, TrapArgs, KERN_SUCCESS};
pub use task::{kernel_task, task_create, task_find, Task};
pub use thread::TaskId;
pub use thread::{thread_create, thread_find, Thread, ThreadId, ThreadState};
pub use thread_swap::{swapper_init, thread_doswapin, thread_swapin, SwapStats};
pub use zalloc::{zalloc, zcram, zfree, zget, zinit, Zone, ZoneId, ZoneType};

/// Initialize the kern subsystem
pub fn init() {
    processor::init();
    timer::init();
    zalloc::zone_bootstrap();
    kalloc::kalloc_init();
    task::init();
    thread::init();
    exception::init();
    host::init();
    ipc_kobject::init();
    syscall_sw::init();
    startup::init();
    counters::init();
    thread_swap::init();
    queue::init();
}
