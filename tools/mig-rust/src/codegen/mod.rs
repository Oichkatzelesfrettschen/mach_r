/// Code generation for MIG - C and Rust output
pub mod c_generator;
pub mod c_header;
pub mod c_server_stubs;
pub mod c_user_stubs;
pub mod rust_generator;
pub mod rust_stubs; // Type-safe Rust IPC stubs

use crate::error::CodegenError;
use crate::parser::ast::Subsystem;

/// Code generator trait
pub trait CodeGenerator {
    fn generate_user_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_user_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_server_header(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
    fn generate_server_impl(&self, subsystem: &Subsystem) -> Result<String, CodegenError>;
}
