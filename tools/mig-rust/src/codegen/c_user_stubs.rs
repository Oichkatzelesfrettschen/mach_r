//! Complete C user stub generation with message packing

use crate::semantic::{AnalyzedSubsystem, AnalyzedRoutine};
use crate::parser::ast::Direction;
use super::CodegenError;

pub struct CUserStubGenerator {
    user_prefix: String,
}

impl CUserStubGenerator {
    pub fn new() -> Self {
        Self {
            user_prefix: String::new(),
        }
    }

    /// Generate complete user stub implementation
    pub fn generate(&self, analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Header comment
        output.push_str(&format!("/* User stubs for {} subsystem */\n\n", analyzed.name));
        output.push_str(&format!("#include \"{}.h\"\n", analyzed.name));
        output.push_str("#include <mach/message.h>\n");
        output.push_str("#include <mach/mach_init.h>\n");
        output.push_str("#include <mach/mig_errors.h>\n\n");

        // Helper for reply port
        output.push_str("/* Get reply port (simplified) */\n");
        output.push_str("static mach_port_t mig_get_reply_port(void) {\n");
        output.push_str("    return mach_reply_port();\n");
        output.push_str("}\n\n");

        // Generate stub for each routine
        for routine in &analyzed.routines {
            output.push_str(&self.generate_user_stub(routine, &analyzed.name)?);
            output.push('\n');
        }

        Ok(output)
    }

    /// Generate a single user stub
    fn generate_user_stub(&self, routine: &AnalyzedRoutine, subsystem_name: &str) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Function signature
        output.push_str(&format!("kern_return_t {}(\n", routine.user_function_name));

        // Parameters
        for (i, arg) in routine.routine.args.iter().enumerate() {
            let c_type = self.get_c_type_for_arg(arg);
            let param = match arg.direction {
                Direction::Out | Direction::InOut => {
                    format!("    {} *{}", c_type, arg.name)
                }
                Direction::In | Direction::RequestPort => {
                    format!("    {} {}", c_type, arg.name)
                }
                _ => format!("    {} {}", c_type, arg.name),
            };

            output.push_str(&param);
            if i < routine.routine.args.len() - 1 {
                output.push_str(",\n");
            } else {
                output.push_str(")\n");
            }
        }

        if routine.routine.args.is_empty() {
            output.push_str("    void)\n");
        }

        output.push_str("{\n");

        // Message structures
        output.push_str(&self.generate_message_structures(routine)?);

        // Variable declarations
        output.push_str("    mach_msg_return_t msg_result;\n");
        output.push_str("    mach_port_t reply_port;\n\n");

        // Get server port (first argument)
        let server_port = routine.routine.args.first()
            .map(|arg| arg.name.as_str())
            .unwrap_or("MACH_PORT_NULL");

        // Initialize request message
        output.push_str("    /* Initialize request */\n");
        output.push_str(&format!("    Mess.In.Head.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE);\n"));
        output.push_str(&format!("    Mess.In.Head.msgh_size = sizeof(Request);\n"));
        output.push_str(&format!("    Mess.In.Head.msgh_remote_port = {};\n", server_port));
        output.push_str("    reply_port = mig_get_reply_port();\n");
        output.push_str("    Mess.In.Head.msgh_local_port = reply_port;\n");
        output.push_str(&format!("    Mess.In.Head.msgh_id = {};\n\n", routine.number));

        // Pack input arguments
        output.push_str(&self.generate_input_packing(routine)?);

        // Make IPC call
        if routine.is_simple {
            // Simpleroutine: send only
            output.push_str("    /* Send message (no reply) */\n");
            output.push_str("    msg_result = mach_msg(\n");
            output.push_str("        &Mess.In.Head,\n");
            output.push_str("        MACH_SEND_MSG,\n");
            output.push_str("        sizeof(Request),\n");
            output.push_str("        0,\n");
            output.push_str("        MACH_PORT_NULL,\n");
            output.push_str("        MACH_MSG_TIMEOUT_NONE,\n");
            output.push_str("        MACH_PORT_NULL);\n\n");
            output.push_str("    return msg_result;\n");
        } else {
            // Routine: send and receive
            output.push_str("    /* Send request and receive reply */\n");
            output.push_str("    msg_result = mach_msg(\n");
            output.push_str("        &Mess.In.Head,\n");
            output.push_str("        MACH_SEND_MSG | MACH_RCV_MSG,\n");
            output.push_str("        sizeof(Request),\n");
            output.push_str("        sizeof(Reply),\n");
            output.push_str("        reply_port,\n");
            output.push_str("        MACH_MSG_TIMEOUT_NONE,\n");
            output.push_str("        MACH_PORT_NULL);\n\n");

            output.push_str("    if (msg_result != MACH_MSG_SUCCESS) {\n");
            output.push_str("        return msg_result;\n");
            output.push_str("    }\n\n");

            // Unpack output arguments
            output.push_str(&self.generate_output_unpacking(routine)?);

            output.push_str("    return Mess.Out.RetCode;\n");
        }

