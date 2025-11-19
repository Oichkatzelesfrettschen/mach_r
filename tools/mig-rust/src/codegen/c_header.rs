//! C header file generation for MIG subsystems
//!
//! Generates .h files with function prototypes and type definitions.

use crate::semantic::AnalyzedSubsystem;
use super::CodegenError;

/// Generate C header file for user-side stubs
pub fn generate_user_header(analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
    let mut output = String::new();
    let guard_name = format!("_{}_user_", analyzed.name.to_uppercase());

    // Include guard start
    output.push_str(&format!("#ifndef {}\n", guard_name));
    output.push_str(&format!("#define {}\n\n", guard_name));

    // C++ compatibility
    output.push_str("#ifdef __cplusplus\n");
    output.push_str("extern \"C\" {\n");
    output.push_str("#endif\n\n");

    // Header comment
    output.push_str(&format!("/* User header for {} subsystem */\n\n", analyzed.name));

    // Required Mach headers
    output.push_str("#include <mach/kern_return.h>\n");
    output.push_str("#include <mach/port.h>\n");
    output.push_str("#include <mach/message.h>\n");
    output.push_str("#include <mach/std_types.h>\n\n");

    // Fallback typedef for mach_msg_type_number_t
    output.push_str("/* Fallback for mach_msg_type_number_t if not in system headers */\n");
    output.push_str("#ifndef mach_msg_type_number_t\n");
    output.push_str("typedef uint32_t mach_msg_type_number_t;\n");
    output.push_str("#endif\n\n");

    // Function prototypes
    output.push_str("/* User-side function prototypes */\n\n");

    for routine in &analyzed.routines {
        // Function comment
        output.push_str(&format!("/* Routine {} */\n", routine.name));

        // Return type
        output.push_str("extern kern_return_t ");
        output.push_str(&routine.user_function_name);
        output.push_str("(\n");

        // Parameters (including count parameters for arrays)
        let params = generate_function_params_for_header_with_layout(routine);
        if params.is_empty() {
            output.push_str("    void");
        } else {
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    output.push_str(",\n");
                }
                output.push_str(&param);
            }
        }

        output.push_str(");\n\n");
    }

    // C++ compatibility end
    output.push_str("#ifdef __cplusplus\n");
    output.push_str("}\n");
    output.push_str("#endif\n\n");

    // Include guard end
    output.push_str(&format!("#endif /* {} */\n", guard_name));

    Ok(output)
}

/// Generate C header file for server-side stubs
pub fn generate_server_header(analyzed: &AnalyzedSubsystem) -> Result<String, CodegenError> {
    let mut output = String::new();
    let guard_name = format!("_{}_server_", analyzed.name.to_uppercase());

    // Include guard start
    output.push_str(&format!("#ifndef {}\n", guard_name));
    output.push_str(&format!("#define {}\n\n", guard_name));

    // C++ compatibility
    output.push_str("#ifdef __cplusplus\n");
    output.push_str("extern \"C\" {\n");
    output.push_str("#endif\n\n");

    // Header comment
    output.push_str(&format!("/* Server header for {} subsystem */\n\n", analyzed.name));

    // Required Mach headers
    output.push_str("#include <mach/kern_return.h>\n");
    output.push_str("#include <mach/port.h>\n");
    output.push_str("#include <mach/message.h>\n");
    output.push_str("#include <mach/std_types.h>\n");
    output.push_str("#include <mach/boolean.h>\n\n");

    // Fallback typedef for mach_msg_type_number_t
    output.push_str("/* Fallback for mach_msg_type_number_t if not in system headers */\n");
    output.push_str("#ifndef mach_msg_type_number_t\n");
    output.push_str("typedef uint32_t mach_msg_type_number_t;\n");
    output.push_str("#endif\n\n");

    // Server implementation function prototypes
    output.push_str("/* Server implementation functions (provided by user) */\n\n");

    for routine in &analyzed.routines {
        // Function comment
        output.push_str(&format!("/* Routine {} implementation */\n", routine.name));

        // Return type
        output.push_str("extern kern_return_t ");
        output.push_str(&format!("{}_impl", routine.name));
        output.push_str("(\n");

        // Parameters (including count parameters for arrays)
        let params = generate_function_params_for_header_with_layout(routine);
        if params.is_empty() {
            output.push_str("    void");
        } else {
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    output.push_str(",\n");
                }
                output.push_str(&param);
            }
        }

        output.push_str(");\n\n");
    }

    // Demux function
    output.push_str("/* Demux function */\n");
    output.push_str(&format!("extern boolean_t {}_server(\n", analyzed.name));
    output.push_str("    mach_msg_header_t *InHeadP,\n");
    output.push_str("    mach_msg_header_t *OutHeadP);\n\n");

    // C++ compatibility end
    output.push_str("#ifdef __cplusplus\n");
    output.push_str("}\n");
    output.push_str("#endif\n\n");

    // Include guard end
    output.push_str(&format!("#endif /* {} */\n", guard_name));

    Ok(output)
}

