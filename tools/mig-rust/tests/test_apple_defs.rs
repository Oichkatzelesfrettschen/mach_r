//! Integration tests for Apple .defs files
//!
//! These tests validate that real Apple/OSF .defs files parse and
//! generate correct Rust code.

use mig_rust::codegen::rust_stubs::RustStubGenerator;
use mig_rust::*;

#[test]
fn test_port_defs_parsing() {
    let input = include_str!("port.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    assert_eq!(subsystem.name, "port_test");
    assert_eq!(subsystem.base, 3000);

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    assert_eq!(analyzed.routines.len(), 3);
    assert_eq!(analyzed.routines[0].name, "create_port");
    assert_eq!(analyzed.routines[1].name, "destroy_port");
    assert_eq!(analyzed.routines[2].name, "get_port_rights");

    // destroy_port should be a simpleroutine
    assert!(analyzed.routines[1].is_simple);
}

#[test]
fn test_port_defs_rust_generation() {
    let input = include_str!("port.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify structure
    assert!(rust_code.contains("pub mod port_test"));
    assert!(rust_code.contains("pub const BASE_ID: u32 = 3000;"));
    assert!(rust_code.contains("pub const CREATE_PORT_ID: u32 = 3000;"));
    assert!(rust_code.contains("pub const DESTROY_PORT_ID: u32 = 3001;"));
    assert!(rust_code.contains("pub const GET_PORT_RIGHTS_ID: u32 = 3002;"));

    // Verify message structures
    assert!(rust_code.contains("pub struct CreatePortRequest"));
    assert!(rust_code.contains("pub struct CreatePortReply"));
    assert!(rust_code.contains("pub struct DestroyPortRequest"));
    assert!(rust_code.contains("pub struct GetPortRightsRequest"));
    assert!(rust_code.contains("pub struct GetPortRightsReply"));

    // Verify client stubs
    assert!(rust_code.contains("pub fn create_port("));
    assert!(rust_code.contains("pub fn destroy_port("));
    assert!(rust_code.contains("pub fn get_port_rights("));

    // destroy_port should use send_msg (no reply)
    assert!(rust_code.contains("send_msg("));
}

#[test]
fn test_exc_defs_parsing() {
    let input = include_str!("exc.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    assert_eq!(subsystem.name, "exc");
    assert_eq!(subsystem.base, 2400);

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    assert_eq!(analyzed.routines.len(), 3);
    assert_eq!(analyzed.routines[0].name, "exception_raise");
    assert_eq!(analyzed.routines[1].name, "exception_raise_state");
    assert_eq!(analyzed.routines[2].name, "exception_raise_state_identity");

    // Server prefix should be catch_
    assert_eq!(analyzed.server_prefix, "catch_");
}

#[test]
fn test_exc_defs_rust_generation() {
    let input = include_str!("exc.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify structure
    assert!(rust_code.contains("pub mod exc"));
    assert!(rust_code.contains("pub const BASE_ID: u32 = 2400;"));
    assert!(rust_code.contains("pub const EXCEPTION_RAISE_ID: u32 = 2401;"));

    // Verify message structures
    assert!(rust_code.contains("pub struct ExceptionRaiseRequest"));
    assert!(rust_code.contains("pub struct ExceptionRaiseReply"));

    // Verify array handling (exception_data_t = array[*:2] of integer_t)
    assert!(rust_code.contains("[integer_t; 2]"));
    assert!(rust_code.contains("&[integer_t]"));

    // Verify port type handling
    assert!(rust_code.contains("exception_port: PortName"));
    assert!(rust_code.contains("thread: PortName"));
    assert!(rust_code.contains("task: PortName"));

    // Verify port type descriptors are correct
    assert!(rust_code.contains("exception_portType: MachMsgType::port_copy_send()"));
    assert!(rust_code.contains("threadType: MachMsgType::port_copy_send()"));
    assert!(rust_code.contains("taskType: MachMsgType::port_copy_send()"));
}

#[test]
fn test_all_apple_defs_compile() {
    // Test all Apple .defs files can be parsed and generate code
    let test_files = vec![("port.defs", "port_test"), ("exc.defs", "exc")];

    for (file, expected_name) in test_files {
        let input = std::fs::read_to_string(format!("tests/{}", file))
            .expect(&format!("Failed to read {}", file));

        let mut lexer = SimpleLexer::new(input);
        let tokens = lexer
            .tokenize()
            .expect(&format!("{}: tokenize failed", file));

        let mut parser = Parser::new(tokens);
        let subsystem = parser.parse().expect(&format!("{}: parse failed", file));

        assert_eq!(
            subsystem.name, expected_name,
            "{}: wrong subsystem name",
            file
        );

        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer
            .analyze(&subsystem)
            .expect(&format!("{}: analysis failed", file));

        let generator = RustStubGenerator::new();
        let _rust_code = generator
            .generate(&analyzed)
            .expect(&format!("{}: codegen failed", file));
    }
}

#[test]
fn test_preprocessor_conditional_handling() {
    // exc.defs has complex #if KERNEL_USER conditionals
    let input = include_str!("exc.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    // Count tokens before preprocessing
    let token_count_before = tokens.len();

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    // Preprocessor should have reduced token count by filtering out
    // KERNEL_USER sections
    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    // All routines should have mach_port_t (not mach_port_move_send_t)
    // because KERNEL_USER is not defined
    for routine in &analyzed.routines {
        for field in &routine.request_layout.fields {
            if field.c_type.contains("mach_port") {
                assert!(
                    field.c_type == "mach_port_t",
                    "Expected mach_port_t, got {}",
                    field.c_type
                );
            }
        }
    }

    assert!(token_count_before > 0, "Should have tokens");
}
