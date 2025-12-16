//! Host Abstraction
//!
//! Based on Mach4 kern/host.h and mach/host_info.h
//!
//! The host represents the machine itself. It provides:
//! - Host ports (normal and privileged)
//! - Host information (CPU count, memory size, etc.)
//! - Processor set management
//! - System-wide statistics

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::kern::processor::{ProcessorId, ProcessorSetId};

// ============================================================================
// Host Information Types
// ============================================================================

/// Host info flavor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HostInfoFlavor {
    /// Basic host info
    Basic = 1,
    /// Processor slot numbers
    ProcessorSlots = 2,
    /// Scheduling info
    SchedInfo = 3,
    /// Load average info
    LoadInfo = 4,
}

impl HostInfoFlavor {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            1 => Some(Self::Basic),
            2 => Some(Self::ProcessorSlots),
            3 => Some(Self::SchedInfo),
            4 => Some(Self::LoadInfo),
            _ => None,
        }
    }
}

// ============================================================================
// CPU Type and Subtype
// ============================================================================

/// CPU type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuType(pub i32);

impl CpuType {
    pub const ANY: Self = Self(-1);
    pub const VAX: Self = Self(1);
    pub const MC680X0: Self = Self(6);
    pub const X86: Self = Self(7);
    pub const I386: Self = Self(7);
    pub const X86_64: Self = Self(0x1000007);
    pub const MIPS: Self = Self(8);
    pub const MC98000: Self = Self(10);
    pub const HPPA: Self = Self(11);
    pub const ARM: Self = Self(12);
    pub const ARM64: Self = Self(0x100000C);
    pub const MC88000: Self = Self(13);
    pub const SPARC: Self = Self(14);
    pub const I860: Self = Self(15);
    pub const ALPHA: Self = Self(16);
    pub const POWERPC: Self = Self(18);
    pub const POWERPC64: Self = Self(0x1000012);
    pub const RISCV: Self = Self(24);

    pub fn name(&self) -> &'static str {
        match self.0 {
            -1 => "any",
            1 => "vax",
            6 => "mc680x0",
            7 => "x86",
            0x1000007 => "x86_64",
            8 => "mips",
            10 => "mc98000",
            11 => "hppa",
            12 => "arm",
            0x100000C => "arm64",
            13 => "mc88000",
            14 => "sparc",
            15 => "i860",
            16 => "alpha",
            18 => "powerpc",
            0x1000012 => "powerpc64",
            24 => "riscv",
            _ => "unknown",
        }
    }

    /// Is this a 64-bit CPU type?
    pub fn is_64bit(&self) -> bool {
        (self.0 & 0x1000000) != 0
    }
}

/// CPU subtype
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuSubtype(pub i32);

impl CpuSubtype {
    pub const MULTIPLE: Self = Self(-1);
    pub const LITTLE_ENDIAN: Self = Self(0);
    pub const BIG_ENDIAN: Self = Self(1);

    // x86 subtypes
    pub const X86_ALL: Self = Self(3);
    pub const X86_64_ALL: Self = Self(3);
    pub const X86_ARCH1: Self = Self(4);
    pub const X86_64_H: Self = Self(8);

    // ARM subtypes
    pub const ARM_ALL: Self = Self(0);
    pub const ARM_V7: Self = Self(9);
    pub const ARM_V7S: Self = Self(11);
    pub const ARM_V7K: Self = Self(12);
    pub const ARM64_ALL: Self = Self(0);
    pub const ARM64_V8: Self = Self(1);
    pub const ARM64E: Self = Self(2);
}

// ============================================================================
// Host Basic Info
// ============================================================================

/// Basic host information
#[derive(Debug, Clone, Default)]
pub struct HostBasicInfo {
    /// Maximum number of CPUs possible
    pub max_cpus: u32,
    /// Number of CPUs currently available
    pub avail_cpus: u32,
    /// Total memory size in bytes
    pub memory_size: u64,
    /// CPU type
    pub cpu_type: CpuType,
    /// CPU subtype
    pub cpu_subtype: CpuSubtype,
}