/// Generate function parameters for header including count parameters for arrays (layout-based)
fn generate_function_params_for_header_with_layout(routine: &crate::semantic::AnalyzedRoutine) -> Vec<String> {
    use crate::parser::ast::Direction;
    let mut params = Vec::new();

    for arg in &routine.routine.args {
        // Check if this argument is an array by checking the message layout
        let is_array_in_request = routine.request_layout.fields.iter()
            .any(|f| f.name == arg.name && f.is_array);
        let is_array_in_reply = routine.reply_layout.as_ref()
            .map(|layout| layout.fields.iter().any(|f| f.name == arg.name && f.is_array))
            .unwrap_or(false);

        let is_array = is_array_in_request || is_array_in_reply;

        // Get the C type for the argument from the message layout (which has resolved types)
        let base_type = if is_array {
            // For arrays, get the element type from the layout
            let field_in_request = routine.request_layout.fields.iter()
                .find(|f| f.name == arg.name && f.is_array);
            let field_in_reply = routine.reply_layout.as_ref()
                .and_then(|layout| layout.fields.iter().find(|f| f.name == arg.name && f.is_array));

            let field = field_in_request.or(field_in_reply);
            if let Some(f) = field {
                // The c_type in the layout might be a pointer type already, strip it
                f.c_type.trim_end_matches('*').trim().to_string()
            } else {
                match &arg.arg_type {
                    crate::parser::ast::TypeSpec::Array { element, .. } => {
                        if let crate::parser::ast::TypeSpec::Basic(elem_name) = &**element {
                            elem_name.clone()
                        } else {
                            "void".to_string()
                        }
                    }
                    _ => "void".to_string(),
                }
            }
        } else {
            // For non-arrays, get from AST
            match &arg.arg_type {
                crate::parser::ast::TypeSpec::Basic(name) => name.clone(),
                _ => "void".to_string(),
            }
        };

        // Generate the parameter
        if is_array {
            // For arrays, always use pointers
            let param = match arg.direction {
                Direction::In | Direction::InOut => {
                    format!("    const {} *{}", base_type, arg.name)
                }
                Direction::Out => {
                    format!("    {} *{}", base_type, arg.name)
                }
                _ => format!("    {} *{}", base_type, arg.name),
            };
            params.push(param);

            // Add count parameter for IN/INOUT arrays
            if matches!(arg.direction, Direction::In | Direction::InOut) {
                params.push(format!("    mach_msg_type_number_t {}Cnt", arg.name));
            }
            // Add count parameter pointer for OUT arrays
            if matches!(arg.direction, Direction::Out) {
                params.push(format!("    mach_msg_type_number_t *{}Cnt", arg.name));
            }
        } else {
            // For non-arrays, use value or pointer based on direction
            let param = match arg.direction {
                Direction::Out | Direction::InOut => {
                    format!("    {} *{}", base_type, arg.name)
                }
                Direction::In | Direction::RequestPort => {
                    format!("    {} {}", base_type, arg.name)
                }
                _ => format!("    {} {}", base_type, arg.name),
            };
            params.push(param);
        }
    }

    params
}
