//! Device Server Routines
//!
//! Based on Mach4 device/ds_routines.c
//! Implements the device server interface for handling device requests.

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use crate::device::conf;
use crate::device::dev_hdr::{self, DeviceResult, MachDevice};
use crate::device::io_req::{IoMode, IoReqId, IoRequest};
use crate::ipc::PortName;

// ============================================================================
// Device Server Port
// ============================================================================

/// Device server state
pub struct DeviceServer {
    /// Master device port
    pub master_port: Mutex<Option<PortName>>,
    /// Is server running?
    pub running: bool,
}

impl DeviceServer {
    pub fn new() -> Self {
        Self {
            master_port: Mutex::new(None),
            running: false,
        }
    }

    /// Start the device server
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the device server
    pub fn stop(&mut self) {
        self.running = false;
    }
}

impl Default for DeviceServer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Device Server Operations
// ============================================================================

/// Device open request result
#[derive(Debug)]
pub struct DeviceOpenResult {
    pub device_port: Option<PortName>,
    pub error: DeviceResult,
}

/// Open a device by name
pub fn ds_device_open(name: &str, _mode: u32) -> DeviceOpenResult {
    // Search for device
    let (ops, unit) = match conf::dev_search(name) {
        Some(result) => result,
        None => {
            return DeviceOpenResult {
                device_port: None,
                error: DeviceResult::DeviceNotFound,
            };
        }
    };

    // Get or create device
    let device = match dev_hdr::lookup(name) {
        Some(dev) => dev,
        None => {
            // Create new device
            let dev_number = (ops.name.len() as u32) << 8 | unit;
            let dev = dev_hdr::register(String::from(name), dev_number);
            dev.set_ops(ops);
            dev
        }
    };

    // Open the device
    let result = dev_hdr::device_open(&device);

    if result == DeviceResult::Success {
        // Create port for device
        let port = PortName(device.id.0 as u32 + 0x1000); // Synthetic port
        dev_hdr::dev_port_enter(port, device.id);

        DeviceOpenResult {
            device_port: Some(port),
            error: DeviceResult::Success,
        }
    } else {
        DeviceOpenResult {
            device_port: None,
            error: result,
        }
    }
}

/// Close a device
pub fn ds_device_close(device_port: PortName) -> DeviceResult {
    let device = match dev_hdr::dev_port_lookup(device_port) {
        Some(dev) => dev,
        None => return DeviceResult::DeviceNotFound,
    };

    dev_hdr::device_close(&device)
}

/// Device write
pub fn ds_device_write(
    device_port: PortName,
    mode: IoMode,
    recnum: u64,
    data: Vec<u8>,
) -> Result<u32, DeviceResult> {
    let device = dev_hdr::dev_port_lookup(device_port).ok_or(DeviceResult::DeviceNotFound)?;

    // Check device is open
    if !device.is_open() {
        return Err(DeviceResult::NotOpen);
    }

    // Get ops
    let ops = device.get_ops().ok_or(DeviceResult::NotSupported)?;

    let write_fn = ops.write.ok_or(DeviceResult::NotSupported)?;

    // Create I/O request
    let mut req = IoRequest::write(
        IoReqId(0), // Will be assigned by manager
        device.id,
        device.unit,
        recnum,
        data.clone(),
    );
    req.mode = mode;

    // Start I/O
    device.io_start();

    // Call driver
    let result = write_fn(&device, &req);

    // Complete I/O
    device.io_done();

    match result {
        DeviceResult::Success => Ok(data.len() as u32 - req.get_residual()),
        err => Err(err),
    }
}

/// Device read
pub fn ds_device_read(
    device_port: PortName,
    mode: IoMode,
    recnum: u64,
    count: u64,
) -> Result<Vec<u8>, DeviceResult> {
    let device = dev_hdr::dev_port_lookup(device_port).ok_or(DeviceResult::DeviceNotFound)?;

    // Check device is open
    if !device.is_open() {
        return Err(DeviceResult::NotOpen);
    }

    // Get ops
    let ops = device.get_ops().ok_or(DeviceResult::NotSupported)?;

    let read_fn = ops.read.ok_or(DeviceResult::NotSupported)?;

    // Create I/O request
    let mut req = IoRequest::read(IoReqId(0), device.id, device.unit, recnum, count);
    req.mode = mode;

    // Start I/O
    device.io_start();

    // Call driver
    let result = read_fn(&device, &req);

    // Complete I/O
    device.io_done();

    match result {
        DeviceResult::Success => Ok(req.get_data()),
        err => Err(err),
    }
}

