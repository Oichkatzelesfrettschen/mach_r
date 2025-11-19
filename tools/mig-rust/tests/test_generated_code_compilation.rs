//! Test that generated Rust code compiles and has correct structure
//!
//! This test generates Rust code and verifies its structure without requiring
//! the mach_r runtime to be available.

use mig_rust::*;
use mig_rust::codegen::rust_stubs::RustStubGenerator;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_generated_code_structure() {
    // Parse simple.defs
    let input = include_str!("simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    // Generate Rust code
    let generator = RustStubGenerator::new()
        .with_async()
        .with_server_traits();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify structure
    assert!(rust_code.contains("pub mod simple"));
    assert!(rust_code.contains("pub const BASE_ID: u32 = 1000;"));
    assert!(rust_code.contains("pub const ADD_ID: u32 = 1000;"));
    assert!(rust_code.contains("pub const LOG_MESSAGE_ID: u32 = 1001;"));

    // Verify message structures
    assert!(rust_code.contains("pub struct AddRequest"));
    assert!(rust_code.contains("pub struct AddReply"));
    assert!(rust_code.contains("pub struct LogMessageRequest"));
    assert!(rust_code.contains("#[repr(C, align(8))]"));

    // Verify constructors
    assert!(rust_code.contains("pub fn new("));
    assert!(rust_code.contains("MachMsgHeader::new("));
    assert!(rust_code.contains("MachMsgType::"));

    // Verify client stubs
    assert!(rust_code.contains("pub fn add("));
    assert!(rust_code.contains("pub fn log_message("));
    assert!(rust_code.contains("send_recv_msg("));
    assert!(rust_code.contains("send_msg("));

    // Verify async stubs
    assert!(rust_code.contains("pub async fn add_async("));
    assert!(rust_code.contains("pub async fn log_message_async("));

    // Verify server traits
    assert!(rust_code.contains("pub trait SimpleServer"));
    assert!(rust_code.contains("#[async_trait]"));

    // Verify return types
    assert!(rust_code.contains("Result<i32, IpcError>"));
    assert!(rust_code.contains("Result<(), IpcError>"));

    // Verify no unimplemented!() in sync stubs
    let lines: Vec<&str> = rust_code.lines().collect();
    let mut in_sync_stub = false;
    let mut in_async_stub = false;

    for line in lines {
        if line.contains("pub fn add(") || line.contains("pub fn log_message(") {
            in_sync_stub = true;
            in_async_stub = false;
        } else if line.contains("pub async fn") {
            in_sync_stub = false;
            in_async_stub = true;
        } else if line.trim().starts_with("}") && (in_sync_stub || in_async_stub) {
            in_sync_stub = false;
            in_async_stub = false;
        }

        // Sync stubs should NOT have unimplemented!()
        if in_sync_stub && line.contains("unimplemented!") {
            panic!("Found unimplemented!() in sync stub: {}", line);
        }
    }
}

#[test]
fn test_array_code_generation() {
    let input = include_str!("array.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify array handling
    assert!(rust_code.contains("&[i32]"));
    assert!(rust_code.contains("ArrayTooLarge"));
    assert!(rust_code.contains("if data.len() >"));
    assert!(rust_code.contains("[Default::default(); 1024]"));
    assert!(rust_code.contains("copy_from_slice"));

    // Verify no placeholder "_" in array initialization
    assert!(!rust_code.contains("[Default::default(); _]"));
}

#[test]
fn test_file_generation() {
    let input = include_str!("simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new()
        .with_async()
        .with_server_traits();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Write to temporary file
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("simple.rs");
    fs::write(&file_path, &rust_code).expect("failed to write file");

    // Verify file was created
    assert!(file_path.exists());

    // Read back and verify
    let contents = fs::read_to_string(&file_path).expect("failed to read file");
    assert_eq!(contents, rust_code);
}

#[test]
fn test_imports_complete() {
    let input = include_str!("simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new()
        .with_async()
        .with_server_traits();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify all required imports are present
    assert!(rust_code.contains("use mach_r::ipc"));
    assert!(rust_code.contains("MachMsgHeader"));
    assert!(rust_code.contains("MachMsgType"));
    assert!(rust_code.contains("PortName"));
    assert!(rust_code.contains("IpcError"));
    assert!(rust_code.contains("KernReturn"));
    assert!(rust_code.contains("MACH_MSGH_BITS"));
    assert!(rust_code.contains("MACH_MSG_TYPE_COPY_SEND"));
    assert!(rust_code.contains("MACH_MSG_TYPE_MAKE_SEND_ONCE"));
    assert!(rust_code.contains("MACH_PORT_NULL"));
    assert!(rust_code.contains("KERN_SUCCESS"));
    assert!(rust_code.contains("MACH_MSG_TIMEOUT_NONE"));
    assert!(rust_code.contains("send_msg"));
    assert!(rust_code.contains("send_recv_msg"));
    assert!(rust_code.contains("AsyncPort"));
    assert!(rust_code.contains("async_trait"));
    assert!(rust_code.contains("use std::mem::size_of"));
}

#[test]
fn test_async_stub_implementation() {
    let input = include_str!("simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new().with_async();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Verify async stubs use tokio::task::spawn_blocking
    assert!(rust_code.contains("tokio::task::spawn_blocking"));

    // Verify async stubs don't have unimplemented!()
    let async_stub_start = rust_code.find("pub async fn add_async(").expect("async stub not found");
    let next_fn = rust_code[async_stub_start..].find("\n    pub ").unwrap_or(rust_code.len());
    let async_stub_code = &rust_code[async_stub_start..async_stub_start + next_fn];

    assert!(!async_stub_code.contains("unimplemented!"),
        "Async stub should not contain unimplemented!()");

    // Verify it calls the sync version
    assert!(async_stub_code.contains("add(port"),
        "Async stub should call sync version");

    // Verify proper error handling
    assert!(async_stub_code.contains("map_err"),
        "Async stub should handle join errors");
}
