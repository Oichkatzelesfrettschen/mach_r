//! Build system implementation for Mach_R
//! Pure Rust, no_std compatible build system inspired by Ninja

use heapless::{String, Vec, FnvIndexMap};

pub mod rules;
pub mod targets;
pub mod executor;

/// Maximum number of build targets
const MAX_TARGETS: usize = 1024;
/// Maximum number of build rules
const MAX_RULES: usize = 64;
/// Maximum number of dependencies per target
const MAX_DEPS: usize = 32;

/// Build target representation
#[derive(Debug, Clone)]
pub struct Target {
    /// Target output path
    pub output: String<512>,
    /// Input files
    pub inputs: Vec<String<512>, MAX_DEPS>,
    /// Rule name to use
    pub rule: String<64>,
    /// Target-specific variables
    pub variables: FnvIndexMap<String<64>, String<256>, 16>,
    /// Target hash for incremental builds
    pub hash: u64,
    /// Last build timestamp
    pub timestamp: u64,
}

impl Target {
    /// Create a new build target
    pub fn new(output: &str, rule: &str) -> Result<Self, &'static str> {
        let mut target_output = String::new();
        target_output.push_str(output).map_err(|_| "Output path too long")?;
        
        let mut target_rule = String::new();
        target_rule.push_str(rule).map_err(|_| "Rule name too long")?;
        
        Ok(Self {
            output: target_output,
            inputs: Vec::new(),
            rule: target_rule,
            variables: FnvIndexMap::new(),
            hash: 0,
            timestamp: 0,
        })
    }
    
    /// Add input dependency
    pub fn add_input(&mut self, input: &str) -> Result<(), &'static str> {
        let mut input_path = String::new();
        input_path.push_str(input).map_err(|_| "Input path too long")?;
        self.inputs.push(input_path).map_err(|_| "Too many inputs")?;
        Ok(())
    }
    
    /// Set target variable
    pub fn set_variable(&mut self, name: &str, value: &str) -> Result<(), &'static str> {
        let mut var_name = String::new();
        var_name.push_str(name).map_err(|_| "Variable name too long")?;
        
        let mut var_value = String::new();
        var_value.push_str(value).map_err(|_| "Variable value too long")?;
        
        self.variables.insert(var_name, var_value)
            .map_err(|_| "Too many variables")?;
        Ok(())
    }
    
    /// Check if target needs rebuilding
    pub fn needs_rebuild(&self) -> bool {
        // TODO: Implement proper timestamp/hash checking
        // For now, always rebuild
        true
    }
}

/// Build rule definition
#[derive(Debug, Clone)]
pub struct Rule {
    /// Rule name
    pub name: String<64>,
    /// Command template
    pub command: String<1024>,
    /// Description template
    pub description: String<256>,
    /// Rule variables
    pub variables: FnvIndexMap<String<64>, String<256>, 16>,
}

impl Rule {
    /// Create a new build rule
    pub fn new(name: &str, command: &str) -> Result<Self, &'static str> {
        let mut rule_name = String::new();
        rule_name.push_str(name).map_err(|_| "Rule name too long")?;
        
        let mut rule_command = String::new();
        rule_command.push_str(command).map_err(|_| "Command too long")?;
        
        Ok(Self {
            name: rule_name,
            command: rule_command,
            description: String::new(),
            variables: FnvIndexMap::new(),
        })
    }
    
    /// Set rule description
    pub fn set_description(&mut self, desc: &str) -> Result<(), &'static str> {
        self.description.clear();
        self.description.push_str(desc).map_err(|_| "Description too long")?;
        Ok(())
    }
    
    /// Set rule variable
    pub fn set_variable(&mut self, name: &str, value: &str) -> Result<(), &'static str> {
        let mut var_name = String::new();
        var_name.push_str(name).map_err(|_| "Variable name too long")?;
        
        let mut var_value = String::new();
        var_value.push_str(value).map_err(|_| "Variable value too long")?;
        
        self.variables.insert(var_name, var_value)
            .map_err(|_| "Too many variables")?;
        Ok(())
    }
    
    /// Expand command template with variables
    pub fn expand_command(&self, target: &Target, global_vars: &FnvIndexMap<String<64>, String<256>, 32>) -> Result<String<1024>, &'static str> {
        let mut result = String::new();
        let command_str = self.command.as_str();
        let mut chars = command_str.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek().is_some() {
                let mut var_name = String::<64>::new();
                
                // Handle ${var} syntax
                if chars.peek() == Some(&'{') {
                    chars.next(); // consume '{'
                    while let Some(ch) = chars.next() {
                        if ch == '}' {
                            break;
                        }
                        var_name.push(ch).map_err(|_| "Variable name too long")?;
                    }
                } else {
                    // Handle $var syntax
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            var_name.push(ch).map_err(|_| "Variable name too long")?;
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                
                // Look up variable in order: target, rule, global
                let value = target.variables.get(&var_name)
                    .or_else(|| self.variables.get(&var_name))
                    .or_else(|| global_vars.get(&var_name))
                    .map(|s| s.as_str());
                
                if let Some(val) = value {
                    result.push_str(val).map_err(|_| "Expanded command too long")?;
                } else {
                    // Handle built-in variables
                    match var_name.as_str() {
                        "out" => result.push_str(&target.output).map_err(|_| "Expanded command too long")?,
                        "in" => {
                            // Join all inputs with spaces
                            for (i, input) in target.inputs.iter().enumerate() {
                                if i > 0 {
                                    result.push(' ').map_err(|_| "Expanded command too long")?;
                                }
                                result.push_str(input).map_err(|_| "Expanded command too long")?;
                            }
                        }
                        _ => {
                            // Unknown variable - leave as is
                            result.push('$').map_err(|_| "Expanded command too long")?;
                            result.push_str(&var_name).map_err(|_| "Expanded command too long")?;
                        }
                    }
                }
            } else {
                result.push(ch).map_err(|_| "Expanded command too long")?;
            }
        }
        
        Ok(result)
    }
}

