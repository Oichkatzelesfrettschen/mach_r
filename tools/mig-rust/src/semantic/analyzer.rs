//! Semantic analyzer for MIG subsystems

use crate::parser::ast::{Subsystem, Statement, Routine};
use super::types::TypeResolver;
use super::layout::{MessageLayoutCalculator, MessageLayout};
use super::SemanticError;

/// Analyzed subsystem with semantic information
#[derive(Debug, Clone)]
pub struct AnalyzedSubsystem {
    /// Subsystem name
    pub name: String,
    /// Base routine number
    pub base: u32,
    /// Analyzed routines
    pub routines: Vec<AnalyzedRoutine>,
    /// Server prefix
    pub server_prefix: String,
    /// User prefix
    pub user_prefix: String,
}

/// Analyzed routine with semantic information
#[derive(Debug, Clone)]
pub struct AnalyzedRoutine {
    /// Routine name
    pub name: String,
    /// Routine number (base + index)
    pub number: u32,
    /// Is this a simpleroutine?
    pub is_simple: bool,
    /// Original routine AST
    pub routine: Routine,
    /// Request message layout
    pub request_layout: MessageLayout,
    /// Reply message layout (None for simpleroutine)
    pub reply_layout: Option<MessageLayout>,
    /// User-side function name
    pub user_function_name: String,
    /// Server-side handler name
    pub server_function_name: String,
}

/// Semantic analyzer
pub struct SemanticAnalyzer {
    type_resolver: TypeResolver,
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer
    pub fn new() -> Self {
        Self {
            type_resolver: TypeResolver::new(),
        }
    }

    /// Analyze a subsystem
    pub fn analyze(&mut self, subsystem: &Subsystem) -> Result<AnalyzedSubsystem, SemanticError> {
        // Resolve types
        self.type_resolver.resolve_subsystem(subsystem)?;

        // Extract prefixes from statements
        let mut server_prefix = "_X".to_string();
        let mut user_prefix = String::new();

        for statement in &subsystem.statements {
            match statement {
                Statement::ServerPrefix(prefix) => {
                    server_prefix = prefix.clone();
                }
                Statement::UserPrefix(prefix) => {
                    user_prefix = prefix.clone();
                }
                _ => {}
            }
        }

        // Analyze routines
        let mut routines = Vec::new();
        let mut routine_index = 0;

        for statement in &subsystem.statements {
            match statement {
                Statement::Routine(routine) => {
                    let analyzed = self.analyze_routine(
                        routine,
                        subsystem.base + routine_index,
                        false,
                        &user_prefix,
                        &server_prefix,
                    )?;
                    routines.push(analyzed);
                    routine_index += 1;
                }
                Statement::SimpleRoutine(routine) => {
                    let analyzed = self.analyze_routine(
                        routine,
                        subsystem.base + routine_index,
                        true,
                        &user_prefix,
                        &server_prefix,
                    )?;
                    routines.push(analyzed);
                    routine_index += 1;
                }
                Statement::Skip => {
                    // Skip increments the routine number without creating a routine
                    routine_index += 1;
                }
                _ => {}
            }
        }

        Ok(AnalyzedSubsystem {
            name: subsystem.name.clone(),
            base: subsystem.base,
            routines,
            server_prefix,
            user_prefix,
        })
    }

    /// Analyze a single routine
    fn analyze_routine(
        &self,
        routine: &Routine,
        number: u32,
        is_simple: bool,
        user_prefix: &str,
        server_prefix: &str,
    ) -> Result<AnalyzedRoutine, SemanticError> {
        // Calculate message layouts
        let layout_calc = MessageLayoutCalculator::new(&self.type_resolver);
        let request_layout = layout_calc.calculate_request_layout(routine);
        let reply_layout = if !is_simple {
            Some(layout_calc.calculate_reply_layout(routine))
        } else {
            None
        };

        // Generate function names
        let user_function_name = format!("{}{}", user_prefix, routine.name);
        let server_function_name = format!("{}{}", server_prefix, routine.name);

        Ok(AnalyzedRoutine {
            name: routine.name.clone(),
            number,
            is_simple,
            routine: routine.clone(),
            request_layout,
            reply_layout,
            user_function_name,
            server_function_name,
        })
    }

    /// Get the type resolver
    pub fn type_resolver(&self) -> &TypeResolver {
        &self.type_resolver
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
