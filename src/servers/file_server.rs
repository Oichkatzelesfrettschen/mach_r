//! File Server Implementation using MIG
//!
//! Provides file system operations via Mach message passing.
//! Uses the MIG (Mach Interface Generator) system for type-safe IPC.

use crate::message::Message;
use crate::types::{PortId, TaskId};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

/// File Server operation codes (MIG-style)
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum FileServerOp {
    FileOpen = 3001,
    FileClose = 3002,
    FileRead = 3003,
    FileWrite = 3004,
    FileSeek = 3005,
    FileStat = 3006,
    FileMkdir = 3007,
    FileRmdir = 3008,
    FileUnlink = 3009,
}

/// File system statistics structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileStat {
    pub size: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
}

/// Open file descriptor
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    pub fd: i32,
    pub path: String,
    pub flags: u32,
    pub offset: u64,
    pub owner_task: TaskId,
}

/// Simple in-memory file system
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub data: Vec<u8>,
    pub stat: FileStat,
    pub is_directory: bool,
}

/// File Server implementation
pub struct FileServer {
    /// Server port for receiving requests
    server_port: PortId,
    /// Server task ID
    server_task: TaskId,
    /// Open file descriptors
    file_descriptors: Mutex<BTreeMap<i32, FileDescriptor>>,
    /// Next available file descriptor
    next_fd: Mutex<i32>,
    /// In-memory file system
    files: Mutex<BTreeMap<String, FileEntry>>,
}

impl FileServer {
    /// Create a new File Server
    pub fn new(server_task: TaskId) -> Self {
        let server_port = PortId(200); // Well-known file server port

        let server = Self {
            server_port,
            server_task,
            file_descriptors: Mutex::new(BTreeMap::new()),
            next_fd: Mutex::new(3), // Start after stdin/stdout/stderr
            files: Mutex::new(BTreeMap::new()),
        };

        // Create root directory
        server.create_initial_filesystem();

        server
    }

    /// Get the server port ID
    pub fn server_port(&self) -> PortId {
        self.server_port
    }

    /// Create initial filesystem structure
    fn create_initial_filesystem(&self) {
        let mut files = self.files.lock();
        let current_time = crate::arch::current_timestamp();

        // Root directory
        files.insert(
            "/".to_string(),
            FileEntry {
                path: "/".to_string(),
                data: Vec::new(),
                stat: FileStat {
                    size: 0,
                    mode: 0o755 | 0o040000, // Directory mode
                    uid: 0,
                    gid: 0,
                    atime: current_time,
                    mtime: current_time,
                    ctime: current_time,
                },
                is_directory: true,
            },
        );

        // /dev directory
        files.insert(
            "/dev".to_string(),
            FileEntry {
                path: "/dev".to_string(),
                data: Vec::new(),
                stat: FileStat {
                    size: 0,
                    mode: 0o755 | 0o040000,
                    uid: 0,
                    gid: 0,
                    atime: current_time,
                    mtime: current_time,
                    ctime: current_time,
                },
                is_directory: true,
            },
        );

        // /dev/null
        files.insert(
            "/dev/null".to_string(),
            FileEntry {
                path: "/dev/null".to_string(),
                data: Vec::new(),
                stat: FileStat {
                    size: 0,
                    mode: 0o666 | 0o020000, // Character device
                    uid: 0,
                    gid: 0,
                    atime: current_time,
                    mtime: current_time,
                    ctime: current_time,
                },
                is_directory: false,
            },
        );

        // Sample text file
        let hello_content = b"Hello from Mach_R file system!\n";
        files.insert(
            "/hello.txt".to_string(),
            FileEntry {
                path: "/hello.txt".to_string(),
                data: hello_content.to_vec(),
                stat: FileStat {
                    size: hello_content.len() as u64,
                    mode: 0o644 | 0o100000, // Regular file
                    uid: 0,
                    gid: 0,
                    atime: current_time,
                    mtime: current_time,
                    ctime: current_time,
                },
                is_directory: false,
            },
        );
    }

    /// Allocate a new file descriptor
    fn allocate_fd(&self) -> i32 {
        let mut next_fd = self.next_fd.lock();
        let fd = *next_fd;
        *next_fd += 1;
        fd
    }

