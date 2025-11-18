//! Type resolution and validation for MIG types

use crate::parser::ast::{TypeDecl, TypeSpec, Subsystem, Statement};
use std::collections::HashMap;

/// Mach message type encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachMsgType {
    /// Boolean
    TypeBool,
    /// 16-bit integer
    TypeInteger16,
    /// 32-bit integer
    TypeInteger32,
    /// 64-bit integer
    TypeInteger64,
    /// Byte
    TypeByte,
    /// Character
    TypeChar,
    /// Real (floating point)
    TypeReal,
    /// String
    TypeString,
    /// Port with disposition
    TypePort(PortDisposition),
    /// Polymorphic (runtime-determined)
    TypePolymorphic,
}

/// Port disposition for IPC rights transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDisposition {
    /// Transfer receive right
    MoveReceive,
    /// Copy send right
    CopySend,
    /// Create and transfer send right
    MakeSend,
    /// Transfer send right
    MoveSend,
    /// Create and transfer send-once right
    MakeSendOnce,
    /// Transfer send-once right
    MoveSendOnce,
    /// Receive right (in message)
    PortReceive,
    /// Send right (in message)
    PortSend,
    /// Send-once right (in message)
    PortSendOnce,
    /// Port name (no rights)
    PortName,
}

/// Type size information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSize {
    /// Fixed size in bytes
    Fixed(usize),
    /// Variable size with maximum
    Variable { max: usize },
    /// Size determined at runtime
    Indefinite,
}

/// Resolved type information
#[derive(Debug, Clone)]
pub struct ResolvedType {
    /// Type name
    pub name: String,
    /// Mach message type
    pub mach_type: MachMsgType,
    /// C type mapping (from ctype annotation)
    pub c_type: Option<String>,
    /// Size information
    pub size: TypeSize,
    /// Is this an array type?
    pub is_array: bool,
    /// Array element type (if is_array is true)
    pub array_element: Option<Box<ResolvedType>>,
    /// Array size specification (if is_array is true)
    pub array_size: Option<crate::parser::ast::ArraySize>,
    /// Is this a polymorphic type?
    pub is_polymorphic: bool,
}

/// Type resolver for MIG types
pub struct TypeResolver {
    /// Type table mapping names to resolved types
    types: HashMap<String, ResolvedType>,
}

impl TypeResolver {
    /// Create a new type resolver with builtin types
    pub fn new() -> Self {
        let mut resolver = Self {
            types: HashMap::new(),
        };
        resolver.add_builtins();
        resolver
    }

    /// Add builtin Mach types
    fn add_builtins(&mut self) {
        // Integer types
        self.add_primitive("char", MachMsgType::TypeChar, "char", 1);
        self.add_primitive("short", MachMsgType::TypeInteger16, "short", 2);
        self.add_primitive("int", MachMsgType::TypeInteger32, "int", 4);
        self.add_primitive("int32", MachMsgType::TypeInteger32, "int32_t", 4);
        self.add_primitive("int32_t", MachMsgType::TypeInteger32, "int32_t", 4);
        self.add_primitive("int64", MachMsgType::TypeInteger64, "int64_t", 8);
        self.add_primitive("unsigned", MachMsgType::TypeInteger32, "unsigned int", 4);
        self.add_primitive("unsigned32", MachMsgType::TypeInteger32, "uint32_t", 4);
        self.add_primitive("unsigned64", MachMsgType::TypeInteger64, "uint64_t", 8);

        // MIG standard integer types (from std_types.defs)
        self.add_primitive("integer_8", MachMsgType::TypeByte, "int8_t", 1);
        self.add_primitive("integer_16", MachMsgType::TypeInteger16, "int16_t", 2);
        self.add_primitive("integer_32", MachMsgType::TypeInteger32, "int32_t", 4);
        self.add_primitive("integer_64", MachMsgType::TypeInteger64, "int64_t", 8);

        // Boolean
        self.add_primitive("boolean_t", MachMsgType::TypeBool, "boolean_t", 4);

        // Natural types (platform-dependent, assume 32-bit for now)
        self.add_primitive("natural_t", MachMsgType::TypeInteger32, "natural_t", 4);
        self.add_primitive("integer_t", MachMsgType::TypeInteger32, "integer_t", 4);

        // Port types
        self.add_port_type("mach_port_t", PortDisposition::CopySend, "mach_port_t");
        self.add_port_type("mach_port_name_t", PortDisposition::PortName, "mach_port_t");
        self.add_port_type("mach_port_move_receive_t", PortDisposition::MoveReceive, "mach_port_t");
        self.add_port_type("mach_port_copy_send_t", PortDisposition::CopySend, "mach_port_t");
        self.add_port_type("mach_port_make_send_t", PortDisposition::MakeSend, "mach_port_t");
        self.add_port_type("mach_port_move_send_t", PortDisposition::MoveSend, "mach_port_t");
        self.add_port_type("mach_port_make_send_once_t", PortDisposition::MakeSendOnce, "mach_port_t");
        self.add_port_type("mach_port_move_send_once_t", PortDisposition::MoveSendOnce, "mach_port_t");
        self.add_port_type("mach_port_receive_t", PortDisposition::PortReceive, "mach_port_t");
        self.add_port_type("mach_port_send_t", PortDisposition::PortSend, "mach_port_t");
        self.add_port_type("mach_port_send_once_t", PortDisposition::PortSendOnce, "mach_port_t");

        // Other common types
        self.add_primitive("kern_return_t", MachMsgType::TypeInteger32, "kern_return_t", 4);
        self.add_primitive("mach_msg_type_name_t", MachMsgType::TypeInteger32, "mach_msg_type_name_t", 4);
        self.add_primitive("mach_msg_timeout_t", MachMsgType::TypeInteger32, "mach_msg_timeout_t", 4);
        self.add_primitive("mach_msg_option_t", MachMsgType::TypeInteger32, "mach_msg_option_t", 4);
        self.add_primitive("mach_port_seqno_t", MachMsgType::TypeInteger32, "mach_port_seqno_t", 4);
    }

