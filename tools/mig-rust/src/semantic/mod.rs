//! Semantic analysis for MIG interface definitions
//!
//! This module provides semantic analysis capabilities including:
//! - Type resolution and validation
//! - Message layout calculation
//! - Routine analysis and numbering
//! - Semantic validation and error reporting

pub mod analyzer;
pub mod layout;
pub mod types;

pub use analyzer::{AnalyzedRoutine, AnalyzedSubsystem, SemanticAnalyzer};
pub use layout::{MessageLayout, MessageLayoutCalculator};
pub use types::{MachMsgType, ResolvedType, TypeResolver};

/// Semantic analysis errors
#[derive(Debug, Clone)]
pub enum SemanticError {
    /// Undefined type reference
    UndefinedType { name: String, location: String },
    /// Type mismatch
    TypeMismatch {
        expected: String,
        found: String,
        location: String,
    },
    /// Invalid array bounds
    InvalidArrayBounds { routine: String, argument: String },
    /// Message too large
    MessageTooLarge {
        routine: String,
        size: usize,
        max: usize,
    },
    /// Duplicate routine number
    DuplicateRoutineNumber {
        routine1: String,
        routine2: String,
        number: u32,
    },
    /// Invalid port disposition
    InvalidPortDisposition {
        routine: String,
        argument: String,
        reason: String,
    },
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticError::UndefinedType { name, location } => {
                write!(f, "Undefined type '{}' at {}", name, location)
            }
            SemanticError::TypeMismatch {
                expected,
                found,
                location,
            } => {
                write!(
                    f,
                    "Type mismatch at {}: expected '{}', found '{}'",
                    location, expected, found
                )
            }
            SemanticError::InvalidArrayBounds { routine, argument } => {
                write!(
                    f,
                    "Invalid array bounds in routine '{}', argument '{}'",
                    routine, argument
                )
            }
            SemanticError::MessageTooLarge { routine, size, max } => {
                write!(
                    f,
                    "Message too large in routine '{}': {} bytes (max {})",
                    routine, size, max
                )
            }
            SemanticError::DuplicateRoutineNumber {
                routine1,
                routine2,
                number,
            } => {
                write!(
                    f,
                    "Duplicate routine number {}: '{}' and '{}'",
                    number, routine1, routine2
                )
            }
            SemanticError::InvalidPortDisposition {
                routine,
                argument,
                reason,
            } => {
                write!(
                    f,
                    "Invalid port disposition in routine '{}', argument '{}': {}",
                    routine, argument, reason
                )
            }
        }
    }
}

impl std::error::Error for SemanticError {}