/// Build graph and execution context
pub struct BuildSystem {
    /// All build targets
    pub targets: Vec<Target, MAX_TARGETS>,
    /// All build rules
    pub rules: Vec<Rule, MAX_RULES>,
    /// Global variables
    pub variables: FnvIndexMap<String<64>, String<256>, 32>,
    /// Build parallelism level
    pub parallelism: usize,
    /// Verbose output flag
    pub verbose: bool,
}

impl BuildSystem {
    /// Create a new build system
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            rules: Vec::new(),
            variables: FnvIndexMap::new(),
            parallelism: 1,
            verbose: false,
        }
    }
    
    /// Add a build rule
    pub fn add_rule(&mut self, rule: Rule) -> Result<(), &'static str> {
        self.rules.push(rule).map_err(|_| "Too many rules")?;
        Ok(())
    }
    
    /// Add a build target
    pub fn add_target(&mut self, target: Target) -> Result<(), &'static str> {
        self.targets.push(target).map_err(|_| "Too many targets")?;
        Ok(())
    }
    
    /// Set global variable
    pub fn set_variable(&mut self, name: &str, value: &str) -> Result<(), &'static str> {
        let mut var_name = String::new();
        var_name.push_str(name).map_err(|_| "Variable name too long")?;
        
        let mut var_value = String::new();
        var_value.push_str(value).map_err(|_| "Variable value too long")?;
        
        self.variables.insert(var_name, var_value)
            .map_err(|_| "Too many global variables")?;
        Ok(())
    }
    
    /// Find rule by name
    pub fn find_rule(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.name.as_str() == name)
    }
    
    /// Find target by output path
    pub fn find_target(&self, output: &str) -> Option<&Target> {
        self.targets.iter().find(|target| target.output.as_str() == output)
    }
    
    /// Find target by output path (mutable)
    pub fn find_target_mut(&mut self, output: &str) -> Option<&mut Target> {
        self.targets.iter_mut().find(|target| target.output.as_str() == output)
    }
    
    /// Build a specific target
    pub fn build_target(&mut self, output: &str) -> Result<bool, &'static str> {
        executor::build_target(self, output)
    }
    
    /// Build all targets
    pub fn build_all(&mut self) -> Result<bool, &'static str> {
        let mut all_success = true;
        
        for i in 0..self.targets.len() {
            let output = self.targets[i].output.clone();
            let success = self.build_target(&output)?;
            if !success {
                all_success = false;
                if !self.verbose {
                    break; // Stop on first error unless verbose
                }
            }
        }
        
        Ok(all_success)
    }
    
    /// Clean all build outputs
    pub fn clean(&mut self) -> Result<(), &'static str> {
        // TODO: Remove all target output files
        Ok(())
    }
    
    /// Get build statistics
    pub fn get_stats(&self) -> BuildStats {
        BuildStats {
            target_count: self.targets.len(),
            rule_count: self.rules.len(),
            variable_count: self.variables.len(),
        }
    }
}

/// Build statistics
#[derive(Debug)]
pub struct BuildStats {
    pub target_count: usize,
    pub rule_count: usize,
    pub variable_count: usize,
}

static mut BUILD_SYSTEM: Option<BuildSystem> = None;

/// Initialize the build system
pub fn init() -> Result<(), &'static str> {
    let mut build_system = BuildSystem::new();
    
    // Add default rules
    rules::add_default_rules(&mut build_system)?;
    
    unsafe {
        BUILD_SYSTEM = Some(build_system);
    }
    
    Ok(())
}

/// Get the global build system
pub fn get_build_system() -> Option<&'static mut BuildSystem> {
    unsafe { BUILD_SYSTEM.as_mut() }
}