        output.push_str("}\n");

        Ok(output)
    }

    /// Generate message structure definitions
    fn generate_message_structures(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Request structure
        output.push_str("    typedef struct {\n");
        output.push_str("        mach_msg_header_t Head;\n");

        // Add fields from request layout
        for field in &routine.request_layout.fields {
            output.push_str(&format!("        {} {};\n", field.c_type, field.name));
        }

        output.push_str("    } Request;\n\n");

        // Reply structure (if not simpleroutine)
        if !routine.is_simple {
            if let Some(ref reply_layout) = routine.reply_layout {
                output.push_str("    typedef struct {\n");
                output.push_str("        mach_msg_header_t Head;\n");

                // Add fields from reply layout
                for field in &reply_layout.fields {
                    output.push_str(&format!("        {} {};\n", field.c_type, field.name));
                }

                output.push_str("    } Reply;\n\n");
            }
        }

        // Union for alignment
        output.push_str("    union {\n");
        output.push_str("        Request In;\n");
        if !routine.is_simple {
            output.push_str("        Reply Out;\n");
        }
        output.push_str("    } Mess;\n\n");

        Ok(output)
    }

    /// Generate input parameter packing code
    fn generate_input_packing(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("    /* Pack input parameters */\n");

        for field in &routine.request_layout.fields {
            if field.is_type_descriptor {
                // Type descriptor
                let base_name = field.name.strip_suffix("Type").unwrap_or(&field.name);
                output.push_str(&format!("    Mess.In.{}.msgt_name = MACH_MSG_TYPE_INTEGER_32;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_size = 32;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_number = 1;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_inline = TRUE;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_longform = FALSE;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_deallocate = FALSE;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_unused = 0;\n", field.name));
            } else if field.is_count_field {
                // Count field - for now, set to max or 0 (TODO: get actual count from parameter)
                let count = field.max_array_elements.unwrap_or(0);
                output.push_str(&format!("    Mess.In.{} = {}; /* TODO: use actual array count */\n", field.name, count));
            } else if !field.is_array {
                // Regular data field
                output.push_str(&format!("    Mess.In.{} = {};\n", field.name, field.name));
            } else {
                // Array data field
                output.push_str(&format!("    Mess.In.{} = {}; /* TODO: handle array data */\n", field.name, field.name));
            }
        }

        output.push('\n');
        Ok(output)
    }

    /// Generate output parameter unpacking code
    fn generate_output_unpacking(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("    /* Unpack output parameters */\n");

        if let Some(ref reply_layout) = routine.reply_layout {
            for field in &reply_layout.fields {
                // Skip type descriptors, RetCode, and count fields (for now)
                if !field.is_type_descriptor && field.name != "RetCode" && !field.is_count_field {
                    if field.is_array {
                        output.push_str(&format!("    *{} = Mess.Out.{}; /* TODO: handle array unpacking */\n", field.name, field.name));
                    } else {
                        output.push_str(&format!("    *{} = Mess.Out.{};\n", field.name, field.name));
                    }
                }
            }
        }

        output.push('\n');
        Ok(output)
    }

    /// Get C type for an argument
    fn get_c_type_for_arg(&self, arg: &crate::parser::ast::Argument) -> String {
        match &arg.arg_type {
            crate::parser::ast::TypeSpec::Basic(name) => {
                match name.as_str() {
                    "int32_t" | "int" => "int32_t".to_string(),
                    "mach_port_t" => "mach_port_t".to_string(),
                    _ => name.clone(),
                }
            }
            _ => "int32_t".to_string(),
        }
    }
}

impl Default for CUserStubGenerator {
    fn default() -> Self {
        Self::new()
    }
}
