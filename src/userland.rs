//! Userland Applications Framework for Mach_R
//!
//! Provides the foundation for user-space applications running on Mach_R.
//! Includes a process manager, application loader, and runtime environment.

use crate::types::TaskId;
use crate::port::Port;
use crate::servers::{file_server, vm_server};
use crate::utilities::Utilities;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;

/// User process descriptor
#[derive(Debug, Clone)]
pub struct UserProcess {
    /// Process ID
    pub pid: TaskId,
    /// Process name
    pub name: String,
    /// Current state
    pub state: ProcessState,
    /// Parent process ID
    pub parent: Option<TaskId>,
    /// Child processes
    pub children: Vec<TaskId>,
    /// Virtual memory usage
    pub memory_usage: usize,
    /// Open file descriptors
    pub open_files: Vec<i32>,
    /// Environment variables
    pub environment: Vec<(String, String)>,
    /// Current working directory
    pub current_dir: String,
}

/// Process states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessState {
    /// Process is running
    Running,
    /// Process is sleeping/waiting
    Sleeping,
    /// Process is stopped
    Stopped,
    /// Process has terminated
    Zombie,
    /// Process terminated abnormally
    Crashed,
}

/// Application binary format
#[derive(Debug, Clone)]
pub struct ApplicationBinary {
    /// Binary name
    pub name: String,
    /// Entry point address
    pub entry_point: usize,
    /// Code segment
    pub code: Vec<u8>,
    /// Data segment
    pub data: Vec<u8>,
    /// Required permissions
    pub permissions: Vec<String>,
    /// Dependencies
    pub dependencies: Vec<String>,
}

/// User application runtime
pub struct UserRuntime {
    /// Associated process
    pub process: UserProcess,
    /// Utilities instance
    pub utilities: Utilities,
    /// Message ports
    pub ports: Vec<Arc<Port>>,
}

impl UserRuntime {
    /// Create new user runtime
    pub fn new(process: UserProcess) -> Self {
        let utilities = Utilities::new(process.pid);
        
        Self {
            process,
            utilities,
            ports: Vec::new(),
        }
    }

    /// Execute a command in this runtime
    pub fn execute_command(&mut self, command: &str) -> Result<i32, &'static str> {
        crate::println!("[PID {}] Executing: {}", self.process.pid.0, command);
        
        // Update process state
        self.process.state = ProcessState::Running;
        