impl HostBasicInfo {
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Host Scheduling Info
// ============================================================================

/// Scheduling information
#[derive(Debug, Clone)]
pub struct HostSchedInfo {
    /// Minimum timeout in milliseconds
    pub min_timeout: u32,
    /// Minimum quantum in milliseconds
    pub min_quantum: u32,
}

impl Default for HostSchedInfo {
    fn default() -> Self {
        Self {
            min_timeout: 10, // 10ms minimum timeout
            min_quantum: 10, // 10ms minimum quantum
        }
    }
}

impl HostSchedInfo {
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Host Load Info
// ============================================================================

/// Load scale factor (fixed point 8.8)
pub const LOAD_SCALE: u32 = 1000;

/// Load average information
#[derive(Debug, Clone, Default)]
pub struct HostLoadInfo {
    /// Load averages (1, 5, 15 minutes) scaled by LOAD_SCALE
    pub avenrun: [u32; 3],
    /// Mach factors (1, 5, 15 minutes) scaled by LOAD_SCALE
    pub mach_factor: [u32; 3],
}

impl HostLoadInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get 1-minute load average as float
    pub fn load_avg_1(&self) -> f32 {
        self.avenrun[0] as f32 / LOAD_SCALE as f32
    }

    /// Get 5-minute load average as float
    pub fn load_avg_5(&self) -> f32 {
        self.avenrun[1] as f32 / LOAD_SCALE as f32
    }

    /// Get 15-minute load average as float
    pub fn load_avg_15(&self) -> f32 {
        self.avenrun[2] as f32 / LOAD_SCALE as f32
    }
}

// ============================================================================
// Kernel Version
// ============================================================================

/// Maximum kernel version string length
pub const KERNEL_VERSION_MAX: usize = 512;

/// Maximum boot info string length
pub const KERNEL_BOOT_INFO_MAX: usize = 4096;

// ============================================================================
// Host Statistics
// ============================================================================

/// Host-wide statistics
#[derive(Debug, Default)]
pub struct HostStats {
    /// Total tasks created
    pub tasks_created: AtomicU64,
    /// Total threads created
    pub threads_created: AtomicU64,
    /// Total IPC messages sent
    pub messages_sent: AtomicU64,
    /// Total IPC messages received
    pub messages_received: AtomicU64,
    /// Total page faults
    pub page_faults: AtomicU64,
    /// Total COW faults
    pub cow_faults: AtomicU64,
    /// Total page-ins
    pub pageins: AtomicU64,
    /// Total page-outs
    pub pageouts: AtomicU64,
    /// System uptime in seconds
    pub uptime_seconds: AtomicU64,
}

impl HostStats {
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Host Ports
// ============================================================================

/// Host ports
#[derive(Debug, Default)]
pub struct HostPorts {
    /// Host self port (normal access)
    pub host_self: Mutex<Option<PortName>>,
    /// Host privileged port (privileged operations)
    pub host_priv_self: Mutex<Option<PortName>>,
}

impl HostPorts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set host self port
    pub fn set_self_port(&self, port: PortName) {
        *self.host_self.lock() = Some(port);
    }

    /// Set host privileged port
    pub fn set_priv_port(&self, port: PortName) {
        *self.host_priv_self.lock() = Some(port);
    }

    /// Get host self port
    pub fn get_self_port(&self) -> Option<PortName> {
        *self.host_self.lock()
    }

    /// Get host privileged port
    pub fn get_priv_port(&self) -> Option<PortName> {
        *self.host_priv_self.lock()
    }
}

// ============================================================================
// Host Structure
// ============================================================================

/// The host structure represents the machine
#[derive(Debug)]
pub struct Host {
    /// Host ports
    pub ports: HostPorts,

    /// Basic info
    pub basic_info: Mutex<HostBasicInfo>,

    /// Scheduling info
    pub sched_info: HostSchedInfo,

    /// Load info
    pub load_info: Mutex<HostLoadInfo>,

    /// Kernel version string
    pub kernel_version: Mutex<String>,

