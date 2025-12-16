//! Complete C user stub generation with message packing

use super::CodegenError;
use crate::parser::ast::Direction;
use crate::semantic::{AnalyzedRoutine, AnalyzedSubsystem};

pub struct CUserStubGenerator {
    _user_prefix: String,
}

impl CUserStubGenerator {
    pub fn new() -> Self {
        Self {
            _user_prefix: String::new(),
        }
    }

    /// Generate complete user stub implementation
    pub fn generate(&self, analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Header comment
        output.push_str(&format!(
            "/* User stubs for {} subsystem */\n\n",
            analyzed.name
        ));
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

    /// Generate function parameters including count parameters for arrays
    fn generate_function_parameters(&self, routine: &AnalyzedRoutine) -> Vec<String> {
        let mut params = Vec::new();

        for arg in &routine.routine.args {
            // Check if this is an array parameter by checking the message layout
            let is_array_in_request = routine
                .request_layout
                .fields
                .iter()
                .any(|f| f.name == arg.name && f.is_array);
            let is_array_in_reply = routine
                .reply_layout
                .as_ref()
                .map(|layout| {
                    layout
                        .fields
                        .iter()
                        .any(|f| f.name == arg.name && f.is_array)
                })
                .unwrap_or(false);

            let is_array = is_array_in_request || is_array_in_reply;

            // Get the resolved C type from the message layout
            let base_type = if is_array {
                // For arrays, get the element type from the layout
                let field_in_request = routine
                    .request_layout
                    .fields
                    .iter()
                    .find(|f| f.name == arg.name && f.is_array);
                let field_in_reply = routine.reply_layout.as_ref().and_then(|layout| {
                    layout
                        .fields
                        .iter()
                        .find(|f| f.name == arg.name && f.is_array)
                });

                let field = field_in_request.or(field_in_reply);
                if let Some(f) = field {
                    // The c_type in the layout is the resolved element type
                    f.c_type.trim_end_matches('*').trim().to_string()
                } else {
                    // Fallback to AST type
                    self.get_c_type_for_arg(arg)
                        .trim_end_matches('*')
                        .trim()
                        .to_string()
                }
            } else {
                // For non-arrays, use AST type
                self.get_c_type_for_arg(arg)
            };

            // Generate the main parameter
            let param = match arg.direction {
                Direction::Out | Direction::InOut => {
                    if is_array {
                        // For arrays, always use pointer
                        format!("    {} *{}", base_type, arg.name)
                    } else {
                        format!("    {} *{}", base_type, arg.name)
                    }
                }
                Direction::In | Direction::RequestPort => {
                    if is_array {
                        // For IN arrays, use const pointer
                        format!("    const {} *{}", base_type, arg.name)
                    } else {
                        format!("    {} {}", base_type, arg.name)
                    }
                }
                _ => format!("    {} {}", base_type, arg.name),
            };
            params.push(param);

            // For IN arrays, add count parameter immediately after the array
            if is_array && matches!(arg.direction, Direction::In | Direction::InOut) {
                params.push(format!("    mach_msg_type_number_t {}Cnt", arg.name));
            }

            // For OUT arrays, add count parameter as pointer
            if is_array && matches!(arg.direction, Direction::Out | Direction::InOut) {
                if matches!(arg.direction, Direction::Out) {
                    params.push(format!("    mach_msg_type_number_t *{}Cnt", arg.name));
                }
            }
        }

        params
    }

    /// Generate a single user stub
    fn generate_user_stub(
        &self,
        routine: &AnalyzedRoutine,
        _subsystem_name: &str,
    ) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Function signature
        output.push_str(&format!("kern_return_t {}(\n", routine.user_function_name));

        // Generate parameters including count parameters for arrays
        let params = self.generate_function_parameters(routine);
        if params.is_empty() {
            output.push_str("    void)\n");
        } else {
            for (i, param) in params.iter().enumerate() {
                output.push_str(&param);
                if i < params.len() - 1 {
                    output.push_str(",\n");
                } else {
                    output.push_str(")\n");
                }
            }
        }

        output.push_str("{\n");

        // Message structures
        output.push_str(&self.generate_message_structures(routine)?);

        // Variable declarations
        output.push_str("    mach_msg_return_t msg_result;\n");
        output.push_str("    mach_port_t reply_port;\n\n");

        // Get server port (first argument)
        let server_port = routine
            .routine
            .args
            .first()
            .map(|arg| arg.name.as_str())
            .unwrap_or("MACH_PORT_NULL");

        // Initialize request message
        output.push_str("    /* Initialize request */\n");
        output.push_str(&format!("    Mess.In.Head.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE);\n"));
        output.push_str(&format!("    Mess.In.Head.msgh_size = sizeof(Request);\n"));
        output.push_str(&format!(
            "    Mess.In.Head.msgh_remote_port = {};\n",
            server_port
        ));
        output.push_str("    reply_port = mig_get_reply_port();\n");
        output.push_str("    Mess.In.Head.msgh_local_port = reply_port;\n");
        output.push_str(&format!(
            "    Mess.In.Head.msgh_id = {};\n\n",
            routine.number
        ));

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
    fn generate_message_structures(
        &self,
        routine: &AnalyzedRoutine,
    ) -> Result<String, CodegenError> {
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
                // Type descriptor - use actual field type
                let base_name = field.name.strip_suffix("Type").unwrap_or(&field.name);

                // Find the corresponding data field to get its mach_type
                let data_field = routine
                    .request_layout
                    .fields
                    .iter()
                    .find(|f| f.name == base_name && !f.is_type_descriptor)
                    .unwrap_or(field);

                let mach_const = data_field.mach_type.to_mach_constant();
                let bit_size = data_field.mach_type.bit_size();

                output.push_str(&format!(
                    "    Mess.In.{}.msgt_name = {};\n",
                    field.name, mach_const
                ));
                output.push_str(&format!(
                    "    Mess.In.{}.msgt_size = {};\n",
                    field.name, bit_size
                ));
                output.push_str(&format!("    Mess.In.{}.msgt_number = 1;\n", field.name));
                output.push_str(&format!("    Mess.In.{}.msgt_inline = TRUE;\n", field.name));
                output.push_str(&format!(
                    "    Mess.In.{}.msgt_longform = FALSE;\n",
                    field.name
                ));
                output.push_str(&format!(
                    "    Mess.In.{}.msgt_deallocate = FALSE;\n",
                    field.name
                ));
                output.push_str(&format!("    Mess.In.{}.msgt_unused = 0;\n", field.name));
            } else if field.is_count_field {
                // Count field - use the actual count parameter
                output.push_str(&format!("    Mess.In.{} = {};\n", field.name, field.name));
            } else if !field.is_array {
                // Regular data field
                output.push_str(&format!("    Mess.In.{} = {};\n", field.name, field.name));
            } else {
                // Array data field - for inline arrays, assign pointer directly
                output.push_str(&format!(
                    "    Mess.In.{} = (typeof(Mess.In.{})){};\n",
                    field.name, field.name, field.name
                ));
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
                // Skip type descriptors and RetCode
                if field.is_type_descriptor || field.name == "RetCode" {
                    continue;
                }

                if field.is_count_field {
                    // This is a count field - extract and set the count parameter from type descriptor
                    let array_name = field.name.strip_suffix("Cnt").unwrap_or(&field.name);
                    output.push_str(&format!(
                        "    *{}Cnt = Mess.Out.{}Type.msgt_number;\n",
                        array_name, array_name
                    ));
                } else if field.is_array {
                    // For OUT arrays: Note that proper inline array support requires
                    // message structure changes. For now, this is a placeholder.
                    output.push_str(&format!(
                        "    /* TODO: Implement proper inline array unpacking for {} */\n",
                        field.name
                    ));
                    output.push_str(&format!(
                        "    /* Would need memcpy from inline message data */\n"
                    ));
                } else {
                    // Regular scalar output parameter
                    output.push_str(&format!("    *{} = Mess.Out.{};\n", field.name, field.name));
                }
            }
        }

        output.push('\n');
        Ok(output)
    }

    /// Get C type for an argument
    fn get_c_type_for_arg(&self, arg: &crate::parser::ast::Argument) -> String {
        match &arg.arg_type {
            crate::parser::ast::TypeSpec::Basic(name) => match name.as_str() {
                "int32_t" | "int" => "int32_t".to_string(),
                "mach_port_t" => "mach_port_t".to_string(),
                _ => name.clone(),
            },
            _ => "int32_t".to_string(),
        }
    }
}

impl Default for CUserStubGenerator {
    fn default() -> Self {
        Self::new()
    }
}
