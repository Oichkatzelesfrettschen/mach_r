//! TCP/IP stack integration
//! Pure Rust network stack for Mach_R (internal implementation)

use heapless::Vec;

/// Maximum number of concurrent TCP sockets
const MAX_SOCKETS: usize = 16;
/// Maximum number of network interfaces
const MAX_IFACES: usize = 4;

/// Network interface wrapper
pub struct NetworkInterface {
    initialized: bool,
    ip_address: [u8; 4],
}

impl NetworkInterface {
    /// Create a new network interface
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            initialized: false,
            ip_address: [192, 168, 1, 100],
        })
    }

    /// Initialize the interface
    pub fn init(&mut self, ip_address: [u8; 4]) -> Result<(), &'static str> {
        self.ip_address = ip_address;
        self.initialized = true;
        Ok(())
    }

    /// Process network packets
    pub fn poll(&mut self) -> Result<bool, &'static str> {
        if !self.initialized {
            return Err("Interface not initialized");
        }
        // TODO: Poll the interface and process packets
        Ok(false)
    }

    /// Create a TCP socket (returns socket ID)
    pub fn create_tcp_socket(&mut self) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Interface not initialized");
        }
        // TODO: Create and manage TCP socket
        Ok(1) // Return dummy socket ID
    }
}

/// TCP/IP stack manager
pub struct TcpStack {
    interfaces: Vec<NetworkInterface, MAX_IFACES>,
    initialized: bool,
}

impl TcpStack {
    /// Create a new TCP/IP stack
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize the TCP/IP stack
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Ok(());
        }

        // Create default network interface
        let mut iface = NetworkInterface::new()?;
        iface.init([192, 168, 1, 100])?;
        self.interfaces
            .push(iface)
            .map_err(|_| "Too many interfaces")?;

        self.initialized = true;
        Ok(())
    }

    /// Process network events
    pub fn process(&mut self) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("TCP stack not initialized");
        }

        for iface in &mut self.interfaces {
            iface.poll()?;
        }

        Ok(())
    }
}

static mut TCP_STACK: Option<TcpStack> = None;

/// Initialize the TCP/IP subsystem
pub fn init() -> Result<(), &'static str> {
    let mut stack = TcpStack::new();
    stack.init()?;

    unsafe {
        TCP_STACK = Some(stack);
    }

    Ok(())
}

/// Get a reference to the global TCP stack
pub fn get_stack() -> Option<&'static mut TcpStack> {
    unsafe { (*core::ptr::addr_of_mut!(TCP_STACK)).as_mut() }
}
