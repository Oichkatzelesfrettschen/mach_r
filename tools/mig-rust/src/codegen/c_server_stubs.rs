//! Complete C server stub generation with message unpacking and demux

use super::CodegenError;
use crate::parser::ast::Direction;
use crate::semantic::{AnalyzedRoutine, AnalyzedSubsystem};

pub struct CServerStubGenerator {
    _server_prefix: String,
}

impl CServerStubGenerator {
    pub fn new() -> Self {
        Self {
            _server_prefix: "_X".to_string(),
        }
    }

    /// Generate complete server stub implementation
    pub fn generate(&self, analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Header comment
        output.push_str(&format!(
            "/* Server stubs for {} subsystem */\n\n",
            analyzed.name
        ));
        output.push_str(&format!("#include \"{}Server.h\"\n", analyzed.name));
        output.push_str("#include <mach/message.h>\n");
        output.push_str("#include <mach/mig_errors.h>\n");
        output.push_str("#include <mach/ndr.h>\n\n");

        // MIG error codes (if not in headers)
        output.push_str("/* MIG error codes */\n");
        output.push_str("#ifndef MIG_NO_REPLY\n");
        output.push_str("#define MIG_NO_REPLY    (-305)\n");
        output.push_str("#define MIG_BAD_ID      (-303)\n");
        output.push_str("#define MIG_BAD_ARGUMENTS (-304)\n");
        output.push_str("#endif\n\n");

        // mig_reply_error_t structure
        output.push_str("/* Reply error structure */\n");
        output.push_str("typedef struct {\n");
        output.push_str("    mach_msg_header_t Head;\n");
        output.push_str("    NDR_record_t NDR;\n");
        output.push_str("    kern_return_t RetCode;\n");
        output.push_str("} mig_reply_error_t;\n\n");

        // NDR record declaration (if not in ndr.h)
        output.push_str("/* NDR record (if not provided by system) */\n");
        output.push_str("#ifndef NDR_RECORD\n");
        output.push_str("#define NDR_RECORD\n");
        output.push_str("const NDR_record_t NDR_record = { 0, 0, 0, 0, 0, 0, 0, 0 };\n");
        output.push_str("#endif\n\n");

        // Forward declarations of user-supplied implementation functions
        output.push_str("/* User-supplied implementation functions */\n");
        for routine in &analyzed.routines {
            output.push_str(&self.generate_impl_prototype(routine)?);
        }
        output.push_str("\n");

        // Generate server stub for each routine
        for routine in &analyzed.routines {
            output.push_str(&self.generate_server_stub(routine, &analyzed.name)?);
            output.push('\n');
        }

        // Generate demux function
        output.push_str(&self.generate_demux(analyzed)?);

        Ok(output)
    }

    /// Generate user implementation function prototype
    fn generate_impl_prototype(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("extern kern_return_t ");
        output.push_str(&format!("{}_impl(\n", routine.name));

        // Generate parameters with layout-based type resolution
        let params = self.generate_impl_parameters(routine);
        if params.is_empty() {
            output.push_str("    void");
        } else {
            for (i, param) in params.iter().enumerate() {
                output.push_str(&param);
                if i < params.len() - 1 {
                    output.push_str(",\n");
                }
            }
        }

        output.push_str(");\n");
        Ok(output)
    }