    /// Add a primitive type
    fn add_primitive(&mut self, name: &str, mach_type: MachMsgType, c_type: &str, size: usize) {
        self.types.insert(name.to_string(), ResolvedType {
            name: name.to_string(),
            mach_type,
            c_type: Some(c_type.to_string()),
            size: TypeSize::Fixed(size),
            is_array: false,
            array_element: None,
            array_size: None,
            is_polymorphic: false,
        });
    }

    /// Add a port type
    fn add_port_type(&mut self, name: &str, disposition: PortDisposition, c_type: &str) {
        self.types.insert(name.to_string(), ResolvedType {
            name: name.to_string(),
            mach_type: MachMsgType::TypePort(disposition),
            c_type: Some(c_type.to_string()),
            size: TypeSize::Fixed(4), // Port names are 32-bit
            is_array: false,
            array_element: None,
            array_size: None,
            is_polymorphic: false,
        });
    }

    /// Resolve types from a subsystem
    pub fn resolve_subsystem(&mut self, subsystem: &Subsystem) -> Result<(), super::SemanticError> {
        // Process type declarations
        for statement in &subsystem.statements {
            if let Statement::TypeDecl(decl) = statement {
                self.resolve_type_decl(decl)?;
            }
        }
        Ok(())
    }

    /// Resolve a type declaration
    fn resolve_type_decl(&mut self, decl: &TypeDecl) -> Result<(), super::SemanticError> {
        use crate::parser::ast::{ArraySize};

        let resolved = match &decl.spec {
            TypeSpec::Basic(base_name) => {
                // Look up base type
                let base_type = self.lookup(base_name)?.clone();
                ResolvedType {
                    name: decl.name.clone(),
                    mach_type: base_type.mach_type,
                    c_type: Some(decl.name.clone()), // Use declared name as C type
                    size: base_type.size,
                    is_array: false,
                    array_element: None,
                    array_size: None,
                    is_polymorphic: base_type.is_polymorphic,
                }
            }
            TypeSpec::Array { element, size } => {
                // Resolve element type
                if let TypeSpec::Basic(elem_name) = element.as_ref() {
                    let elem_type = self.lookup(elem_name)?.clone();

                    // Determine actual size based on array size spec
                    let type_size = match size {
                        ArraySize::Fixed(n) => {
                            // Fixed size array
                            if let TypeSize::Fixed(elem_size) = elem_type.size {
                                TypeSize::Fixed(elem_size * (*n as usize))
                            } else {
                                TypeSize::Indefinite
                            }
                        }
                        ArraySize::Variable | ArraySize::VariableWithMax(_) => {
                            // Variable size arrays are indefinite
                            TypeSize::Indefinite
                        }
                    };

                    ResolvedType {
                        name: decl.name.clone(),
                        mach_type: elem_type.mach_type,
                        c_type: Some(format!("{}*", elem_type.c_type.as_ref().unwrap_or(elem_name))),
                        size: type_size,
                        is_array: true,
                        array_element: Some(Box::new(elem_type)),
                        array_size: Some(*size),
                        is_polymorphic: false,
                    }
                } else {
                    return Err(super::SemanticError::UndefinedType {
                        name: decl.name.clone(),
                        location: "type declaration".to_string(),
                    });
                }
            }
            _ => {
                // For complex types, create a placeholder
                ResolvedType {
                    name: decl.name.clone(),
                    mach_type: MachMsgType::TypeInteger32,
                    c_type: Some(decl.name.clone()),
                    size: TypeSize::Fixed(4),
                    is_array: false,
                    array_element: None,
                    array_size: None,
                    is_polymorphic: false,
                }
            }
        };

        self.types.insert(decl.name.clone(), resolved);
        Ok(())
    }

