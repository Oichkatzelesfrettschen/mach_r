//! Message layout calculation for Mach IPC messages

use super::types::{TypeResolver, TypeSize};
use crate::parser::ast::{Argument, Direction, Routine};

/// Message layout information
#[derive(Debug, Clone)]
pub struct MessageLayout {
    /// Total header size (mach_msg_header_t)
    pub header_size: usize,
    /// Body size (fixed part)
    pub body_fixed_size: usize,
    /// Body size (variable part)
    pub body_variable_max: usize,
    /// Total minimum size
    pub min_size: usize,
    /// Total maximum size
    pub max_size: usize,
    /// Fields in message body
    pub fields: Vec<MessageField>,
}

/// A field in the message body
#[derive(Debug, Clone)]
pub struct MessageField {
    /// Field name
    pub name: String,
    /// C type
    pub c_type: String,
    /// Mach message type
    pub mach_type: super::types::MachMsgType,
    /// Offset from message start (if fixed)
    pub offset: Option<usize>,
    /// Field size
    pub size: FieldSize,
    /// Is this a type descriptor field?
    pub is_type_descriptor: bool,
    /// Is this an array type?
    pub is_array: bool,
    /// Is this a count field for an array?
    pub is_count_field: bool,
    /// Maximum array elements (for variable arrays)
    pub max_array_elements: Option<u32>,
}

/// Field size information
#[derive(Debug, Clone, Copy)]
pub enum FieldSize {
    /// Fixed size
    Fixed(usize),
    /// Variable size
    Variable { max: usize },
}

/// Message layout calculator
pub struct MessageLayoutCalculator<'a> {
    type_resolver: &'a TypeResolver,
}

impl<'a> MessageLayoutCalculator<'a> {
    /// Create a new layout calculator
    pub fn new(type_resolver: &'a TypeResolver) -> Self {
        Self { type_resolver }
    }

    /// Calculate layout for a routine's request message
    pub fn calculate_request_layout(&self, routine: &Routine) -> MessageLayout {
        let mut layout = MessageLayout {
            header_size: 24, // sizeof(mach_msg_header_t) = 24 bytes
            body_fixed_size: 0,
            body_variable_max: 0,
            min_size: 24,
            max_size: 24,
            fields: Vec::new(),
        };

        let mut current_offset = 24; // Start after header

        // Add fields for each IN or INOUT argument
        for arg in &routine.args {
            match arg.direction {
                Direction::In | Direction::InOut => {
                    if let Some(field) = self.create_message_field(arg, current_offset) {
                        // Add type descriptor (8 bytes: mach_msg_type_t)
                        layout.fields.push(MessageField {
                            name: format!("{}Type", arg.name),
                            c_type: "mach_msg_type_t".to_string(),
                            mach_type: super::types::MachMsgType::TypeInteger32, // Descriptor itself is not sent, this is placeholder
                            offset: Some(current_offset),
                            size: FieldSize::Fixed(8),
                            is_type_descriptor: true,
                            is_array: false,
                            is_count_field: false,
                            max_array_elements: None,
                        });
                        current_offset += 8;
                        layout.body_fixed_size += 8;

                        // For variable arrays, add count field
                        if field.is_array {
                            layout.fields.push(MessageField {
                                name: format!("{}Cnt", arg.name),
                                c_type: "mach_msg_type_number_t".to_string(),
                                mach_type: super::types::MachMsgType::TypeInteger32,
                                offset: Some(current_offset),
                                size: FieldSize::Fixed(4),
                                is_type_descriptor: false,
                                is_array: false,
                                is_count_field: true,
                                max_array_elements: None,
                            });
                            current_offset += 4;
                            layout.body_fixed_size += 4;
                        }

                        // Add data field
                        match field.size {
                            FieldSize::Fixed(size) => {
                                layout.body_fixed_size += size;
                                current_offset += size;
                            }
                            FieldSize::Variable { max } => {
                                layout.body_variable_max += max;
                            }
                        }

                        layout.fields.push(field);
                    }
                }
                _ => {} // Skip OUT parameters in request
            }
        }

        layout.min_size = layout.header_size + layout.body_fixed_size;
        layout.max_size = layout.min_size + layout.body_variable_max;

        layout
    }

