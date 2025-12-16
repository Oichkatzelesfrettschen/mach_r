//! Network stack integration module
//! Integrates smoltcp TCP/IP stack and Makiko SSH implementation

use heapless::Vec;

pub mod ssh;
pub mod tcp;

/// Network configuration for Mach_R
#[derive(Debug)]
pub struct NetworkConfig {
    /// IP address for the system
    pub ip_addr: [u8; 4],
    /// Subnet mask
    pub subnet_mask: [u8; 4],
    /// Gateway address
    pub gateway: [u8; 4],
    /// DNS servers
    pub dns_servers: Vec<[u8; 4], 3>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        let mut dns_servers = Vec::new();
        dns_servers.push([8, 8, 8, 8]).ok();
        dns_servers.push([8, 8, 4, 4]).ok();
        dns_servers.push([1, 1, 1, 1]).ok();

        Self {
            ip_addr: [192, 168, 1, 100],
            subnet_mask: [255, 255, 255, 0],
            gateway: [192, 168, 1, 1],
            dns_servers,
        }
    }
}

/// Initialize the network stack
pub fn init() -> Result<(), &'static str> {
    // Initialize smoltcp TCP/IP stack
    tcp::init()?;

    // Initialize SSH server
    ssh::init()?;

    Ok(())
}
