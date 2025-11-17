/// Code generation for MIG - C and Rust output

pub mod c_generator;
pub mod rust_generator;

use crate::parser::ast::Subsystem;

/// Code generator trait
pub trait CodeGenerator {
    fn generate_user_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_user_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_server_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_server_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
}

#[derive(Debug, Clone)]
pub struct CodegenError {
    pub message: String,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}
