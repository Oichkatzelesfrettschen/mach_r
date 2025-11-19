//! Integration tests for Rust code generation

use mig_rust::*;
use mig_rust::codegen::rust_stubs::RustStubGenerator;

#[test]
fn test_simple_rust_generation() {
    // Parse simple.defs
    let input = include_str!("../../tests/simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse_subsystem().expect("parse failed");

    // Semantic analysis
    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    // Generate Rust code
    let generator = RustStubGenerator::new();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Basic validation
    assert!(rust_code.contains("pub mod"));
    assert!(rust_code.contains("pub const BASE_ID"));
    assert!(rust_code.contains("Request"));

    // Print for manual inspection
    println!("Generated Rust code:\n{}", rust_code);
}

#[test]
fn test_array_rust_generation() {
    let input = include_str!("../../tests/array.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse_subsystem().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    let generator = RustStubGenerator::new();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Should contain array types
    assert!(rust_code.contains("[]"));  // Array syntax
    assert!(rust_code.contains("ArrayTooLarge"));  // Error handling

    println!("Generated Rust code with arrays:\n{}", rust_code);
}

#[test]
fn test_async_rust_generation() {
    let input = include_str!("../../tests/simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse_subsystem().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    // Generate with async API
    let generator = RustStubGenerator::new().with_async();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Should contain async keywords
    assert!(rust_code.contains("async"));
    assert!(rust_code.contains("AsyncPort"));

    println!("Generated async Rust code:\n{}", rust_code);
}

#[test]
fn test_server_trait_generation() {
    let input = include_str!("../../tests/simple.defs");

    let mut lexer = SimpleLexer::new(input.to_string());
    let tokens = lexer.tokenize().expect("tokenize failed");

    let mut parser = Parser::new(tokens);
    let subsystem = parser.parse_subsystem().expect("parse failed");

    let mut analyzer = SemanticAnalyzer::new();
    let analyzed = analyzer.analyze(&subsystem).expect("analysis failed");

    // Generate with server traits
    let generator = RustStubGenerator::new().with_server_traits();
    let rust_code = generator.generate(&analyzed).expect("codegen failed");

    // Should contain trait definitions
    assert!(rust_code.contains("pub trait"));
    assert!(rust_code.contains("Server"));

    println!("Generated Rust code with server traits:\n{}", rust_code);
}
