//! Proptest strategies for generating valid .defs content
//!
//! This module provides strategies for property-based testing of the
//! mig-rust parser and code generator.

use proptest::prelude::*;

// ════════════════════════════════════════════════════════════
// Basic Strategies
// ════════════════════════════════════════════════════════════

/// Generate valid identifiers (C-style)
pub fn identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,30}".prop_map(|s| s.to_string())
}

/// Generate valid subsystem names
pub fn subsystem_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,20}".prop_map(|s| s.to_string())
}

/// Generate valid base IDs (1000-9999)
pub fn base_id() -> impl Strategy<Value = u32> {
    1000u32..10000u32
}

/// Generate valid built-in type names (no custom types)
pub fn builtin_type_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("int32_t".to_string()),
        Just("uint32_t".to_string()),
        Just("int64_t".to_string()),
        Just("uint64_t".to_string()),
        Just("mach_port_t".to_string()),
        Just("boolean_t".to_string()),
        Just("integer_t".to_string()),
        Just("natural_t".to_string()),
    ]
}

/// Generate valid type names (includes custom types for typedef declarations)
pub fn type_name() -> impl Strategy<Value = String> {
    prop_oneof![
        builtin_type_name(),
        identifier().prop_map(|s| format!("{}_t", s)),
    ]
}

/// Generate valid directions
pub fn direction() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("in"), Just("out"), Just("inout"),]
}

// ════════════════════════════════════════════════════════════
// Composite Strategies
// ════════════════════════════════════════════════════════════

/// Generate a valid argument (only uses built-in types)
pub fn argument() -> impl Strategy<Value = String> {
    (direction(), identifier(), builtin_type_name())
        .prop_map(|(dir, name, ty)| format!("{} {} : {}", dir, name, ty))
}

/// Generate a list of arguments (1-5)
pub fn argument_list() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(argument(), 1..6)
}

/// Generate a valid routine
pub fn routine() -> impl Strategy<Value = String> {
    (
        prop::bool::ANY, // is_simple
        identifier(),    // name
        argument_list(), // args
    )
        .prop_map(|(is_simple, name, args)| {
            let keyword = if is_simple {
                "simpleroutine"
            } else {
                "routine"
            };
            let args_str = args.join(";\n    ");
            format!("{} {}(\n    {}\n);", keyword, name, args_str)
        })
}

/// Generate a valid subsystem
pub fn subsystem() -> impl Strategy<Value = String> {
    (
        subsystem_name(),
        base_id(),
        prop::collection::vec(routine(), 1..5),
    )
        .prop_map(|(name, base, routines)| {
            let routines_str = routines.join("\n\n");
            format!("subsystem {} {};\n\n{}", name, base, routines_str)
        })
}

// ════════════════════════════════════════════════════════════
// Array Strategies
// ════════════════════════════════════════════════════════════

/// Generate array size specifications
pub fn array_size() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("*".to_string()),
        (1u32..1024u32).prop_map(|n| format!("*:{}", n)),
    ]
}

/// Generate array type definitions (only uses built-in element types)
pub fn array_type() -> impl Strategy<Value = String> {
    (array_size(), builtin_type_name())
        .prop_map(|(size, elem_type)| format!("array[{}] of {}", size, elem_type))
}

// ════════════════════════════════════════════════════════════
// Type Definition Strategies
// ════════════════════════════════════════════════════════════

/// Generate a type definition (aliases to built-in types)
pub fn type_definition() -> impl Strategy<Value = String> {
    (identifier(), prop_oneof![builtin_type_name(), array_type()])
        .prop_map(|(name, def)| format!("type {}_t = {};", name, def))
}

/// Generate a subsystem with type definitions
pub fn subsystem_with_types() -> impl Strategy<Value = String> {
    (
        subsystem_name(),
        base_id(),
        prop::collection::vec(type_definition(), 0..5),
        prop::collection::vec(routine(), 1..5),
    )
        .prop_map(|(name, base, types, routines)| {
            let mut output = format!("subsystem {} {};\n\n", name, base);

            if !types.is_empty() {
                output.push_str(&types.join("\n"));
                output.push_str("\n\n");
            }

            output.push_str(&routines.join("\n\n"));
            output
        })
}