    /// Boot info string
    pub boot_info: Mutex<String>,

    /// Host statistics
    pub stats: HostStats,

    /// Default processor set
    pub default_pset: Mutex<Option<ProcessorSetId>>,

    /// All processor sets
    pub processor_sets: Mutex<Vec<ProcessorSetId>>,

    /// Online processors
    pub processors: Mutex<Vec<ProcessorId>>,
}

impl Host {
    /// Create a new host
    pub fn new() -> Self {
        Self {
            ports: HostPorts::new(),
            basic_info: Mutex::new(HostBasicInfo::new()),
            sched_info: HostSchedInfo::default(),
            load_info: Mutex::new(HostLoadInfo::new()),
            kernel_version: Mutex::new(String::from("Mach_R 0.1.0")),
            boot_info: Mutex::new(String::new()),
            stats: HostStats::new(),
            default_pset: Mutex::new(None),
            processor_sets: Mutex::new(Vec::new()),
            processors: Mutex::new(Vec::new()),
        }
    }

    // === Basic info operations ===

    /// Get basic host info
    pub fn get_basic_info(&self) -> HostBasicInfo {
        self.basic_info.lock().clone()
    }

    /// Set maximum CPUs
    pub fn set_max_cpus(&self, max_cpus: u32) {
        self.basic_info.lock().max_cpus = max_cpus;
    }

    /// Set available CPUs
    pub fn set_avail_cpus(&self, avail_cpus: u32) {
        self.basic_info.lock().avail_cpus = avail_cpus;
    }

    /// Set memory size
    pub fn set_memory_size(&self, memory_size: u64) {
        self.basic_info.lock().memory_size = memory_size;
    }

    /// Set CPU type
    pub fn set_cpu_type(&self, cpu_type: CpuType, cpu_subtype: CpuSubtype) {
        let mut info = self.basic_info.lock();
        info.cpu_type = cpu_type;
        info.cpu_subtype = cpu_subtype;
    }

    // === Scheduling info ===

    /// Get scheduling info
    pub fn get_sched_info(&self) -> HostSchedInfo {
        self.sched_info.clone()
    }

    // === Load info ===

    /// Get load info
    pub fn get_load_info(&self) -> HostLoadInfo {
        self.load_info.lock().clone()
    }

    /// Update load averages
    pub fn update_load(&self, load_1: u32, load_5: u32, load_15: u32) {
        let mut info = self.load_info.lock();
        info.avenrun[0] = load_1;
        info.avenrun[1] = load_5;
        info.avenrun[2] = load_15;
    }

    /// Update mach factors
    pub fn update_mach_factor(&self, mf_1: u32, mf_5: u32, mf_15: u32) {
        let mut info = self.load_info.lock();
        info.mach_factor[0] = mf_1;
        info.mach_factor[1] = mf_5;
        info.mach_factor[2] = mf_15;
    }

    // === Version info ===

    /// Get kernel version
    pub fn get_kernel_version(&self) -> String {
        self.kernel_version.lock().clone()
    }

    /// Set kernel version
    pub fn set_kernel_version(&self, version: &str) {
        let mut ver = self.kernel_version.lock();
        ver.clear();
        ver.push_str(version);
    }

    /// Get boot info
    pub fn get_boot_info(&self) -> String {
        self.boot_info.lock().clone()
    }

    /// Set boot info
    pub fn set_boot_info(&self, info: &str) {
        let mut boot = self.boot_info.lock();
        boot.clear();
        boot.push_str(info);
    }

    // === Processor management ===

    /// Set default processor set
    pub fn set_default_pset(&self, pset: ProcessorSetId) {
        *self.default_pset.lock() = Some(pset);
    }

    /// Get default processor set
    pub fn get_default_pset(&self) -> Option<ProcessorSetId> {
        *self.default_pset.lock()
    }

    /// Add processor set
    pub fn add_processor_set(&self, pset: ProcessorSetId) {
        self.processor_sets.lock().push(pset);
    }