    /// Generate implementation function parameters with resolved types
    fn generate_impl_parameters(&self, routine: &AnalyzedRoutine) -> Vec<String> {
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

            // Generate the parameter
            let param = match arg.direction {
                Direction::Out | Direction::InOut => {
                    if is_array {
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

    /// Generate a single server stub (_X routine)
    fn generate_server_stub(
        &self,
        routine: &AnalyzedRoutine,
        _subsystem_name: &str,
    ) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Function signature
        output.push_str(&format!(
            "kern_return_t {}(\n",
            routine.server_function_name
        ));
        output.push_str("    mach_msg_header_t *InHeadP,\n");
        output.push_str("    mach_msg_header_t *OutHeadP)\n");
        output.push_str("{\n");

        // Message structures
        output.push_str(&self.generate_server_message_structures(routine)?);

        // Cast messages
        output.push_str("    Request *In0P = (Request *) InHeadP;\n");
        if !routine.is_simple {
            output.push_str("    Reply *OutP = (Reply *) OutHeadP;\n\n");
        } else {
            output.push_str("\n");
        }

        // Validate request message size
        output.push_str("    /* Validate request */\n");
        output.push_str("    if (In0P->Head.msgh_size != sizeof(Request)) {\n");
        output.push_str("        return MIG_BAD_ARGUMENTS;\n");
        output.push_str("    }\n\n");

        // Extract parameters
        output.push_str(&self.generate_parameter_extraction(routine)?);

        // Call user-supplied implementation
        output.push_str(&self.generate_impl_call(routine)?);

        // Handle return code for non-simpleroutines
        if !routine.is_simple {
            output.push_str("\n    if (OutP->RetCode != KERN_SUCCESS) {\n");
            output.push_str("        return MIG_NO_REPLY;\n");
            output.push_str("    }\n\n");

            // Pack reply
            output.push_str(&self.generate_reply_packing(routine)?);
        }

        output.push_str("\n    return KERN_SUCCESS;\n");
        output.push_str("}\n");

        Ok(output)
    }

    /// Generate message structure definitions for server stub
    fn generate_server_message_structures(
        &self,
        routine: &AnalyzedRoutine,
    ) -> Result<String, CodegenError> {
        let mut output = String::new();

        // Request structure - use layout fields directly
        output.push_str("    typedef struct {\n");
        output.push_str("        mach_msg_header_t Head;\n");

        for field in &routine.request_layout.fields {
            output.push_str(&format!("        {} {};\n", field.c_type, field.name));
        }

        output.push_str("    } Request;\n\n");

        // Reply structure (if not simpleroutine)
        if !routine.is_simple {
            if let Some(ref reply_layout) = routine.reply_layout {
                output.push_str("    typedef struct {\n");
                output.push_str("        mach_msg_header_t Head;\n");

                for field in &reply_layout.fields {
                    output.push_str(&format!("        {} {};\n", field.c_type, field.name));
                }

                output.push_str("    } Reply;\n\n");
            }
        }

        Ok(output)
    }

    /// Generate parameter extraction code with validation
    fn generate_parameter_extraction(
        &self,
        routine: &AnalyzedRoutine,
    ) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("    /* Validate and extract parameters */\n");

        // Validate type descriptors and extract array counts from request
        for field in &routine.request_layout.fields {
            if field.is_type_descriptor {
                let base_name = field.name.strip_suffix("Type").unwrap_or(&field.name);

                // Find the corresponding data field
                let data_field = routine
                    .request_layout
                    .fields
                    .iter()
                    .find(|f| f.name == base_name && !f.is_type_descriptor)
                    .unwrap_or(field);

                let mach_const = data_field.mach_type.to_mach_constant();
                let bit_size = data_field.mach_type.bit_size();

                // Validate msgt_name
                output.push_str(&format!(
                    "    if (In0P->{}.msgt_name != {}) {{\n",
                    field.name, mach_const
                ));
                output.push_str("        return MIG_BAD_ARGUMENTS;\n");
                output.push_str("    }\n");

                // Validate msgt_size
                output.push_str(&format!(
                    "    if (In0P->{}.msgt_size != {}) {{\n",
                    field.name, bit_size
                ));
                output.push_str("        return MIG_BAD_ARGUMENTS;\n");
                output.push_str("    }\n");

                // For arrays, extract and validate count
                if data_field.is_array {
                    let count_name = format!("{}Cnt", base_name);
                    output.push_str(&format!(
                        "    mach_msg_type_number_t {} = In0P->{}.msgt_number;\n",
                        count_name, field.name
                    ));

                    // Validate count bounds if there's a maximum
                    if let Some(max) = data_field.max_array_elements {
                        output.push_str(&format!("    if ({} > {}) {{\n", count_name, max));
                        output.push_str(
                            "        return MIG_BAD_ARGUMENTS; /* Array count exceeds maximum */\n",
                        );
                        output.push_str("    }\n");
                    }
                } else {
                    // Non-array: validate msgt_number is 1
                    output.push_str(&format!(
                        "    if (In0P->{}.msgt_number != 1) {{\n",
                        field.name
                    ));
                    output.push_str("        return MIG_BAD_ARGUMENTS;\n");
                    output.push_str("    }\n");
                }

                // Validate inline flag
                output.push_str(&format!("    if (!In0P->{}.msgt_inline) {{\n", field.name));
                output.push_str(
                    "        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */\n",
                );
                output.push_str("    }\n");

                output.push('\n');
            }
        }

        // Declare variables for OUT parameters with resolved types
        if let Some(ref reply_layout) = routine.reply_layout {
            for field in &reply_layout.fields {
                // Skip type descriptors, RetCode, and count fields
                if field.is_type_descriptor || field.name == "RetCode" || field.is_count_field {
                    continue;
                }

                // Declare local variable for OUT parameter
                // For arrays, use pointer type; for scalars, use value type
                if field.is_array {
                    // For OUT arrays, declare as pointer (matches impl signature)
                    output.push_str(&format!("    {} {};\n", field.c_type, field.name));
                    // Also declare count variable for OUT arrays
                    output.push_str(&format!("    mach_msg_type_number_t {}Cnt;\n", field.name));
                } else {
                    output.push_str(&format!("    {} {};\n", field.c_type, field.name));
                }
            }
        }

        output.push('\n');
        Ok(output)
    }

    /// Generate call to user implementation function
    fn generate_impl_call(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("    /* Call user implementation */\n");

        if !routine.is_simple {
            output.push_str("    OutP->RetCode = ");
        }

        output.push_str(&format!("{}_impl(\n", routine.name));

        // Build argument list with count parameters for arrays
        let mut first = true;
        for arg in &routine.routine.args {
            // Check if this argument is an array
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

            if !first {
                output.push_str(",\n");
            }
            first = false;

            output.push_str("        ");

            match arg.direction {
                Direction::In | Direction::RequestPort => {
                    // IN parameters: pass from message
                    output.push_str(&format!("In0P->{}", arg.name));
                }
                Direction::Out => {
                    // OUT parameters: pass address of local variable
                    output.push_str(&format!("&{}", arg.name));
                }
                Direction::InOut => {
                    // INOUT: pass address from message
                    output.push_str(&format!("&In0P->{}", arg.name));
                }
                _ => {
                    output.push_str(&format!("In0P->{}", arg.name));
                }
            }

            // Add count parameter for arrays
            if is_array {
                if matches!(arg.direction, Direction::In | Direction::InOut) {
                    // IN array: pass count value
                    output.push_str(",\n        ");
                    output.push_str(&format!("{}Cnt", arg.name));
                }
                if matches!(arg.direction, Direction::Out) {
                    // OUT array: pass count pointer
                    output.push_str(",\n        ");
                    output.push_str(&format!("&{}Cnt", arg.name));
                }
            }
        }

        if !routine.routine.args.is_empty() {
            output.push('\n');
        }

        output.push_str("    );\n");

        // For simpleroutines, we don't check return code
        if routine.is_simple {
            output.push_str("\n    return MIG_NO_REPLY;  /* Simpleroutine: no reply */\n");
        }

        Ok(output)
    }

    /// Generate reply message packing code
    fn generate_reply_packing(&self, routine: &AnalyzedRoutine) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str("    /* Pack reply */\n");
        output.push_str("    OutP->Head.msgh_size = sizeof(Reply);\n\n");

        // Pack OUT and INOUT parameters from reply layout
        if let Some(ref reply_layout) = routine.reply_layout {
            for field in &reply_layout.fields {
                if field.is_type_descriptor && field.name != "RetCodeType" {
                    let base_name = field.name.strip_suffix("Type").unwrap_or(&field.name);

                    // Find the corresponding data field
                    let data_field = reply_layout
                        .fields
                        .iter()
                        .find(|f| f.name == base_name && !f.is_type_descriptor)
                        .unwrap_or(field);

                    let mach_const = data_field.mach_type.to_mach_constant();
                    let bit_size = data_field.mach_type.bit_size();

                    // Pack type descriptor
                    output.push_str(&format!(
                        "    OutP->{}.msgt_name = {};\n",
                        field.name, mach_const
                    ));
                    output.push_str(&format!(
                        "    OutP->{}.msgt_size = {};\n",
                        field.name, bit_size
                    ));

                    // For arrays, use the count variable; for scalars, use 1
                    if data_field.is_array {
                        let count_name = format!("{}Cnt", base_name);
                        output.push_str(&format!(
                            "    OutP->{}.msgt_number = {}; /* Array count */\n",
                            field.name, count_name
                        ));
                    } else {
                        output.push_str(&format!("    OutP->{}.msgt_number = 1;\n", field.name));
                    }

                    output.push_str(&format!("    OutP->{}.msgt_inline = TRUE;\n", field.name));
                    output.push_str(&format!(
                        "    OutP->{}.msgt_longform = FALSE;\n",
                        field.name
                    ));
                    output.push_str(&format!(
                        "    OutP->{}.msgt_deallocate = FALSE;\n",
                        field.name
                    ));
                    output.push_str(&format!("    OutP->{}.msgt_unused = 0;\n\n", field.name));
                } else if !field.is_type_descriptor
                    && !field.is_count_field
                    && field.name != "RetCode"
                {
                    // Pack data field (for OUT parameters, use local variable)
                    if field.is_array {
                        output.push_str(&format!(
                            "    OutP->{} = {}; /* TODO: handle array packing */\n",
                            field.name, field.name
                        ));
                    } else {
                        output.push_str(&format!("    OutP->{} = {};\n", field.name, field.name));
                    }
                    output.push('\n');
                }
            }
        }

        Ok(output)
    }

    /// Generate demux function
    fn generate_demux(&self, analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
        let mut output = String::new();

        output.push_str(&format!(
            "/* Demux function for {} subsystem */\n",
            analyzed.name
        ));
        output.push_str("#ifdef __cplusplus\n");
        output.push_str("extern \"C\" {\n");
        output.push_str("#endif\n\n");

        output.push_str(&format!("boolean_t {}_server(\n", analyzed.name));
        output.push_str("    mach_msg_header_t *InHeadP,\n");
        output.push_str("    mach_msg_header_t *OutHeadP)\n");
        output.push_str("{\n");

        output.push_str("    mach_msg_id_t msgid;\n");
        output.push_str("    kern_return_t check_result;\n\n");

        // Initialize reply header
        output.push_str("    /* Initialize reply header */\n");
        output.push_str("    OutHeadP->msgh_bits = MACH_MSGH_BITS(\n");
        output.push_str("        MACH_MSGH_BITS_REMOTE(InHeadP->msgh_bits),\n");
        output.push_str("        0);\n");
        output.push_str("    OutHeadP->msgh_remote_port = InHeadP->msgh_reply_port;\n");
        output.push_str("    OutHeadP->msgh_size = sizeof(mig_reply_error_t);\n");
        output.push_str("    OutHeadP->msgh_local_port = MACH_PORT_NULL;\n");
        output.push_str("    OutHeadP->msgh_id = InHeadP->msgh_id + 100;\n\n");

        output.push_str("    msgid = InHeadP->msgh_id;\n\n");

        // Dispatch based on message ID
        output.push_str(&format!("    /* Dispatch to appropriate handler */\n"));
        output.push_str(&format!(
            "    if (msgid >= {} && msgid < {} + {}) {{\n",
            analyzed.base,
            analyzed.base,
            analyzed.routines.len()
        ));
        output.push_str(&format!("        switch (msgid - {}) {{\n", analyzed.base));

        for (i, routine) in analyzed.routines.iter().enumerate() {
            output.push_str(&format!(
                "            case {}:  /* {} */\n",
                i, routine.name
            ));
            output.push_str(&format!(
                "                check_result = {}(InHeadP, OutHeadP);\n",
                routine.server_function_name
            ));

            if routine.is_simple {
                output.push_str("                if (check_result == MIG_NO_REPLY) {\n");
                output.push_str(
                    "                    return FALSE;  /* No reply for simpleroutine */\n",
                );
                output.push_str("                }\n");
            }

            output.push_str("                if (check_result == KERN_SUCCESS) {\n");
            output.push_str("                    return TRUE;\n");
            output.push_str("                }\n");
            output.push_str("                break;\n\n");
        }

        output.push_str("            default:\n");
        output.push_str("                break;\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        // Unknown message - send error reply
        output.push_str("    /* Unknown message ID - send error reply */\n");
        output.push_str("    ((mig_reply_error_t *)OutHeadP)->NDR = NDR_record;\n");
        output.push_str("    ((mig_reply_error_t *)OutHeadP)->RetCode = MIG_BAD_ID;\n\n");

        output.push_str("    return FALSE;\n");
        output.push_str("}\n\n");

        output.push_str("#ifdef __cplusplus\n");
        output.push_str("}\n");
        output.push_str("#endif\n");

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

impl Default for CServerStubGenerator {
    fn default() -> Self {
        Self::new()
    }
}
