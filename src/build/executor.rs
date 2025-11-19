//! Build execution engine
//! Handles dependency resolution, parallel execution, and incremental builds

use heapless::{Vec, String};
use super::BuildSystem;

/// Build execution result
#[derive(Debug, Clone, Copy)]
pub struct BuildResult {
    pub success: bool,
    pub targets_built: usize,
    pub targets_up_to_date: usize,
    pub build_time_ms: u64,
}

/// Build a specific target and its dependencies
pub fn build_target(build_system: &mut BuildSystem, output: &str) -> Result<bool, &'static str> {
    // Check if target exists
    let target_index = build_system.targets.iter()
        .position(|t| t.output.as_str() == output)
        .ok_or("Target not found")?;
    
    // Build dependencies first
    let dependencies = get_target_dependencies(build_system, target_index)?;
    
    for dep_output in &dependencies {
        let dep_success = build_target(build_system, dep_output)?;
        if !dep_success {
            return Ok(false);
        }
    }
    
    // Check if target needs rebuilding
    if !build_system.targets[target_index].needs_rebuild() {
        if build_system.verbose {
            // TODO: Print "Target up to date"
        }
        return Ok(true);
    }
    
    // Execute the build command for this target
    execute_target_command(build_system, target_index)
}

/// Get all dependencies for a target
fn get_target_dependencies(build_system: &BuildSystem, target_index: usize) -> Result<Vec<String<512>, 32>, &'static str> {
    let mut deps = Vec::new();
    let target = &build_system.targets[target_index];
    
    for input in &target.inputs {
        // Check if input is another target's output
        if build_system.targets.iter().any(|t| t.output.as_str() == input.as_str()) {
            let mut dep = String::new();
            dep.push_str(input).map_err(|_| "Dependency path too long")?;
            deps.push(dep).map_err(|_| "Too many dependencies")?;
        }
    }
    
    Ok(deps)
}

/// Execute the build command for a target
fn execute_target_command(build_system: &mut BuildSystem, target_index: usize) -> Result<bool, &'static str> {
    let target = &build_system.targets[target_index];
    let rule = build_system.find_rule(&target.rule)
        .ok_or("Rule not found for target")?;
    
    // Expand command template
    let expanded_command = rule.expand_command(target, &build_system.variables)?;
    
    if build_system.verbose {
        // TODO: Print expanded command
    } else if !rule.description.is_empty() {
        // TODO: Print rule description with variables expanded
    }
    
    // Execute the command
    execute_shell_command(&expanded_command)
}

/// Execute a shell command
fn execute_shell_command(command: &str) -> Result<bool, &'static str> {
    // TODO: Implement actual command execution
    // This would involve:
    // 1. Parsing the command line
    // 2. Setting up process environment
    // 3. Executing the command
    // 4. Capturing output and exit code
    // 5. Handling errors appropriately
    
    // For now, simulate command execution
    if command.is_empty() {
        return Ok(false);
    }
    
    // Simulate some command patterns
    if command.contains("rustc") || command.contains("cargo") {
        // Simulate Rust compilation
        Ok(true)
    } else if command.contains("gcc") || command.contains("clang") {
        // Simulate C/C++ compilation
        Ok(true)
    } else if command.contains("cp") || command.contains("mkdir") {
        // Simulate file operations
        Ok(true)
    } else {
        // Unknown command - assume success for now
        Ok(true)
    }
}

/// Build multiple targets in parallel
pub fn build_targets_parallel(build_system: &mut BuildSystem, outputs: &[&str]) -> Result<BuildResult, &'static str> {
    let start_time = get_timestamp_ms();
    let mut targets_built = 0;
    let mut targets_up_to_date = 0;
    let mut all_success = true;
    
    // TODO: Implement actual parallel execution
    // For now, execute sequentially
    
    for output in outputs {
        if let Some(target_index) = build_system.targets.iter().position(|t| t.output.as_str() == *output) {
            if build_system.targets[target_index].needs_rebuild() {
                let success = build_target(build_system, output)?;
                if success {
                    targets_built += 1;
                } else {
                    all_success = false;
                    break;
                }
            } else {
                targets_up_to_date += 1;
            }
        }
    }
    
    let end_time = get_timestamp_ms();
    
    Ok(BuildResult {
        success: all_success,
        targets_built,
        targets_up_to_date,
        build_time_ms: end_time - start_time,
    })
}

