/// C code generator for legacy Mach compatibility

use super::{CodeGenerator, CodegenError};
use crate::parser::ast::*;

pub struct CCodeGenerator {
    user_prefix: String,
    server_prefix: String,
}

impl CCodeGenerator {
    pub fn new() -> Self {
        Self {
            user_prefix: String::new(),
            server_prefix: "_X".to_string(),
        }
    }
}

impl Default for CCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator for CCodeGenerator {
    fn generate_user_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Header guard
        let guard = format!("_{}_user_", subsystem.name.to_uppercase());
        output.push_str(&format!("#ifndef {}\n", guard));
        output.push_str(&format!("#define {}\n\n", guard));

        // Includes
        output.push_str("#include <mach/kern_return.h>\n");
        output.push_str("#include <mach/port.h>\n");
        output.push_str("#include <mach/message.h>\n\n");

        // Function prototypes
        for statement in &subsystem.statements {
            if let Statement::Routine(routine) | Statement::SimpleRoutine(routine) = statement {
                output.push_str(&self.generate_user_prototype(routine)?);
                output.push('\n');
            }
        }

        output.push_str(&format!("\n#endif /* {} */\n", guard));

        Ok(output)
    }

    fn generate_user_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str(&format!("/* User stubs for {} */\n\n", subsystem.name));
        output.push_str(&format!("#include \"{}.h\"\n\n", subsystem.name));

        // TODO: Generate user stub implementations

        Ok(output)
    }

    fn generate_server_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        let guard = format!("_{}_server_", subsystem.name.to_uppercase());
        output.push_str(&format!("#ifndef {}\n", guard));
        output.push_str(&format!("#define {}\n\n", guard));

        output.push_str("#include <mach/kern_return.h>\n");
        output.push_str("#include <mach/port.h>\n");
        output.push_str("#include <mach/message.h>\n\n");

        // Server function prototypes
        for statement in &subsystem.statements {
            if let Statement::Routine(routine) | Statement::SimpleRoutine(routine) = statement {
                output.push_str(&self.generate_server_prototype(routine)?);
                output.push('\n');
            }
        }

        // Demux function
        output.push_str(&format!(
            "\nboolean_t {}_server(\n    mach_msg_header_t *InHeadP,\n    mach_msg_header_t *OutHeadP);\n",
            subsystem.name
        ));

        output.push_str(&format!("\n#endif /* {} */\n", guard));

        Ok(output)
    }

    fn generate_server_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str(&format!("/* Server stubs for {} */\n\n", subsystem.name));
        output.push_str(&format!("#include \"{}Server.h\"\n\n", subsystem.name));

        // TODO: Generate server stub implementations and demux function

        Ok(output)
    }
}

impl CCodeGenerator {
    /// Map MIG type to C type
    fn map_type(&self, type_spec: &TypeSpec) -> String {
        match type_spec {
            TypeSpec::Basic(name) => {
                // Map common Mach types
                match name.as_str() {
                    "mach_port_t" => "mach_port_t".to_string(),
                    "int32_t" => "int32_t".to_string(),
                    "int" => "int".to_string(),
                    "uint32_t" => "uint32_t".to_string(),
                    _ => name.clone(),
                }
            }
            TypeSpec::Array { element, .. } => {
                format!("{}*", self.map_type(element))
            }
            TypeSpec::Pointer(inner) => {
                format!("{}*", self.map_type(inner))
            }
            TypeSpec::Struct(_) => "void*".to_string(), // TODO: proper struct handling
            TypeSpec::StructArray { element, .. } => {
                format!("{}*", self.map_type(element))
            }
            TypeSpec::CString { .. } => "char*".to_string(),
        }
    }

    fn generate_user_prototype(&self, routine: &Routine) -> Result<String, CodegenError> {
        let mut proto = String::new();

        proto.push_str("kern_return_t ");
        proto.push_str(&self.user_prefix);
        proto.push_str(&routine.name);
        proto.push_str("(\n");

        // Generate parameters from arguments
        let mut first = true;
        for arg in &routine.args {
            if !first {
                proto.push_str(",\n");
            }
            first = false;

            proto.push_str("    ");

            // Add direction qualifier
            match arg.direction {
                Direction::Out | Direction::InOut => {
                    let c_type = self.map_type(&arg.arg_type);
                    // Out parameters are always pointers
                    if c_type.ends_with('*') {
                        proto.push_str(&c_type);
                    } else {
                        proto.push_str(&format!("{}*", c_type));
                    }
                }
                Direction::In => {
                    proto.push_str(&self.map_type(&arg.arg_type));
                }
                Direction::RequestPort | Direction::ReplyPort
                | Direction::SReplyPort | Direction::UReplyPort => {
                    proto.push_str("mach_port_t");
                }
                Direction::WaitTime => {
                    proto.push_str("mach_msg_timeout_t");
                }
                Direction::MsgOption => {
                    proto.push_str("mach_msg_option_t");
                }
                Direction::MsgSeqno => {
                    proto.push_str("mach_port_seqno_t");
                }
            }

            proto.push(' ');
            proto.push_str(&arg.name);
        }

        if routine.args.is_empty() {
            proto.push_str("    void");
        }

        proto.push_str(");\n");

        Ok(proto)
    }

    fn generate_server_prototype(&self, routine: &Routine) -> Result<String, CodegenError> {
        let mut proto = String::new();

        proto.push_str("kern_return_t ");
        proto.push_str(&self.server_prefix);
        proto.push_str(&routine.name);
        proto.push_str("(\n");
        proto.push_str("    mach_msg_header_t *InHeadP,\n");
        proto.push_str("    mach_msg_header_t *OutHeadP);\n");

        Ok(proto)
    }
}
