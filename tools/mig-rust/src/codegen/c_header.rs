//! C header file generation for MIG subsystems
//!
//! Generates .h files with function prototypes and type definitions.

use crate::semantic::AnalyzedSubsystem;
use crate::parser::ast::Direction;
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

    // Function prototypes
    output.push_str("/* User-side function prototypes */\n\n");

    for routine in &analyzed.routines {
        // Function comment
        output.push_str(&format!("/* Routine {} */\n", routine.name));

        // Return type
        output.push_str("extern kern_return_t ");
        output.push_str(&routine.user_function_name);
        output.push_str("(\n");

        // Parameters
        if routine.routine.args.is_empty() {
            output.push_str("    void");
        } else {
            for (i, arg) in routine.routine.args.iter().enumerate() {
                if i > 0 {
                    output.push_str(",\n");
                }

                // Generate parameter
                let c_type = get_c_type_for_arg(arg);
                match arg.direction {
                    Direction::Out | Direction::InOut => {
                        output.push_str(&format!("    {} *{}", c_type, arg.name));
                    }
                    Direction::In | Direction::RequestPort => {
                        output.push_str(&format!("    {} {}", c_type, arg.name));
                    }
                    _ => {
                        output.push_str(&format!("    {} {}", c_type, arg.name));
                    }
                }
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

    // Server implementation function prototypes
    output.push_str("/* Server implementation functions (provided by user) */\n\n");

    for routine in &analyzed.routines {
        // Function comment
        output.push_str(&format!("/* Routine {} implementation */\n", routine.name));

        // Return type
        output.push_str("extern kern_return_t ");
        output.push_str(&format!("{}_impl", routine.name));
        output.push_str("(\n");

        // Parameters
        if routine.routine.args.is_empty() {
            output.push_str("    void");
        } else {
            for (i, arg) in routine.routine.args.iter().enumerate() {
                if i > 0 {
                    output.push_str(",\n");
                }

                // Generate parameter
                let c_type = get_c_type_for_arg(arg);
                match arg.direction {
                    Direction::Out | Direction::InOut => {
                        output.push_str(&format!("    {} *{}", c_type, arg.name));
                    }
                    Direction::In | Direction::RequestPort => {
                        output.push_str(&format!("    {} {}", c_type, arg.name));
                    }
                    _ => {
                        output.push_str(&format!("    {} {}", c_type, arg.name));
                    }
                }
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

/// Get C type for an argument (helper function)
fn get_c_type_for_arg(arg: &crate::parser::ast::Argument) -> String {
    use crate::parser::ast::TypeSpec;

    // Extract the type name from the TypeSpec
    match &arg.arg_type {
        TypeSpec::Basic(name) => name.clone(),
        TypeSpec::Array { element, .. } => {
            // For arrays, return element type pointer
            match &**element {
                TypeSpec::Basic(name) => format!("{}*", name),
                _ => "void*".to_string(), // Fallback
            }
        }
        TypeSpec::Pointer(inner) => {
            match &**inner {
                TypeSpec::Basic(name) => format!("{}*", name),
                _ => "void*".to_string(),
            }
        }
        _ => "void*".to_string(), // Fallback for other types
    }
}