    /// Handle file open request
    pub fn file_open(&self, path: String, flags: u32, requesting_task: TaskId) -> Result<i32, i32> {
        let files = self.files.lock();

        // Check if file exists
        if let Some(file_entry) = files.get(&path) {
            if file_entry.is_directory && (flags & 0o3) != 0 {
                // O_RDONLY = 0
                return Err(-21); // EISDIR
            }

            let fd = self.allocate_fd();
            let file_desc = FileDescriptor {
                fd,
                path: path.clone(),
                flags,
                offset: 0,
                owner_task: requesting_task,
            };

            let mut descriptors = self.file_descriptors.lock();
            descriptors.insert(fd, file_desc);

            Ok(fd)
        } else if flags & 0o100 != 0 {
            // O_CREAT
            // Create new file
            drop(files);
            let mut files = self.files.lock();
            let current_time = crate::arch::current_timestamp();

            let new_file = FileEntry {
                path: path.clone(),
                data: Vec::new(),
                stat: FileStat {
                    size: 0,
                    mode: 0o644 | 0o100000, // Regular file
                    uid: 0,
                    gid: 0,
                    atime: current_time,
                    mtime: current_time,
                    ctime: current_time,
                },
                is_directory: false,
            };

            files.insert(path.clone(), new_file);

            let fd = self.allocate_fd();
            let file_desc = FileDescriptor {
                fd,
                path,
                flags,
                offset: 0,
                owner_task: requesting_task,
            };

            let mut descriptors = self.file_descriptors.lock();
            descriptors.insert(fd, file_desc);

            Ok(fd)
        } else {
            Err(-2) // ENOENT
        }
    }

    /// Handle file close request
    pub fn file_close(&self, fd: i32, requesting_task: TaskId) -> Result<i32, i32> {
        let mut descriptors = self.file_descriptors.lock();

        if let Some(desc) = descriptors.get(&fd) {
            if desc.owner_task != requesting_task {
                return Err(-13); // EACCES
            }

            descriptors.remove(&fd);
            Ok(0)
        } else {
            Err(-9) // EBADF
        }
    }

    /// Handle file read request
    pub fn file_read(
        &self,
        fd: i32,
        count: usize,
        requesting_task: TaskId,
    ) -> Result<(Vec<u8>, isize), i32> {
        let mut descriptors = self.file_descriptors.lock();
        let files = self.files.lock();

        if let Some(desc) = descriptors.get_mut(&fd) {
            if desc.owner_task != requesting_task {
                return Err(-13); // EACCES
            }

            if desc.flags & 0o3 == 1 {
                // O_WRONLY
                return Err(-9); // EBADF
            }

            // Special handling for /dev/null
            if desc.path == "/dev/null" {
                return Ok((Vec::new(), 0));
            }

            if let Some(file_entry) = files.get(&desc.path) {
                if file_entry.is_directory {
                    return Err(-21); // EISDIR
                }

                let start = desc.offset as usize;
                let end = (start + count).min(file_entry.data.len());

                if start >= file_entry.data.len() {
                    return Ok((Vec::new(), 0)); // EOF
                }

                let data = file_entry.data[start..end].to_vec();
                let bytes_read = data.len() as isize;
                desc.offset += bytes_read as u64;

                Ok((data, bytes_read))
            } else {
                Err(-2) // ENOENT
            }
        } else {
            Err(-9) // EBADF
        }
    }

    /// Handle file write request
    pub fn file_write(
        &self,
        fd: i32,
        data: Vec<u8>,
        requesting_task: TaskId,
    ) -> Result<isize, i32> {
        let mut descriptors = self.file_descriptors.lock();
        let mut files = self.files.lock();

        if let Some(desc) = descriptors.get_mut(&fd) {
            if desc.owner_task != requesting_task {
                return Err(-13); // EACCES
            }

            if desc.flags & 0o3 == 0 {
                // O_RDONLY
                return Err(-9); // EBADF
            }

            // Special handling for /dev/null
            if desc.path == "/dev/null" {
                return Ok(data.len() as isize);
            }

            if let Some(file_entry) = files.get_mut(&desc.path) {
                if file_entry.is_directory {
                    return Err(-21); // EISDIR
                }

                let start = desc.offset as usize;

                // Extend file if necessary
                if start + data.len() > file_entry.data.len() {
                    file_entry.data.resize(start + data.len(), 0);
                }

                // Write data
                file_entry.data[start..start + data.len()].copy_from_slice(&data);

                // Update file stats
                file_entry.stat.size = file_entry.data.len() as u64;
                file_entry.stat.mtime = crate::arch::current_timestamp();

                // Update offset
                desc.offset += data.len() as u64;

                Ok(data.len() as isize)
            } else {
                Err(-2) // ENOENT
            }
        } else {
            Err(-9) // EBADF
        }
    }