/// Get build order for all targets (topological sort)
pub fn get_build_order(build_system: &BuildSystem) -> Result<Vec<usize, 1024>, &'static str> {
    let mut build_order = Vec::new();
    let mut visited = Vec::<bool, 1024>::new();
    let mut temp_mark = Vec::<bool, 1024>::new();
    
    // Initialize visited arrays
    for _ in 0..build_system.targets.len() {
        visited.push(false).map_err(|_| "Too many targets for topological sort")?;
        temp_mark.push(false).map_err(|_| "Too many targets for topological sort")?;
    }
    
    // Visit all nodes
    for i in 0..build_system.targets.len() {
        if !visited[i] {
            topological_sort_visit(build_system, i, &mut visited, &mut temp_mark, &mut build_order)?;
        }
    }
    
    // Reverse to get correct order
    build_order.reverse();
    
    Ok(build_order)
}

/// Recursive helper for topological sort
fn topological_sort_visit(
    build_system: &BuildSystem,
    target_index: usize,
    visited: &mut Vec<bool, 1024>,
    temp_mark: &mut Vec<bool, 1024>,
    build_order: &mut Vec<usize, 1024>,
) -> Result<(), &'static str> {
    if temp_mark[target_index] {
        return Err("Circular dependency detected");
    }
    
    if visited[target_index] {
        return Ok(());
    }
    
    temp_mark[target_index] = true;
    
    // Visit dependencies
    let target = &build_system.targets[target_index];
    for input in &target.inputs {
        if let Some(dep_index) = build_system.targets.iter()
            .position(|t| t.output.as_str() == input.as_str()) {
            topological_sort_visit(build_system, dep_index, visited, temp_mark, build_order)?;
        }
    }
    
    temp_mark[target_index] = false;
    visited[target_index] = true;
    build_order.push(target_index).map_err(|_| "Build order too long")?;
    
    Ok(())
}

/// Check if all target inputs exist
pub fn check_target_inputs(build_system: &BuildSystem, target_index: usize) -> Result<bool, &'static str> {
    let target = &build_system.targets[target_index];
    
    for input in &target.inputs {
        // Check if input is a file or another target
        if !build_system.targets.iter().any(|t| t.output.as_str() == input.as_str()) {
            // Input should be a file - TODO: check if it exists
            // For now, assume it exists
        }
    }
    
    Ok(true)
}

/// Calculate hash for target inputs (for incremental builds)
pub fn calculate_target_hash(build_system: &BuildSystem, target_index: usize) -> Result<u64, &'static str> {
    let target = &build_system.targets[target_index];
    let mut hash: u64 = 0;
    
    // Simple hash calculation based on input file names
    for input in &target.inputs {
        for byte in input.as_bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);
        }
    }
    
    // Include rule command in hash
    let rule = build_system.find_rule(&target.rule)
        .ok_or("Rule not found")?;
    
    for byte in rule.command.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);
    }
    
    Ok(hash)
}

/// Get current timestamp in milliseconds
fn get_timestamp_ms() -> u64 {
    // TODO: Implement actual timestamp retrieval
    // For now, return a dummy value
    0
}

/// Clean build artifacts for a target
pub fn clean_target(build_system: &BuildSystem, target_index: usize) -> Result<(), &'static str> {
    let _target = &build_system.targets[target_index];
    
    // TODO: Remove target output file
    // For now, just return success
    
    Ok(())
}

/// Validate build graph for circular dependencies
pub fn validate_build_graph(build_system: &BuildSystem) -> Result<(), &'static str> {
    // Try to get build order - this will fail if there are circular dependencies
    get_build_order(build_system)?;
    Ok(())
}