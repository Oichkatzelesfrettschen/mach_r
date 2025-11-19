//! Property-based tests for mig-rust
//!
//! These tests use proptest to generate random valid .defs content
//! and verify that the lexer, parser, and semantic analyzer handle
//! all valid inputs correctly.

mod proptest_strategies;

use mig_rust::*;
use mig_rust::codegen::rust_stubs::RustStubGenerator;
use proptest::prelude::*;
use proptest_strategies::*;

// ════════════════════════════════════════════════════════════
// Lexer Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Lexer should never panic on valid identifiers
    #[test]
    fn lexer_handles_identifiers(ident in identifier()) {
        let mut lexer = SimpleLexer::new(ident);
        let result = lexer.tokenize();
        assert!(result.is_ok(), "Lexer failed on valid identifier");
    }

    /// Lexer should tokenize valid subsystem declarations
    #[test]
    fn lexer_handles_subsystem_declaration(
        name in subsystem_name(),
        base in base_id()
    ) {
        let input = format!("subsystem {} {};", name, base);
        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer.tokenize().expect("Lexer failed");
        assert!(tokens.len() >= 4); // subsystem, name, base, semicolon
    }

    /// Lexer should handle comments correctly with valid content
    #[test]
    fn lexer_strips_comments(content in "[a-zA-Z0-9_ ]{0,100}") {
        let input = format!("/* comment */ {}", content);
        let mut lexer = SimpleLexer::new(input);
        let result = lexer.tokenize();
        // Should not fail on comments with valid ASCII content
        assert!(result.is_ok());
    }
}

// ════════════════════════════════════════════════════════════
// Parser Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Parser should handle all valid subsystems
    #[test]
    fn parser_handles_valid_subsystems(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content.clone());
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let result = parser.parse();

        assert!(
            result.is_ok(),
            "Parser failed on valid subsystem:\n{}",
            content
        );
    }

    /// Parser correctly extracts subsystem name and base
    #[test]
    fn parser_extracts_metadata(
        name in subsystem_name(),
        base in base_id()
    ) {
        let input = format!("subsystem {} {};\nroutine test(in x : int32_t);", name, base);
        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        assert_eq!(subsystem.name, name);
        assert_eq!(subsystem.base, base);
    }

    /// Parser handles type definitions
    #[test]
    fn parser_handles_type_definitions(typedef in type_definition()) {
        let input = format!(
            "subsystem test 1000;\n{}\nroutine r(in x : int32_t);",
            typedef
        );
        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let result = parser.parse();

        assert!(result.is_ok(), "Parser failed on typedef: {}", typedef);
    }
}

// ════════════════════════════════════════════════════════════
// Semantic Analyzer Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Semantic analyzer should handle valid subsystems
    #[test]
    fn analyzer_handles_valid_subsystems(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content.clone());
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let result = analyzer.analyze(&subsystem);

        assert!(
            result.is_ok(),
            "Analyzer failed on valid subsystem:\n{}",
            content
        );
    }

    /// Routine numbers should be base + index
    #[test]
    fn analyzer_assigns_correct_routine_numbers(
        name in subsystem_name(),
        base in base_id()
    ) {
        let input = format!(
            "subsystem {} {};\n\
             routine r1(in x : int32_t);\n\
             routine r2(in y : int32_t);\n\
             routine r3(in z : int32_t);",
            name, base
        );

        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        assert_eq!(analyzed.routines.len(), 3);
        assert_eq!(analyzed.routines[0].number, base);
        assert_eq!(analyzed.routines[1].number, base + 1);
        assert_eq!(analyzed.routines[2].number, base + 2);
    }

    /// Array bounds should be validated
    #[test]
    fn analyzer_validates_array_bounds(max in 100u32..10000u32) {
        let input = format!(
            "subsystem test 1000;\n\
             type arr_t = array[*:{}] of int32_t;\n\
             routine test(in data : arr_t);",
            max
        );

        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        // Find the array field
        let array_field = analyzed.routines[0]
            .request_layout
            .fields
            .iter()
            .find(|f| f.is_array);

        if let Some(field) = array_field {
            assert_eq!(field.max_array_elements, Some(max));
        }
    }
}

