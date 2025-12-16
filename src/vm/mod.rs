//! Virtual Machine integration module
//! Integrates pure Rust VM implementations including EVM

use heapless::{String, Vec};

pub mod evm_engine;

/// VM execution context
#[derive(Debug)]
pub struct VmContext {
    /// VM identifier
    pub id: u32,
    /// VM type
    pub vm_type: VmType,
    /// Memory limit in bytes
    pub memory_limit: usize,
    /// CPU time limit in milliseconds
    pub cpu_limit: u32,
    /// VM state
    pub state: VmState,
}

/// Supported VM types
#[derive(Debug, Clone, Copy)]
pub enum VmType {
    Ethereum,
    // Future VM types can be added here
}

/// VM execution state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmState {
    Created,
    Running,
    Suspended,
    Stopped,
    Error,
}

/// VM execution result
#[derive(Debug)]
pub struct VmResult {
    /// Exit code
    pub exit_code: i32,
    /// Gas used (for EVM)
    pub gas_used: u64,
    /// Output data
    pub output: Vec<u8, 1024>,
    /// Error message if any
    pub error: Option<String<256>>,
}

/// Virtual Machine Manager
pub struct VmManager {
    /// Active VMs
    vms: Vec<VmContext, 16>,
    /// Next VM ID
    next_id: u32,
    /// Initialized flag
    initialized: bool,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new() -> Self {
        Self {
            vms: Vec::new(),
            next_id: 1,
            initialized: false,
        }
    }

    /// Initialize the VM manager
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Ok(());
        }

        // Initialize EVM engine
        evm_engine::init()?;

        self.initialized = true;
        Ok(())
    }

    /// Create a new VM instance
    pub fn create_vm(
        &mut self,
        vm_type: VmType,
        memory_limit: usize,
        cpu_limit: u32,
    ) -> Result<u32, &'static str> {
        if !self.initialized {
            return Err("VM manager not initialized");
        }

        let id = self.next_id;
        self.next_id += 1;

        let context = VmContext {
            id,
            vm_type,
            memory_limit,
            cpu_limit,
            state: VmState::Created,
        };

        self.vms.push(context).map_err(|_| "Too many VMs")?;
        Ok(id)
    }

    /// Execute code in a VM
    pub fn execute(
        &mut self,
        vm_id: u32,
        code: &[u8],
        data: &[u8],
    ) -> Result<VmResult, &'static str> {
        let vm = self.find_vm_mut(vm_id)?;

        match vm.vm_type {
            VmType::Ethereum => {
                vm.state = VmState::Running;
                let result = evm_engine::execute(code, data)?;
                vm.state = VmState::Stopped;
                Ok(result)
            }
        }
    }

    /// Find a VM by ID
    fn find_vm_mut(&mut self, id: u32) -> Result<&mut VmContext, &'static str> {
        self.vms
            .iter_mut()
            .find(|vm| vm.id == id)
            .ok_or("VM not found")
    }

    /// Get VM list
    pub fn list_vms(&self) -> &Vec<VmContext, 16> {
        &self.vms
    }

    /// Stop a VM
    pub fn stop_vm(&mut self, vm_id: u32) -> Result<(), &'static str> {
        let vm = self.find_vm_mut(vm_id)?;
        vm.state = VmState::Stopped;
        Ok(())
    }

    /// Remove a stopped VM
    pub fn remove_vm(&mut self, vm_id: u32) -> Result<(), &'static str> {
        let pos = self
            .vms
            .iter()
            .position(|vm| vm.id == vm_id)
            .ok_or("VM not found")?;

        if self.vms[pos].state != VmState::Stopped {
            return Err("VM must be stopped before removal");
        }

        self.vms.swap_remove(pos);
        Ok(())
    }
}

static mut VM_MANAGER: Option<VmManager> = None;

/// Initialize the VM subsystem
pub fn init() -> Result<(), &'static str> {
    let mut manager = VmManager::new();
    manager.init()?;

    unsafe {
        VM_MANAGER = Some(manager);
    }

    Ok(())
}

/// Get the global VM manager
pub fn get_manager() -> Option<&'static mut VmManager> {
    unsafe { (*core::ptr::addr_of_mut!(VM_MANAGER)).as_mut() }
}
