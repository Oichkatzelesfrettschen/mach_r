//! Filesystem integration module
//! Integrates pure Rust filesystem implementations

use heapless::Vec;

pub mod fat;
pub mod vfs;

/// Filesystem type enumeration
#[derive(Debug, Clone, Copy)]
pub enum FilesystemType {
    Fat32,
    Fat16,
    Unknown,
}

/// Mount point information
#[derive(Debug)]
pub struct MountPoint {
    pub path: heapless::String<256>,
    pub fs_type: FilesystemType,
    pub device: heapless::String<64>,
    pub readonly: bool,
}

/// Filesystem manager for Mach_R
pub struct FilesystemManager {
    mounts: Vec<MountPoint, 16>,
    initialized: bool,
}

impl FilesystemManager {
    /// Create a new filesystem manager
    pub fn new() -> Self {
        Self {
            mounts: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize the filesystem manager
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Ok(());
        }

        // Initialize VFS layer
        vfs::init()?;

        // Initialize FAT filesystem support
        fat::init()?;

        self.initialized = true;
        Ok(())
    }

    /// Mount a filesystem
    pub fn mount(
        &mut self,
        path: &str,
        device: &str,
        fs_type: FilesystemType,
    ) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Filesystem manager not initialized");
        }

        let mut mount_path = heapless::String::new();
        mount_path.push_str(path).map_err(|_| "Path too long")?;

        let mut mount_device = heapless::String::new();
        mount_device
            .push_str(device)
            .map_err(|_| "Device path too long")?;

        let mount_point = MountPoint {
            path: mount_path,
            fs_type,
            device: mount_device,
            readonly: false,
        };

        self.mounts
            .push(mount_point)
            .map_err(|_| "Too many mount points")?;
        Ok(())
    }

    /// Get mount points
    pub fn get_mounts(&self) -> &Vec<MountPoint, 16> {
        &self.mounts
    }
}

static mut FS_MANAGER: Option<FilesystemManager> = None;

/// Initialize the filesystem subsystem
pub fn init() -> Result<(), &'static str> {
    let mut manager = FilesystemManager::new();
    manager.init()?;

    unsafe {
        FS_MANAGER = Some(manager);
    }

    Ok(())
}

/// Get the global filesystem manager
pub fn get_manager() -> Option<&'static mut FilesystemManager> {
    unsafe { (*core::ptr::addr_of_mut!(FS_MANAGER)).as_mut() }
}