    /// Look up a type by name
    pub fn lookup(&self, name: &str) -> Result<&ResolvedType, super::SemanticError> {
        self.types.get(name).ok_or_else(|| super::SemanticError::UndefinedType {
            name: name.to_string(),
            location: "type lookup".to_string(),
        })
    }

    /// Get the C type for a type name
    pub fn get_c_type(&self, name: &str) -> Option<String> {
        self.types.get(name).and_then(|t| t.c_type.clone())
    }

    /// Get the size of a type
    pub fn get_size(&self, name: &str) -> Option<TypeSize> {
        self.types.get(name).map(|t| t.size)
    }
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl MachMsgType {
    /// Convert MachMsgType to Mach IPC constant name
    pub fn to_mach_constant(&self) -> &'static str {
        match self {
            MachMsgType::TypeBool => "MACH_MSG_TYPE_BOOLEAN",
            MachMsgType::TypeInteger16 => "MACH_MSG_TYPE_INTEGER_16",
            MachMsgType::TypeInteger32 => "MACH_MSG_TYPE_INTEGER_32",
            MachMsgType::TypeInteger64 => "MACH_MSG_TYPE_INTEGER_64",
            MachMsgType::TypeByte => "MACH_MSG_TYPE_BYTE",
            MachMsgType::TypeChar => "MACH_MSG_TYPE_CHAR",
            MachMsgType::TypeReal => "MACH_MSG_TYPE_REAL",
            MachMsgType::TypeString => "MACH_MSG_TYPE_STRING",
            MachMsgType::TypePort(disposition) => disposition.to_mach_constant(),
            MachMsgType::TypePolymorphic => "MACH_MSG_TYPE_POLYMORPHIC",
        }
    }

    /// Get the bit size of this type for msgt_size field
    pub fn bit_size(&self) -> u32 {
        match self {
            MachMsgType::TypeBool => 32,
            MachMsgType::TypeInteger16 => 16,
            MachMsgType::TypeInteger32 => 32,
            MachMsgType::TypeInteger64 => 64,
            MachMsgType::TypeByte => 8,
            MachMsgType::TypeChar => 8,
            MachMsgType::TypeReal => 32,
            MachMsgType::TypeString => 8,
            MachMsgType::TypePort(_) => 32, // Port names are 32-bit
            MachMsgType::TypePolymorphic => 32,
        }
    }
}

impl PortDisposition {
    /// Convert PortDisposition to Mach IPC constant name
    pub fn to_mach_constant(&self) -> &'static str {
        match self {
            PortDisposition::MoveReceive => "MACH_MSG_TYPE_MOVE_RECEIVE",
            PortDisposition::CopySend => "MACH_MSG_TYPE_COPY_SEND",
            PortDisposition::MakeSend => "MACH_MSG_TYPE_MAKE_SEND",
            PortDisposition::MoveSend => "MACH_MSG_TYPE_MOVE_SEND",
            PortDisposition::MakeSendOnce => "MACH_MSG_TYPE_MAKE_SEND_ONCE",
            PortDisposition::MoveSendOnce => "MACH_MSG_TYPE_MOVE_SEND_ONCE",
            PortDisposition::PortReceive => "MACH_MSG_TYPE_PORT_RECEIVE",
            PortDisposition::PortSend => "MACH_MSG_TYPE_PORT_SEND",
            PortDisposition::PortSendOnce => "MACH_MSG_TYPE_PORT_SEND_ONCE",
            PortDisposition::PortName => "MACH_MSG_TYPE_PORT_NAME",
        }
    }
}