    /// Calculate layout for a routine's reply message
    pub fn calculate_reply_layout(&self, routine: &Routine) -> MessageLayout {
        let mut layout = MessageLayout {
            header_size: 24,
            body_fixed_size: 0,
            body_variable_max: 0,
            min_size: 24,
            max_size: 24,
            fields: Vec::new(),
        };

        let mut current_offset = 24;

        // Always include return code
        layout.fields.push(MessageField {
            name: "RetCodeType".to_string(),
            c_type: "mach_msg_type_t".to_string(),
            mach_type: super::types::MachMsgType::TypeInteger32,
            offset: Some(current_offset),
            size: FieldSize::Fixed(8),
            is_type_descriptor: true,
            is_array: false,
            is_count_field: false,
            max_array_elements: None,
        });
        current_offset += 8;

        layout.fields.push(MessageField {
            name: "RetCode".to_string(),
            c_type: "kern_return_t".to_string(),
            mach_type: super::types::MachMsgType::TypeInteger32,
            offset: Some(current_offset),
            size: FieldSize::Fixed(4),
            is_type_descriptor: false,
            is_array: false,
            is_count_field: false,
            max_array_elements: None,
        });
        current_offset += 4;
        layout.body_fixed_size += 12; // RetCodeType (8) + RetCode (4)

        // Add fields for each OUT or INOUT argument
        for arg in &routine.args {
            match arg.direction {
                Direction::Out | Direction::InOut => {
                    if let Some(field) = self.create_message_field(arg, current_offset) {
                        // Add type descriptor
                        layout.fields.push(MessageField {
                            name: format!("{}Type", arg.name),
                            c_type: "mach_msg_type_t".to_string(),
                            mach_type: super::types::MachMsgType::TypeInteger32, // Placeholder
                            offset: Some(current_offset),
                            size: FieldSize::Fixed(8),
                            is_type_descriptor: true,
                            is_array: false,
                            is_count_field: false,
                            max_array_elements: None,
                        });
                        current_offset += 8;
                        layout.body_fixed_size += 8;

                        // For variable arrays, add count field
                        if field.is_array {
                            layout.fields.push(MessageField {
                                name: format!("{}Cnt", arg.name),
                                c_type: "mach_msg_type_number_t".to_string(),
                                mach_type: super::types::MachMsgType::TypeInteger32,
                                offset: Some(current_offset),
                                size: FieldSize::Fixed(4),
                                is_type_descriptor: false,
                                is_array: false,
                                is_count_field: true,
                                max_array_elements: None,
                            });
                            current_offset += 4;
                            layout.body_fixed_size += 4;
                        }

                        // Add data field
                        match field.size {
                            FieldSize::Fixed(size) => {
                                layout.body_fixed_size += size;
                                current_offset += size;
                            }
                            FieldSize::Variable { max } => {
                                layout.body_variable_max += max;
                            }
                        }

                        layout.fields.push(field);
                    }
                }
                _ => {} // Skip IN parameters in reply
            }
        }

        layout.min_size = layout.header_size + layout.body_fixed_size;
        layout.max_size = layout.min_size + layout.body_variable_max;

        layout
    }

    /// Create a message field from an argument
    fn create_message_field(&self, arg: &Argument, offset: usize) -> Option<MessageField> {
        // Determine if this is an array type
        let (is_array, max_elements) = match &arg.arg_type {
            crate::parser::ast::TypeSpec::Array { size, .. } => {
                let max = match size {
                    crate::parser::ast::ArraySize::Fixed(n) => Some(*n),
                    crate::parser::ast::ArraySize::VariableWithMax(n) => Some(*n),
                    crate::parser::ast::ArraySize::Variable => None,
                };
                (true, max)
            }
            crate::parser::ast::TypeSpec::Basic(name) => {
                // Check if the type itself is an array type
                if let Ok(resolved) = self.type_resolver.lookup(name) {
                    if resolved.is_array {
                        let max = match resolved.array_size {
                            Some(crate::parser::ast::ArraySize::Fixed(n)) => Some(n),
                            Some(crate::parser::ast::ArraySize::VariableWithMax(n)) => Some(n),
                            _ => None,
                        };
                        (true, max)
                    } else {
                        (false, None)
                    }
                } else {
                    (false, None)
                }
            }
            _ => (false, None),
        };

        // Get the base type name for C type lookup
        let type_name = match &arg.arg_type {
            crate::parser::ast::TypeSpec::Basic(name) => name.clone(),
            crate::parser::ast::TypeSpec::Array { element, .. } => {
                if let crate::parser::ast::TypeSpec::Basic(name) = element.as_ref() {
                    name.clone()
                } else {
                    "void".to_string()
                }
            }
            _ => "void".to_string(),
        };

        // Look up type
        let c_type = self
            .type_resolver
            .get_c_type(&type_name)
            .unwrap_or_else(|| type_name.clone());

        let size = match self.type_resolver.get_size(&type_name) {
            Some(TypeSize::Fixed(s)) => FieldSize::Fixed(s),
            Some(TypeSize::Variable { max }) => FieldSize::Variable { max },
            _ => FieldSize::Fixed(4), // Default to 4 bytes
        };

        // Get Mach message type
        let mach_type = self
            .type_resolver
            .lookup(&type_name)
            .map(|t| t.mach_type)
            .unwrap_or(super::types::MachMsgType::TypeInteger32);

        Some(MessageField {
            name: arg.name.clone(),
            c_type,
            mach_type,
            offset: Some(offset),
            size,
            is_type_descriptor: false,
            is_array,
            is_count_field: false,
            max_array_elements: max_elements,
        })
    }
}

impl FieldSize {
    /// Get the byte count (max for variable)
    pub fn bytes(&self) -> usize {
        match self {
            FieldSize::Fixed(s) => *s,
            FieldSize::Variable { max } => *max,
        }
    }
}