        // Execute through utilities
        match self.utilities.execute_command(command) {
            Ok(()) => Ok(0),
            Err("exit") => {
                self.process.state = ProcessState::Zombie;
                Ok(0)
            },
            Err(_) => Ok(1), // Non-zero exit code
        }
    }

    /// Allocate memory for this process
    pub fn allocate_memory(&mut self, size: usize) -> Result<usize, &'static str> {
        let vm_server = vm_server::vm_server();
        
        match vm_server.vm_allocate(self.process.pid, size, 0x07) {
            Ok(addr) => {
                self.process.memory_usage += size;
                crate::println!("[PID {}] Allocated {} bytes at 0x{:x}", 
                    self.process.pid.0, size, addr);
                Ok(addr)
            },
            Err(_) => Err("Memory allocation failed")
        }
    }

    /// Open file for this process
    pub fn open_file(&mut self, path: &str) -> Result<i32, &'static str> {
        let file_server = file_server::file_server();
        
        match file_server.file_open(path.to_string(), 0, self.process.pid) {
            Ok(fd) => {
                self.process.open_files.push(fd);
                crate::println!("[PID {}] Opened file '{}' as fd {}", 
                    self.process.pid.0, path, fd);
                Ok(fd)
            },
            Err(_) => Err("File open failed")
        }
    }

    /// Change current directory
    pub fn change_directory(&mut self, path: &str) -> Result<(), &'static str> {
        // Validate directory exists through file server
        let file_server = file_server::file_server();
        
        // In full implementation, would check if path is a directory
        self.process.current_dir = path.to_string();
        crate::println!("[PID {}] Changed directory to '{}'", 
            self.process.pid.0, path);
        Ok(())
    }

    /// Set environment variable
    pub fn set_environment(&mut self, name: &str, value: &str) {
        // Remove existing entry if present
        self.process.environment.retain(|(k, _)| k != name);
        
        // Add new entry
        self.process.environment.push((name.to_string(), value.to_string()));
        crate::println!("[PID {}] Set {}={}", self.process.pid.0, name, value);
    }

    /// Get environment variable
    pub fn get_environment(&self, name: &str) -> Option<&str> {
        self.process.environment.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Process manager for userland applications
pub struct ProcessManager {
    /// All processes in the system
    processes: Mutex<Vec<UserProcess>>,
    /// Next available PID
    next_pid: Mutex<u64>,
}

impl ProcessManager {
    /// Create new process manager
    pub const fn new() -> Self {
        Self {
            processes: Mutex::new(Vec::new()),
            next_pid: Mutex::new(100), // Start user PIDs at 100
        }
    }

    /// Create a new user process
    pub fn create_process(&self, name: &str, parent: Option<TaskId>) -> UserProcess {
        let mut next_pid = self.next_pid.lock();
        let pid = TaskId(*next_pid);
        *next_pid += 1;

        let process = UserProcess {
            pid,
            name: name.to_string(),
            state: ProcessState::Running,
            parent,
            children: Vec::new(),
            memory_usage: 0,
            open_files: {
                let mut files = Vec::new();
                files.extend([0, 1, 2]); // stdin, stdout, stderr
                files
            },
            environment: {
                let mut env = Vec::new();
                env.push(("PATH".to_string(), "/bin:/usr/bin".to_string()));
                env.push(("HOME".to_string(), "/".to_string()));
                env.push(("SHELL".to_string(), "/bin/shell".to_string()));
                env.push(("USER".to_string(), "user".to_string()));
                env.push(("TERM".to_string(), "mach_r".to_string()));
                env
            },
            current_dir: "/".to_string(),
        };

        // Add to parent's children if parent exists
        if let Some(parent_id) = parent {
            let mut processes = self.processes.lock();
            if let Some(parent_proc) = processes.iter_mut().find(|p| p.pid == parent_id) {
                parent_proc.children.push(pid);
            }
        }

        // Add to process list
        self.processes.lock().push(process.clone());

        crate::println!("Created process '{}' with PID {}", name, pid.0);
        process
    }

    /// Find process by PID
    pub fn find_process(&self, pid: TaskId) -> Option<UserProcess> {
        let processes = self.processes.lock();
        processes.iter().find(|p| p.pid == pid).cloned()
    }

    /// List all processes
    pub fn list_processes(&self) -> Vec<UserProcess> {
        self.processes.lock().clone()
    }

    /// Terminate a process
    pub fn terminate_process(&self, pid: TaskId) -> Result<(), &'static str> {
        let mut processes = self.processes.lock();
        
        if let Some(process) = processes.iter_mut().find(|p| p.pid == pid) {
            process.state = ProcessState::Zombie;
            crate::println!("Terminated process {} ('{}')", pid.0, process.name);
            Ok(())
        } else {
            Err("Process not found")
        }
    }

    /// Clean up zombie processes
    pub fn reap_zombies(&self) {
        let mut processes = self.processes.lock();
        let initial_count = processes.len();
        
        processes.retain(|p| p.state != ProcessState::Zombie);
        
        let reaped = initial_count - processes.len();
        if reaped > 0 {
            crate::println!("Reaped {} zombie processes", reaped);
        }
    }
}

/// Application loader and executor
pub struct ApplicationLoader {
    /// Process manager
    process_manager: ProcessManager,
    /// Built-in applications
    builtin_apps: Mutex<Vec<ApplicationBinary>>,
}

impl ApplicationLoader {
    /// Create new application loader
    pub fn new() -> Self {
        let mut loader = Self {
            process_manager: ProcessManager::new(),
            builtin_apps: Mutex::new(Vec::new()),
        };
        
        loader.register_builtin_apps();
        loader
    }