// ════════════════════════════════════════════════════════════
// Code Generation Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Code generator should handle all valid subsystems
    #[test]
    fn codegen_handles_valid_subsystems(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content.clone());
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let result = generator.generate(&analyzed);

        assert!(
            result.is_ok(),
            "Codegen failed on valid subsystem:\n{}",
            content
        );
    }

    /// Generated code should contain all routine names
    #[test]
    fn codegen_includes_all_routines(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let rust_code = generator.generate(&analyzed).expect("Codegen failed");

        // Every routine should have a client stub
        for routine in &analyzed.routines {
            assert!(
                rust_code.contains(&format!("pub fn {}(", routine.name)),
                "Missing client stub for routine {}",
                routine.name
            );
        }
    }

    /// Generated code should have valid structure
    #[test]
    fn codegen_produces_valid_structure(content in subsystem_with_types()) {
        let mut lexer = SimpleLexer::new(content);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let rust_code = generator.generate(&analyzed).expect("Codegen failed");

        // Basic structure checks
        assert!(rust_code.contains("pub mod"));
        assert!(rust_code.contains("pub const BASE_ID"));
        assert!(rust_code.contains("use mach_r::ipc"));
        assert!(rust_code.contains("#[repr(C, align(8))]"));
    }
}

// ════════════════════════════════════════════════════════════
// Edge Case Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// Should handle edge cases without panicking
    #[test]
    fn handles_edge_cases(content in edge_case_subsystem()) {
        let mut lexer = SimpleLexer::new(content.clone());
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let result = generator.generate(&analyzed);

        assert!(
            result.is_ok(),
            "Failed on edge case:\n{}",
            content
        );
    }

    /// Message IDs should be unique
    #[test]
    fn message_ids_are_unique(content in subsystem_with_types()) {
        let mut lexer = SimpleLexer::new(content);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let mut seen_ids = std::collections::HashSet::new();
        for routine in &analyzed.routines {
            assert!(
                seen_ids.insert(routine.number),
                "Duplicate message ID: {}",
                routine.number
            );
        }
    }
}

// ════════════════════════════════════════════════════════════
// Regression Property Tests
// ════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Port types should always use port_copy_send descriptor
    #[test]
    fn port_types_use_correct_descriptor(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let rust_code = generator.generate(&analyzed).expect("Codegen failed");

        // If there's a port field, it should use port_copy_send
        for routine in &analyzed.routines {
            for field in &routine.request_layout.fields {
                if field.c_type.contains("mach_port") {
                    let type_field = format!("{}Type", field.name);
                    if rust_code.contains(&type_field) {
                        assert!(
                            rust_code.contains(&format!(
                                "{}: MachMsgType::port_copy_send()",
                                type_field
                            )),
                            "Port field {} should use port_copy_send()",
                            field.name
                        );
                    }
                }
            }
        }
    }

    /// No unimplemented!() in sync stubs
    #[test]
    fn no_unimplemented_in_sync_stubs(content in subsystem()) {
        let mut lexer = SimpleLexer::new(content);
        let tokens = lexer.tokenize().expect("Lexer failed");

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect("Parser failed");

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem).expect("Analyzer failed");

        let generator = RustStubGenerator::new();
        let rust_code = generator.generate(&analyzed).expect("Codegen failed");

        // Check each sync stub
        for routine in &analyzed.routines {
            let stub_start = format!("pub fn {}(", routine.name);
            let async_start = format!("pub async fn {}_async(", routine.name);

            if let Some(start_pos) = rust_code.find(&stub_start) {
                // Find the end of this function (before async variant)
                let end_pos = if let Some(async_pos) = rust_code.find(&async_start) {
                    async_pos
                } else {
                    rust_code.len()
                };

                let stub_code = &rust_code[start_pos..end_pos];

                assert!(
                    !stub_code.contains("unimplemented!"),
                    "Sync stub for {} contains unimplemented!()",
                    routine.name
                );
            }
        }
    }
}