// ════════════════════════════════════════════════════════════
// Number Strategies
// ════════════════════════════════════════════════════════════

/// Generate valid message IDs
pub fn message_id() -> impl Strategy<Value = u32> {
    1000u32..100000u32
}

/// Generate valid array bounds
pub fn array_bounds() -> impl Strategy<Value = (u32, u32)> {
    (1u32..100u32, 100u32..10000u32).prop_filter("max must be > min", |(min, max)| max > min)
}

// ════════════════════════════════════════════════════════════
// Complex Strategies
// ════════════════════════════════════════════════════════════

/// Generate a complete, valid .defs file
pub fn complete_defs_file() -> impl Strategy<Value = String> {
    (
        subsystem_name(),
        base_id(),
        prop::collection::vec(type_definition(), 0..10),
        prop::collection::vec(routine(), 1..10),
        prop::option::of(Just("ServerPrefix _X".to_string())),
        prop::option::of(identifier().prop_map(|s| format!("UserPrefix {}", s))),
    )
        .prop_map(
            |(name, base, types, routines, server_prefix, user_prefix)| {
                let mut output = String::new();

                // Header comment
                output.push_str("/* Generated by proptest */\n\n");

                // Subsystem declaration
                output.push_str(&format!("subsystem {} {};\n\n", name, base));

                // Prefixes (optional)
                let mut has_prefix = false;
                if let Some(ref sp) = server_prefix {
                    output.push_str(sp);
                    output.push_str(";\n");
                    has_prefix = true;
                }
                if let Some(ref up) = user_prefix {
                    // Only output if not empty
                    if !up.is_empty() {
                        output.push_str(up);
                        output.push_str(";\n");
                        has_prefix = true;
                    }
                }
                if has_prefix {
                    output.push_str("\n");
                }

                // Type definitions
                if !types.is_empty() {
                    output.push_str(&types.join("\n"));
                    output.push_str("\n\n");
                }

                // Routines
                output.push_str(&routines.join("\n\n"));
                output.push('\n');

                output
            },
        )
}

// ════════════════════════════════════════════════════════════
// Edge Case Strategies
// ════════════════════════════════════════════════════════════

/// Generate edge cases for testing robustness
pub fn edge_case_subsystem() -> impl Strategy<Value = String> {
    prop_oneof![
        // Minimal subsystem
        Just("subsystem minimal 1000;\nroutine test(in x : int32_t);".to_string()),
        // Maximum complexity
        complete_defs_file(),
        // Only simpleroutines
        (subsystem_name(), base_id()).prop_map(|(name, base)| {
            format!(
                "subsystem {} {};\n\
                     simpleroutine test1(in x : int32_t);\n\
                     simpleroutine test2(in y : uint32_t);",
                name, base
            )
        }),
        // Large arrays
        (subsystem_name(), base_id()).prop_map(|(name, base)| {
            format!(
                "subsystem {} {};\n\
                     type large_array = array[*:65536] of uint8_t;\n\
                     routine test(in data : large_array);",
                name, base
            )
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_identifier_generates_valid(s in identifier()) {
            assert!(s.len() > 0);
            assert!(s.len() <= 31);
            assert!(s.chars().next().unwrap().is_ascii_lowercase());
        }

        #[test]
        fn test_base_id_in_range(id in base_id()) {
            assert!(id >= 1000);
            assert!(id < 10000);
        }

        #[test]
        fn test_subsystem_generates_parseable(content in subsystem()) {
            // Should contain required keywords
            assert!(content.contains("subsystem"));
            assert!(content.contains("routine") || content.contains("simpleroutine"));
        }

        #[test]
        fn test_array_bounds_valid((min, max) in array_bounds()) {
            assert!(max > min);
            assert!(min >= 1);
        }
    }
}
