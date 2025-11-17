/// MIG type system - IPC types and type checking

use std::collections::HashMap;

/// Type system for MIG
pub struct TypeSystem {
    types: HashMap<String, IpcType>,
}

impl TypeSystem {
    pub fn new() -> Self {
        let mut ts = Self {
            types: HashMap::new(),
        };
        ts.init_builtin_types();
        ts
    }

    /// Initialize built-in Mach IPC types
    fn init_builtin_types(&mut self) {
        // TODO: Add all built-in types from mach/message.h
        // MACH_MSG_TYPE_* constants
    }

    pub fn add_type(&mut self, name: String, ipc_type: IpcType) {
        self.types.insert(name, ipc_type);
    }

    pub fn get_type(&self, name: &str) -> Option<&IpcType> {
        self.types.get(name)
    }
}

impl Default for TypeSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// IPC type information
#[derive(Debug, Clone)]
pub struct IpcType {
    pub name: String,
    pub size_bytes: u32,
    pub alignment: u32,
    pub ipc_type_in: IpcTypeName,
    pub ipc_type_out: IpcTypeName,
    pub element_size_bits: u32,
    pub element_count: ElementCount,
    pub inline: bool,
    pub port_type: Option<PortType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcTypeName {
    Unstructured,
    Bit,
    Boolean,
    Integer8,
    Integer16,
    Integer32,
    Integer64,
    Char,
    Byte,
    Real,
    String,
    PortName,
    MoveReceive,
    MoveSend,
    MoveSendOnce,
    CopySend,
    MakeSend,
    MakeSendOnce,
    Polymorphic,
}

#[derive(Debug, Clone, Copy)]
pub enum ElementCount {
    Fixed(u32),
    Variable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    Send,
    Receive,
    SendOnce,
}