    /// Register built-in applications
    fn register_builtin_apps(&mut self) {
        let mut apps = self.builtin_apps.lock();
        
        // Built-in shell application
        apps.push(ApplicationBinary {
            name: "shell".to_string(),
            entry_point: 0x1000,
            code: alloc::vec![0; 4096], // Placeholder code
            data: alloc::vec![0; 1024], // Placeholder data
            permissions: {
                let mut perms = Vec::new();
                perms.push("file_access".to_string());
                perms.push("memory_alloc".to_string());
                perms
            },
            dependencies: Vec::new(),
        });

        // Built-in text editor
        apps.push(ApplicationBinary {
            name: "edit".to_string(),
            entry_point: 0x1000,
            code: alloc::vec![0; 8192],
            data: alloc::vec![0; 2048],
            permissions: {
                let mut perms = Vec::new();
                perms.push("file_access".to_string());
                perms.push("memory_alloc".to_string());
                perms
            },
            dependencies: Vec::new(),
        });

        // Built-in system monitor
        apps.push(ApplicationBinary {
            name: "monitor".to_string(),
            entry_point: 0x1000,
            code: alloc::vec![0; 4096],
            data: alloc::vec![0; 1024],
            permissions: {
                let mut perms = Vec::new();
                perms.push("system_info".to_string());
                perms
            },
            dependencies: Vec::new(),
        });

        crate::println!("Registered {} built-in applications", apps.len());
    }

    /// Load and execute an application
    pub fn execute_application(&self, name: &str, args: &[&str]) -> Result<i32, &'static str> {
        // Find application binary
        let apps = self.builtin_apps.lock();
        let app = apps.iter().find(|a| a.name == name)
            .ok_or("Application not found")?;

        // Create process
        let process = self.process_manager.create_process(name, Some(crate::types::TaskId(crate::init::INIT_TASK_ID as u64)));
        
        // Create runtime
        let mut runtime = UserRuntime::new(process);

        crate::println!("Loading application '{}' with {} args", name, args.len());

        // Allocate memory for the application
        let code_addr = runtime.allocate_memory(app.code.len())?;
        let data_addr = runtime.allocate_memory(app.data.len())?;

        crate::println!("  Code loaded at: 0x{:x}", code_addr);
        crate::println!("  Data loaded at: 0x{:x}", data_addr);

        // Execute based on application type
        let exit_code = match name {
            "shell" => self.run_shell_app(&mut runtime, args),
            "edit" => self.run_editor_app(&mut runtime, args),
            "monitor" => self.run_monitor_app(&mut runtime, args),
            _ => {
                crate::println!("Unknown application type: {}", name);
                Err("Unknown application")
            }
        }?;

        // Clean up
        self.process_manager.terminate_process(runtime.process.pid)?;

