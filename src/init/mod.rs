//! Init system implementation for Mach_R
//! Pure Rust init system inspired by systemd and modern init systems

use heapless::{FnvIndexMap, String, Vec};

pub mod process;
pub mod service;
pub mod supervisor;

/// Maximum number of services
const MAX_SERVICES: usize = 64;
/// Maximum number of running processes
const MAX_PROCESSES: usize = 128;
/// Maximum service name length
const MAX_SERVICE_NAME: usize = 64;

/// Task ID for the init process
pub const INIT_TASK_ID: u32 = 1;

/// Service state enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceState {
    /// Service is stopped
    Stopped,
    /// Service is starting
    Starting,
    /// Service is running
    Running,
    /// Service is stopping
    Stopping,
    /// Service failed to start or crashed
    Failed,
    /// Service is being restarted
    Restarting,
}

/// Service type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceType {
    /// Simple service - single process
    Simple,
    /// Forking service - forks child processes
    Forking,
    /// Oneshot service - runs once and exits
    Oneshot,
    /// Notify service - sends readiness notification
    Notify,
}

/// Service configuration
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Service name
    pub name: String<MAX_SERVICE_NAME>,
    /// Service type
    pub service_type: ServiceType,
    /// Command to execute
    pub exec_start: String<256>,
    /// Command to stop service
    pub exec_stop: Option<String<256>>,
    /// Working directory
    pub working_directory: String<256>,
    /// Environment variables
    pub environment: FnvIndexMap<String<32>, String<128>, 16>,
    /// User to run as
    pub user: String<32>,
    /// Group to run as
    pub group: String<32>,
    /// Dependencies (services that must start first)
    pub requires: Vec<String<MAX_SERVICE_NAME>, 8>,
    /// Soft dependencies (services that should start first if available)
    pub wants: Vec<String<MAX_SERVICE_NAME>, 8>,
    /// Services that must start after this one
    pub before: Vec<String<MAX_SERVICE_NAME>, 8>,
    /// Services that must start before this one
    pub after: Vec<String<MAX_SERVICE_NAME>, 8>,
    /// Restart policy
    pub restart: RestartPolicy,
    /// Restart delay in milliseconds
    pub restart_delay_ms: u32,
    /// Service enabled for auto-start
    pub enabled: bool,
}

impl ServiceConfig {
    /// Create a new service configuration
    pub fn new(name: &str) -> Result<Self, &'static str> {
        let mut service_name = String::new();
        service_name
            .push_str(name)
            .map_err(|_| "Service name too long")?;

        Ok(Self {
            name: service_name,
            service_type: ServiceType::Simple,
            exec_start: String::new(),
            exec_stop: None,
            working_directory: String::new(),
            environment: FnvIndexMap::new(),
            user: String::new(),
            group: String::new(),
            requires: Vec::new(),
            wants: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            restart: RestartPolicy::OnFailure,
            restart_delay_ms: 1000,
            enabled: false,
        })
    }

    /// Set the command to execute
    pub fn set_exec_start(&mut self, command: &str) -> Result<(), &'static str> {
        self.exec_start.clear();
        self.exec_start
            .push_str(command)
            .map_err(|_| "Command too long")?;
        Ok(())
    }

    /// Add a dependency
    pub fn add_requires(&mut self, service: &str) -> Result<(), &'static str> {
        let mut dep = String::new();
        dep.push_str(service)
            .map_err(|_| "Dependency name too long")?;
        self.requires
            .push(dep)
            .map_err(|_| "Too many dependencies")?;
        Ok(())
    }

    /// Add environment variable
    pub fn add_env(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        let mut env_key = String::new();
        env_key
            .push_str(key)
            .map_err(|_| "Environment key too long")?;

        let mut env_value = String::new();
        env_value
            .push_str(value)
            .map_err(|_| "Environment value too long")?;

        self.environment
            .insert(env_key, env_value)
            .map_err(|_| "Too many environment variables")?;
        Ok(())
    }
}

/// Restart policy for services
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestartPolicy {
    /// Never restart
    Never,
    /// Always restart
    Always,
    /// Restart on failure only
    OnFailure,
    /// Restart on abnormal exit
    OnAbnormal,
}

/// Service runtime information
#[derive(Debug)]
pub struct ServiceRuntime {
    /// Service configuration
    pub config: ServiceConfig,
    /// Current state
    pub state: ServiceState,
    /// Process ID (if running)
    pub pid: Option<u32>,
    /// Start time
    pub start_time: u64,
    /// Restart count
    pub restart_count: u32,
    /// Last exit code
    pub last_exit_code: Option<i32>,
}