    /// Remove processor set
    pub fn remove_processor_set(&self, pset: ProcessorSetId) {
        self.processor_sets.lock().retain(|&p| p != pset);
    }

    /// Get all processor sets
    pub fn get_processor_sets(&self) -> Vec<ProcessorSetId> {
        self.processor_sets.lock().clone()
    }

    /// Add processor
    pub fn add_processor(&self, processor: ProcessorId) {
        let mut procs = self.processors.lock();
        if !procs.contains(&processor) {
            procs.push(processor);
        }
        // Update available CPU count
        self.basic_info.lock().avail_cpus = procs.len() as u32;
    }

    /// Remove processor
    pub fn remove_processor(&self, processor: ProcessorId) {
        let mut procs = self.processors.lock();
        procs.retain(|&p| p != processor);
        // Update available CPU count
        self.basic_info.lock().avail_cpus = procs.len() as u32;
    }

    /// Get processor slots (IDs of online processors)
    pub fn get_processor_slots(&self) -> Vec<ProcessorId> {
        self.processors.lock().clone()
    }

    // === Statistics ===

    /// Record task creation
    pub fn record_task_created(&self) {
        self.stats.tasks_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record thread creation
    pub fn record_thread_created(&self) {
        self.stats.threads_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record message sent
    pub fn record_message_sent(&self) {
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record message received
    pub fn record_message_received(&self) {
        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record page fault
    pub fn record_page_fault(&self) {
        self.stats.page_faults.fetch_add(1, Ordering::Relaxed);
    }

    /// Record COW fault
    pub fn record_cow_fault(&self) {
        self.stats.cow_faults.fetch_add(1, Ordering::Relaxed);
    }

    /// Record page-in
    pub fn record_pagein(&self) {
        self.stats.pageins.fetch_add(1, Ordering::Relaxed);
    }

    /// Record page-out
    pub fn record_pageout(&self) {
        self.stats.pageouts.fetch_add(1, Ordering::Relaxed);
    }

    /// Update uptime
    pub fn set_uptime(&self, seconds: u64) {
        self.stats.uptime_seconds.store(seconds, Ordering::Relaxed);
    }

    /// Get uptime
    pub fn get_uptime(&self) -> u64 {
        self.stats.uptime_seconds.load(Ordering::Relaxed)
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

/// The real host structure (singleton)
static REALHOST: spin::Once<Host> = spin::Once::new();

/// Get the real host
fn realhost() -> &'static Host {
    REALHOST.call_once(|| {
        let host = Host::new();

        // Set default CPU info based on build target
        #[cfg(target_arch = "x86_64")]
        {
            let mut info = host.basic_info.lock();
            info.cpu_type = CpuType::X86_64;
            info.cpu_subtype = CpuSubtype::X86_64_ALL;
        }

        #[cfg(target_arch = "aarch64")]
        {
            let mut info = host.basic_info.lock();
            info.cpu_type = CpuType::ARM64;
            info.cpu_subtype = CpuSubtype::ARM64_ALL;
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            // Default for unknown architectures
        }

        host
    });
    REALHOST.get().unwrap()
}

/// Initialize host subsystem
pub fn init() {
    let _ = realhost();
}

// ============================================================================
// Public API
// ============================================================================

/// Get host self port
pub fn host_self() -> Option<PortName> {
    realhost().ports.get_self_port()
}

/// Get host privileged port
pub fn host_priv_self() -> Option<PortName> {
    realhost().ports.get_priv_port()
}

/// Get host basic info
pub fn host_info_basic() -> HostBasicInfo {
    realhost().get_basic_info()
}

/// Get host scheduling info
pub fn host_info_sched() -> HostSchedInfo {
    realhost().get_sched_info()
}

/// Get host load info
pub fn host_info_load() -> HostLoadInfo {
    realhost().get_load_info()
}

/// Get processor slots
pub fn host_processor_slots() -> Vec<ProcessorId> {
    realhost().get_processor_slots()
}

/// Get kernel version
pub fn host_kernel_version() -> String {
    realhost().get_kernel_version()
}

/// Get boot info
pub fn host_boot_info() -> String {
    realhost().get_boot_info()
}

/// Get default processor set
pub fn host_default_pset() -> Option<ProcessorSetId> {
    realhost().get_default_pset()
}

/// Record task creation
pub fn host_record_task_created() {
    realhost().record_task_created();
}

/// Record thread creation
pub fn host_record_thread_created() {
    realhost().record_thread_created();
}

/// Configure host
pub fn host_configure(max_cpus: u32, memory_size: u64) {
    let host = realhost();
    host.set_max_cpus(max_cpus);
    host.set_memory_size(memory_size);
}

/// Set host ports
pub fn host_set_ports(self_port: PortName, priv_port: PortName) {
    let host = realhost();
    host.ports.set_self_port(self_port);
    host.ports.set_priv_port(priv_port);
}

/// Add processor to host
pub fn host_add_processor(processor: ProcessorId) {
    realhost().add_processor(processor);
}

/// Remove processor from host
pub fn host_remove_processor(processor: ProcessorId) {
    realhost().remove_processor(processor);
}

/// Get host uptime
pub fn host_uptime() -> u64 {
    realhost().get_uptime()
}

/// Set host uptime
pub fn host_set_uptime(seconds: u64) {
    realhost().set_uptime(seconds);
}

// ============================================================================
// Conversion Functions (for IPC)
// ============================================================================

/// Convert port to host (validates port is host port)
pub fn convert_port_to_host(port: PortName) -> Option<&'static Host> {
    let host = realhost();
    if host.ports.get_self_port() == Some(port) {
        Some(host)
    } else {
        None
    }
}

/// Convert port to host_priv (validates port is privileged host port)
pub fn convert_port_to_host_priv(port: PortName) -> Option<&'static Host> {
    let host = realhost();
    if host.ports.get_priv_port() == Some(port) {
        Some(host)
    } else {
        None
    }
}

/// Convert host to port
pub fn convert_host_to_port(_host: &Host) -> Option<PortName> {
    realhost().ports.get_self_port()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_type() {
        assert_eq!(CpuType::X86_64.name(), "x86_64");
        assert!(CpuType::X86_64.is_64bit());
        assert!(!CpuType::X86.is_64bit());
        assert!(CpuType::ARM64.is_64bit());
        assert!(!CpuType::ARM.is_64bit());
    }

    #[test]
    fn test_host_basic_info() {
        let host = Host::new();

        host.set_max_cpus(8);
        host.set_avail_cpus(4);
        host.set_memory_size(16 * 1024 * 1024 * 1024);
        host.set_cpu_type(CpuType::X86_64, CpuSubtype::X86_64_ALL);

        let info = host.get_basic_info();
        assert_eq!(info.max_cpus, 8);
        assert_eq!(info.avail_cpus, 4);
        assert_eq!(info.memory_size, 16 * 1024 * 1024 * 1024);
        assert_eq!(info.cpu_type, CpuType::X86_64);
    }

    #[test]
    fn test_host_load_info() {
        let host = Host::new();

        host.update_load(1000, 500, 250);
        let info = host.get_load_info();

        assert!((info.load_avg_1() - 1.0).abs() < 0.001);
        assert!((info.load_avg_5() - 0.5).abs() < 0.001);
        assert!((info.load_avg_15() - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_host_processors() {
        let host = Host::new();

        host.add_processor(ProcessorId(0));
        host.add_processor(ProcessorId(1));

        let slots = host.get_processor_slots();
        assert_eq!(slots.len(), 2);

        let info = host.get_basic_info();
        assert_eq!(info.avail_cpus, 2);

        host.remove_processor(ProcessorId(0));
        let info = host.get_basic_info();
        assert_eq!(info.avail_cpus, 1);
    }

    #[test]
    fn test_host_stats() {
        let host = Host::new();

        host.record_task_created();
        host.record_task_created();
        host.record_thread_created();

        assert_eq!(host.stats.tasks_created.load(Ordering::Relaxed), 2);
        assert_eq!(host.stats.threads_created.load(Ordering::Relaxed), 1);
    }
}