        Ok(exit_code)
    }

    /// Run shell application
    fn run_shell_app(&self, runtime: &mut UserRuntime, args: &[&str]) -> Result<i32, &'static str> {
        crate::println!("=== Mach_R Interactive Shell ===");
        
        runtime.set_environment("PS1", "mach_r$ ");
        
        if args.is_empty() {
            // Interactive mode
            runtime.utilities.run_interactive_shell();
        } else {
            // Execute single command
            let command = args.join(" ");
            runtime.execute_command(&command)?;
        }
        
        Ok(0)
    }

    /// Run text editor application
    fn run_editor_app(&self, runtime: &mut UserRuntime, args: &[&str]) -> Result<i32, &'static str> {
        crate::println!("=== Mach_R Text Editor ===");
        
        let filename = if args.is_empty() {
            "untitled.txt"
        } else {
            args[0]
        };

        crate::println!("Opening file: {}", filename);
        
        // Simulate text editor functionality
        match runtime.open_file(filename) {
            Ok(fd) => {
                crate::println!("File opened successfully (fd: {})", fd);
                crate::println!("Editor functionality would be available here");
                crate::println!("Commands: :w (save), :q (quit), :wq (save and quit)");
                crate::println!("Text editing simulation complete");
            },
            Err(_) => {
                crate::println!("Creating new file: {}", filename);
                crate::println!("New file editor mode");
            }
        }
        
        Ok(0)
    }

    /// Run system monitor application
    fn run_monitor_app(&self, runtime: &mut UserRuntime, _args: &[&str]) -> Result<i32, &'static str> {
        crate::println!("=== Mach_R System Monitor ===");
        
        // Show system information
        crate::println!("Kernel: {} v{}", crate::NAME, crate::VERSION);
        crate::println!("Architecture: ARM64");
        crate::println!("");

        // Show processes
        crate::println!("Running Processes:");
        let processes = self.process_manager.list_processes();
        for proc in &processes {
            crate::println!("  PID {}: {} ({:?}) - {} bytes", 
                proc.pid.0, proc.name, proc.state, proc.memory_usage);
        }
        crate::println!("");

        // Show system servers
        crate::println!("System Servers:");
        crate::println!("  Name Server: Running on port 100");
        crate::println!("  File Server: Running on port 200");
        crate::println!("  VM Server: Running on port 300");
        crate::println!("");

        // Show memory usage
        crate::println!("Memory Usage:");
        let total_user_memory: usize = processes.iter().map(|p| p.memory_usage).sum();
        crate::println!("  Total user memory: {} bytes", total_user_memory);
        crate::println!("  Page-based allocation active");
        
        // Show VirtIO devices
        crate::println!("VirtIO Devices:");
        let virtio_manager = crate::drivers::virtio::manager();
        crate::println!("  Console: {}", if virtio_manager.has_console() { "Available" } else { "Not found" });
        crate::println!("  Block: {}", if virtio_manager.has_block_device() { "Available" } else { "Not found" });
        crate::println!("  Network: {}", if virtio_manager.has_network_device() { "Available" } else { "Not found" });
        
        Ok(0)
    }

    /// List available applications
    pub fn list_applications(&self) -> Vec<String> {
        let apps = self.builtin_apps.lock();
        apps.iter().map(|a| a.name.clone()).collect()
    }

    /// Get process manager reference
    pub fn process_manager(&self) -> &ProcessManager {
        &self.process_manager
    }
}

/// Global application loader
static mut APPLICATION_LOADER: Option<ApplicationLoader> = None;

/// Initialize userland subsystem
pub fn init() {
    crate::println!("Initializing userland subsystem...");
    
    unsafe {
        APPLICATION_LOADER = Some(ApplicationLoader::new());
    }
    
    crate::println!("Userland initialization complete");
}

/// Get the application loader
pub fn application_loader() -> &'static ApplicationLoader {
    unsafe {
        (*core::ptr::addr_of!(APPLICATION_LOADER)).as_ref()
            .expect("Userland not initialized")
    }
}

/// Execute an application
pub fn execute_app(name: &str, args: &[&str]) -> Result<i32, &'static str> {
    application_loader().execute_application(name, args)
}

/// List all available applications
pub fn list_apps() -> Vec<String> {
    application_loader().list_applications()
}

/// Demonstrate userland capabilities
pub fn demonstrate_userland() {
    crate::println!("\n=== Userland Application Demonstration ===");
    
    let loader = application_loader();
    
    // List available applications
    crate::println!("Available applications:");
    for app in loader.list_applications() {
        crate::println!("  - {}", app);
    }
    crate::println!();
    
    // Execute system monitor
    crate::println!("Running system monitor...");
    if let Err(e) = execute_app("monitor", &[]) {
        crate::println!("Monitor failed: {}", e);
    }
    crate::println!();
    
    // Execute text editor
    crate::println!("Running text editor...");
    if let Err(e) = execute_app("edit", &["example.txt"]) {
        crate::println!("Editor failed: {}", e);
    }
    crate::println!();
    
    // Show process information
    let proc_mgr = loader.process_manager();
    let processes = proc_mgr.list_processes();
    crate::println!("Current processes: {}", processes.len());
    
    // Clean up any zombies
    proc_mgr.reap_zombies();
    
    crate::println!("Userland demonstration complete");
}