/// Get device status
pub fn ds_device_get_status(
    device_port: PortName,
    flavor: u32,
    status: &mut [u32],
) -> DeviceResult {
    let device = match dev_hdr::dev_port_lookup(device_port) {
        Some(dev) => dev,
        None => return DeviceResult::DeviceNotFound,
    };

    // Get ops
    let ops = match device.get_ops() {
        Some(o) => o,
        None => return DeviceResult::NotSupported,
    };

    let getstat_fn = match ops.getstat {
        Some(f) => f,
        None => return DeviceResult::NotSupported,
    };

    getstat_fn(&device, flavor, status)
}

/// Set device status
pub fn ds_device_set_status(device_port: PortName, flavor: u32, status: &[u32]) -> DeviceResult {
    let device = match dev_hdr::dev_port_lookup(device_port) {
        Some(dev) => dev,
        None => return DeviceResult::DeviceNotFound,
    };

    // Get ops
    let ops = match device.get_ops() {
        Some(o) => o,
        None => return DeviceResult::NotSupported,
    };

    let setstat_fn = match ops.setstat {
        Some(f) => f,
        None => return DeviceResult::NotSupported,
    };

    setstat_fn(&device, flavor, status)
}

/// Map device memory
pub fn ds_device_map(
    device_port: PortName,
    prot: u32,
    offset: u64,
    _size: u64,
) -> Result<u64, DeviceResult> {
    let device = dev_hdr::dev_port_lookup(device_port).ok_or(DeviceResult::DeviceNotFound)?;

    // Get ops
    let ops = device.get_ops().ok_or(DeviceResult::NotSupported)?;

    let mmap_fn = ops.mmap.ok_or(DeviceResult::NotSupported)?;

    mmap_fn(&device, offset, prot)
}

// ============================================================================
// Device Reply Port Handling
// ============================================================================

/// Handle port death for a device
pub fn device_port_death(device_port: PortName) {
    if let Some(device) = dev_hdr::dev_port_lookup(device_port) {
        // Notify driver
        if let Some(ops) = device.get_ops() {
            if let Some(port_death_fn) = ops.port_death {
                let _ = port_death_fn(&device, device_port.0);
            }
        }

        // Remove port mapping
        dev_hdr::dev_port_remove(device_port);
    }
}

// ============================================================================
// Device Iterator
// ============================================================================

/// Iterate over all devices, calling function on each
pub fn dev_map<F>(mut f: F)
where
    F: FnMut(&MachDevice) -> bool,
{
    let registry = dev_hdr::registry().lock();
    for device in registry.list() {
        if !f(&device) {
            break;
        }
    }
}

// ============================================================================
// Global State
// ============================================================================

static DEVICE_SERVER: spin::Once<Mutex<DeviceServer>> = spin::Once::new();

/// Initialize device server
pub fn init() {
    DEVICE_SERVER.call_once(|| Mutex::new(DeviceServer::new()));
}

/// Get device server
pub fn server() -> &'static Mutex<DeviceServer> {
    DEVICE_SERVER.get().expect("Device server not initialized")
}

/// Start device server
pub fn start_server() {
    server().lock().start();
}

/// Stop device server
pub fn stop_server() {
    server().lock().stop();
}

/// Set master device port
pub fn set_master_port(port: PortName) {
    *server().lock().master_port.lock() = Some(port);
}

/// Get master device port
pub fn get_master_port() -> Option<PortName> {
    *server().lock().master_port.lock()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_server() {
        let mut server = DeviceServer::new();
        assert!(!server.running);

        server.start();
        assert!(server.running);

        server.stop();
        assert!(!server.running);
    }

    #[test]
    fn test_device_open_not_found() {
        let result = ds_device_open("nonexistent", 0);
        assert_eq!(result.error, DeviceResult::DeviceNotFound);
        assert!(result.device_port.is_none());
    }
}