impl ServiceRuntime {
    /// Create new service runtime
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            config,
            state: ServiceState::Stopped,
            pid: None,
            start_time: 0,
            restart_count: 0,
            last_exit_code: None,
        }
    }

    /// Check if service needs restart based on policy
    pub fn should_restart(&self, exit_code: i32) -> bool {
        match self.config.restart {
            RestartPolicy::Never => false,
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure => exit_code != 0,
            RestartPolicy::OnAbnormal => exit_code != 0, // TODO: more sophisticated logic
        }
    }
}

/// Init system manager
pub struct InitSystem {
    /// All registered services
    services: Vec<ServiceRuntime, MAX_SERVICES>,
    /// Service name to index mapping
    service_index: FnvIndexMap<String<MAX_SERVICE_NAME>, usize, MAX_SERVICES>,
    /// System state
    system_state: SystemState,
    /// Boot timestamp
    boot_time: u64,
    /// Process supervisor
    supervisor: supervisor::ProcessSupervisor,
}

/// System state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemState {
    /// System is booting
    Booting,
    /// System is running normally
    Running,
    /// System is shutting down
    Shutdown,
    /// System is in maintenance mode
    Maintenance,
}

impl InitSystem {
    /// Create a new init system
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
            service_index: FnvIndexMap::new(),
            system_state: SystemState::Booting,
            boot_time: 0, // TODO: get actual timestamp
            supervisor: supervisor::ProcessSupervisor::new(),
        }
    }

    /// Register a service
    pub fn register_service(&mut self, config: ServiceConfig) -> Result<(), &'static str> {
        let service_name = config.name.clone();
        let runtime = ServiceRuntime::new(config);

        let index = self.services.len();
        self.services
            .push(runtime)
            .map_err(|_| "Too many services")?;
        self.service_index
            .insert(service_name, index)
            .map_err(|_| "Service index full")?;

        Ok(())
    }

    /// Start a service
    pub fn start_service(&mut self, name: &str) -> Result<(), &'static str> {
        let index = self.find_service_index(name)?;

        // Check dependencies first
        self.start_dependencies(index)?;

        let service = &mut self.services[index];
        if service.state == ServiceState::Running {
            return Ok(()); // Already running
        }

        service.state = ServiceState::Starting;

        // Start the actual process
        match self
            .supervisor
            .start_process(&service.config.exec_start, &service.config.environment)
        {
            Ok(pid) => {
                service.pid = Some(pid);
                service.state = ServiceState::Running;
                service.start_time = get_current_time();
                Ok(())
            }
            Err(e) => {
                service.state = ServiceState::Failed;
                Err(e)
            }
        }
    }

    /// Stop a service
    pub fn stop_service(&mut self, name: &str) -> Result<(), &'static str> {
        let index = self.find_service_index(name)?;
        let service = &mut self.services[index];

        if service.state != ServiceState::Running {
            return Ok(()); // Not running
        }

        service.state = ServiceState::Stopping;

        if let Some(pid) = service.pid {
            // Send termination signal
            self.supervisor.stop_process(pid)?;
            service.pid = None;
        }

        service.state = ServiceState::Stopped;
        Ok(())
    }

    /// Restart a service
    pub fn restart_service(&mut self, name: &str) -> Result<(), &'static str> {
        let index = self.find_service_index(name)?;
        self.services[index].state = ServiceState::Restarting;

        self.stop_service(name)?;
        self.start_service(name)?;

        self.services[index].restart_count += 1;
        Ok(())
    }

    /// Start dependencies for a service
    fn start_dependencies(&mut self, service_index: usize) -> Result<(), &'static str> {
        // Get list of required services
        let mut required_services = Vec::<String<MAX_SERVICE_NAME>, 8>::new();
        for dep in &self.services[service_index].config.requires {
            required_services
                .push(dep.clone())
                .map_err(|_| "Too many dependencies")?;
        }

        // Start each required service
        for dep_name in &required_services {
            self.start_service(dep_name)?;
        }

        Ok(())
    }

    /// Find service index by name
    fn find_service_index(&self, name: &str) -> Result<usize, &'static str> {
        // Convert str to String for lookup
        let mut service_name: String<MAX_SERVICE_NAME> = String::new();
        service_name
            .push_str(name)
            .map_err(|_| "Service name too long")?;
        self.service_index
            .get(&service_name)
            .copied()
            .ok_or("Service not found")
    }

    /// Get service status
    pub fn get_service_status(&self, name: &str) -> Result<&ServiceRuntime, &'static str> {
        let index = self.find_service_index(name)?;
        Ok(&self.services[index])
    }

    /// List all services
    pub fn list_services(&self) -> &Vec<ServiceRuntime, MAX_SERVICES> {
        &self.services
    }

    /// Enable service for auto-start
    pub fn enable_service(&mut self, name: &str) -> Result<(), &'static str> {
        let index = self.find_service_index(name)?;
        self.services[index].config.enabled = true;
        Ok(())
    }

    /// Disable service auto-start
    pub fn disable_service(&mut self, name: &str) -> Result<(), &'static str> {
        let index = self.find_service_index(name)?;
        self.services[index].config.enabled = false;
        Ok(())
    }

    /// Boot system - start all enabled services
    pub fn boot_system(&mut self) -> Result<(), &'static str> {
        self.system_state = SystemState::Booting;
        self.boot_time = get_current_time();

        // Start enabled services in dependency order
        // TODO: implement proper dependency resolution
        for i in 0..self.services.len() {
            if self.services[i].config.enabled {
                let service_name = self.services[i].config.name.clone();
                self.start_service(&service_name)?;
            }
        }

        self.system_state = SystemState::Running;
        Ok(())
    }

    /// Shutdown system
    pub fn shutdown_system(&mut self) -> Result<(), &'static str> {
        self.system_state = SystemState::Shutdown;

        // Stop all running services
        for i in 0..self.services.len() {
            if self.services[i].state == ServiceState::Running {
                let service_name = self.services[i].config.name.clone();
                self.stop_service(&service_name)?;
            }
        }

        Ok(())
    }

    /// Process management - called periodically to handle process events
    pub fn process_events(&mut self) -> Result<(), &'static str> {
        // Check for dead processes and handle restarts
        self.supervisor.reap_children()?;

        // Handle service restarts
        for i in 0..self.services.len() {
            let service = &mut self.services[i];
            if service.state == ServiceState::Failed {
                if let Some(exit_code) = service.last_exit_code {
                    if service.should_restart(exit_code) {
                        let service_name = service.config.name.clone();
                        // TODO: implement delay
                        self.restart_service(&service_name)?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Get current system time in milliseconds
fn get_current_time() -> u64 {
    // TODO: implement actual timestamp
    0
}

static mut INIT_SYSTEM: Option<InitSystem> = None;

/// Initialize the init system
pub fn init() -> Result<(), &'static str> {
    let mut init_system = InitSystem::new();

    // Register default services
    register_default_services(&mut init_system)?;

    unsafe {
        INIT_SYSTEM = Some(init_system);
    }

    Ok(())
}

/// Register default system services
fn register_default_services(init_system: &mut InitSystem) -> Result<(), &'static str> {
    // Shell service
    let mut shell_config = ServiceConfig::new("mach_r_shell")?;
    shell_config.set_exec_start("/bin/mach_r_shell")?;
    shell_config.service_type = ServiceType::Simple;
    shell_config.restart = RestartPolicy::Always;
    shell_config.enabled = true;
    init_system.register_service(shell_config)?;

    // SSH service
    let mut ssh_config = ServiceConfig::new("ssh")?;
    ssh_config.set_exec_start("/usr/sbin/sshd -D")?;
    ssh_config.service_type = ServiceType::Simple;
    ssh_config.restart = RestartPolicy::Always;
    ssh_config.enabled = true;
    init_system.register_service(ssh_config)?;

    // Network service
    let mut network_config = ServiceConfig::new("network")?;
    network_config.set_exec_start("/sbin/network_init")?;
    network_config.service_type = ServiceType::Oneshot;
    network_config.enabled = true;
    init_system.register_service(network_config)?;

    // Filesystem service
    let mut fs_config = ServiceConfig::new("filesystem")?;
    fs_config.set_exec_start("/sbin/mount_all")?;
    fs_config.service_type = ServiceType::Oneshot;
    fs_config.enabled = true;
    init_system.register_service(fs_config)?;

    Ok(())
}

/// Get the global init system
pub fn get_init_system() -> Option<&'static mut InitSystem> {
    unsafe { (*core::ptr::addr_of_mut!(INIT_SYSTEM)).as_mut() }
}

/// Start the init process - begins system initialization
pub fn start_init() -> ! {
    if let Some(init_system) = get_init_system() {
        // Boot the system services
        if let Err(e) = init_system.boot_system() {
            panic!("Failed to boot system: {}", e);
        }

        // Main init loop - process events and manage services
        loop {
            if let Err(e) = init_system.process_events() {
                // Log error but continue running
                crate::println!("Init system error: {}", e);
            }

            // Small delay to prevent busy loop
            core::hint::spin_loop();
        }
    } else {
        panic!("Init system not initialized!");
    }
}