    /// Handle file stat request
    pub fn file_stat(&self, path: String) -> Result<FileStat, i32> {
        let files = self.files.lock();

        if let Some(file_entry) = files.get(&path) {
            Ok(file_entry.stat)
        } else {
            Err(-2) // ENOENT
        }
    }

    /// Handle message from clients
    pub fn handle_message(&self, msg: Message) -> Option<Message> {
        // Parse operation code from message
        let data = msg.data();
        if data.len() < 4 {
            return Some(self.create_error_reply(msg.remote_port(), -22)); // EINVAL
        }

        let op_code = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let op = match op_code {
            3001 => FileServerOp::FileOpen,
            3002 => FileServerOp::FileClose,
            3003 => FileServerOp::FileRead,
            3004 => FileServerOp::FileWrite,
            3005 => FileServerOp::FileSeek,
            3006 => FileServerOp::FileStat,
            3007 => FileServerOp::FileMkdir,
            3008 => FileServerOp::FileRmdir,
            3009 => FileServerOp::FileUnlink,
            _ => return Some(self.create_error_reply(msg.remote_port(), -22)),
        };

        match op {
            FileServerOp::FileOpen => self.handle_open_msg(msg),
            FileServerOp::FileClose => self.handle_close_msg(msg),
            FileServerOp::FileRead => self.handle_read_msg(msg),
            FileServerOp::FileWrite => self.handle_write_msg(msg),
            FileServerOp::FileSeek => self.handle_seek_msg(msg),
            FileServerOp::FileStat => self.handle_stat_msg(msg),
            FileServerOp::FileMkdir => self.handle_mkdir_msg(msg),
            FileServerOp::FileRmdir => self.handle_rmdir_msg(msg),
            FileServerOp::FileUnlink => self.handle_unlink_msg(msg),
        }
    }

    fn handle_open_msg(&self, msg: Message) -> Option<Message> {
        // Simplified message parsing - in real implementation would have proper MIG serialization
        let requesting_task = TaskId(1);
        match self.file_open("/hello.txt".to_string(), 0, requesting_task) {
            Ok(fd) => {
                let reply_data = fd.to_le_bytes().to_vec();
                Some(Message::new_out_of_line(msg.remote_port(), reply_data))
            }
            Err(errno) => Some(self.create_error_reply(msg.remote_port(), errno)),
        }
    }

    fn handle_close_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_read_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_write_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_seek_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_stat_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_mkdir_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_rmdir_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn handle_unlink_msg(&self, _msg: Message) -> Option<Message> {
        None // Simplified
    }

    fn create_error_reply(&self, remote_port: PortId, errno: i32) -> Message {
        let reply_data = errno.to_le_bytes().to_vec();
        Message::new_out_of_line(remote_port, reply_data)
    }
}

/// Global File Server instance
static mut FILE_SERVER: Option<FileServer> = None;

/// Initialize the File Server
pub fn init() {
    let server_task = TaskId(3); // File server gets task ID 3
    let file_server = FileServer::new(server_task);

    // Register with server registry
    super::SERVER_REGISTRY.register_server("file_server", file_server.server_port());

    // Register with name server
    if let Some(name_server) =
        unsafe { (*core::ptr::addr_of!(super::name_server::NAME_SERVER)).as_ref() }
    {
        let _ = name_server.register(
            "file_server".to_string(),
            file_server.server_port(),
            server_task,
        );
    }

    unsafe {
        FILE_SERVER = Some(file_server);
    }

    crate::println!("File Server initialized on port {}", 200);
}

/// Get the File Server instance
pub fn file_server() -> &'static FileServer {
    unsafe {
        (*core::ptr::addr_of!(FILE_SERVER))
            .as_ref()
            .expect("File Server not initialized")
    }
}
