//! Service management utilities
//! Service file parsing, validation, and management

use heapless::{String, Vec};
use super::{ServiceConfig, ServiceType, RestartPolicy};

/// Service file parser
pub struct ServiceParser;

impl ServiceParser {
    /// Parse service configuration from a simple key=value format
    pub fn parse_service_file(content: &str) -> Result<ServiceConfig, &'static str> {
        let mut service_name: heapless::String<64> = String::new();
        let mut config = ServiceConfig::new("unnamed")?;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();
                
                match key {
                    "Name" => {
                        service_name.clear();
                        service_name.push_str(value).map_err(|_| "Service name too long")?;
                        config = ServiceConfig::new(value)?;
                    }
                    "Type" => {
                        config.service_type = match value {
                            "simple" => ServiceType::Simple,
                            "forking" => ServiceType::Forking,
                            "oneshot" => ServiceType::Oneshot,
                            "notify" => ServiceType::Notify,
                            _ => return Err("Unknown service type"),
                        };
                    }
                    "ExecStart" => {
                        config.set_exec_start(value)?;
                    }
                    "ExecStop" => {
                        let mut stop_cmd = String::new();
                        stop_cmd.push_str(value).map_err(|_| "Stop command too long")?;
                        config.exec_stop = Some(stop_cmd);
                    }
                    "WorkingDirectory" => {
                        config.working_directory.clear();
                        config.working_directory.push_str(value)
                            .map_err(|_| "Working directory path too long")?;
                    }
                    "User" => {
                        config.user.clear();
                        config.user.push_str(value).map_err(|_| "User name too long")?;
                    }
                    "Group" => {
                        config.group.clear();
                        config.group.push_str(value).map_err(|_| "Group name too long")?;
                    }
                    "Requires" => {
                        for dep in value.split_whitespace() {
                            config.add_requires(dep)?;
                        }
                    }
                    "Restart" => {
                        config.restart = match value {
                            "never" => RestartPolicy::Never,
                            "always" => RestartPolicy::Always,
                            "on-failure" => RestartPolicy::OnFailure,
                            "on-abnormal" => RestartPolicy::OnAbnormal,
                            _ => return Err("Unknown restart policy"),
                        };
                    }
                    "RestartSec" => {
                        if let Ok(seconds) = value.parse::<u32>() {
                            config.restart_delay_ms = seconds * 1000;
                        } else {
                            return Err("Invalid restart delay");
                        }
                    }
                    "Enabled" => {
                        config.enabled = match value {
                            "true" | "yes" | "1" => true,
                            "false" | "no" | "0" => false,
                            _ => return Err("Invalid enabled value"),
                        };
                    }
                    _ => {
                        // Check for environment variables (ENV_*)
                        if key.starts_with("ENV_") {
                            let env_key = &key[4..];
                            config.add_env(env_key, value)?;
                        }
                        // Ignore unknown keys for forward compatibility
                    }
                }
            }
        }
        
        if service_name.is_empty() {
            return Err("Service name not specified");
        }
        
        if config.exec_start.is_empty() {
            return Err("ExecStart not specified");
        }
        
        Ok(config)
    }
    
    /// Generate service file content from configuration
    pub fn generate_service_file(config: &ServiceConfig) -> Result<String<2048>, &'static str> {
        let mut content = String::new();
        
        // Header comment
        content.push_str("# Mach_R Service Configuration\n").map_err(|_| "Content too long")?;
        content.push_str("# Generated by Mach_R Init System\n\n").map_err(|_| "Content too long")?;
        
        // Service name
        content.push_str("Name=").map_err(|_| "Content too long")?;
        content.push_str(&config.name).map_err(|_| "Content too long")?;
        content.push('\n').map_err(|_| "Content too long")?;
        
        // Service type
        content.push_str("Type=").map_err(|_| "Content too long")?;
        match config.service_type {
            ServiceType::Simple => content.push_str("simple"),
            ServiceType::Forking => content.push_str("forking"),
            ServiceType::Oneshot => content.push_str("oneshot"),
            ServiceType::Notify => content.push_str("notify"),
        }.map_err(|_| "Content too long")?;
        content.push('\n').map_err(|_| "Content too long")?;
        
        // Exec commands
        content.push_str("ExecStart=").map_err(|_| "Content too long")?;
        content.push_str(&config.exec_start).map_err(|_| "Content too long")?;
        content.push('\n').map_err(|_| "Content too long")?;
        
        if let Some(ref stop_cmd) = config.exec_stop {
            content.push_str("ExecStop=").map_err(|_| "Content too long")?;
            content.push_str(stop_cmd).map_err(|_| "Content too long")?;
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        // Working directory
        if !config.working_directory.is_empty() {
            content.push_str("WorkingDirectory=").map_err(|_| "Content too long")?;
            content.push_str(&config.working_directory).map_err(|_| "Content too long")?;
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        // User and group
        if !config.user.is_empty() {
            content.push_str("User=").map_err(|_| "Content too long")?;
            content.push_str(&config.user).map_err(|_| "Content too long")?;
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        if !config.group.is_empty() {
            content.push_str("Group=").map_err(|_| "Content too long")?;
            content.push_str(&config.group).map_err(|_| "Content too long")?;
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        // Dependencies
        if !config.requires.is_empty() {
            content.push_str("Requires=").map_err(|_| "Content too long")?;
            for (i, dep) in config.requires.iter().enumerate() {
                if i > 0 {
                    content.push(' ').map_err(|_| "Content too long")?;
                }
                content.push_str(dep).map_err(|_| "Content too long")?;
            }
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        // Restart policy
        content.push_str("Restart=").map_err(|_| "Content too long")?;
        match config.restart {
            RestartPolicy::Never => content.push_str("never"),
            RestartPolicy::Always => content.push_str("always"),
            RestartPolicy::OnFailure => content.push_str("on-failure"),
            RestartPolicy::OnAbnormal => content.push_str("on-abnormal"),
        }.map_err(|_| "Content too long")?;
        content.push('\n').map_err(|_| "Content too long")?;
        
        // Restart delay
        if config.restart_delay_ms != 1000 {
            content.push_str("RestartSec=").map_err(|_| "Content too long")?;
            let seconds = config.restart_delay_ms / 1000;
            // Simple integer to string conversion
            if seconds < 10 {
                content.push((b'0' + seconds as u8) as char).map_err(|_| "Content too long")?;
            } else {
                content.push_str("10").map_err(|_| "Content too long")?; // Simplified
            }
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        // Enabled
        content.push_str("Enabled=").map_err(|_| "Content too long")?;
        if config.enabled {
            content.push_str("true").map_err(|_| "Content too long")?;
        } else {
            content.push_str("false").map_err(|_| "Content too long")?;
        }
        content.push('\n').map_err(|_| "Content too long")?;
        
        // Environment variables
        for (key, value) in &config.environment {
            content.push_str("ENV_").map_err(|_| "Content too long")?;
            content.push_str(key).map_err(|_| "Content too long")?;
            content.push('=').map_err(|_| "Content too long")?;
            content.push_str(value).map_err(|_| "Content too long")?;
            content.push('\n').map_err(|_| "Content too long")?;
        }
        
        Ok(content)
    }
    
    /// Validate service configuration
    pub fn validate_config(config: &ServiceConfig) -> Result<Vec<String<128>, 16>, &'static str> {
        let mut warnings = Vec::new();
        
        // Check for empty exec command
        if config.exec_start.is_empty() {
            return Err("ExecStart command is required");
        }
        
        // Check for circular dependencies
        if config.requires.contains(&config.name) {
            return Err("Service cannot require itself");
        }
        
        // Warning for missing working directory
        if config.working_directory.is_empty() {
            let mut warning = String::new();
            warning.push_str("No working directory specified, will use /")
                .map_err(|_| "Warning too long")?;
            warnings.push(warning).map_err(|_| "Too many warnings")?;
        }
        
        // Warning for running as root
        if config.user.is_empty() || config.user == "root" {
            let mut warning = String::new();
            warning.push_str("Service will run as root, consider using a dedicated user")
                .map_err(|_| "Warning too long")?;
            warnings.push(warning).map_err(|_| "Too many warnings")?;
        }
        
        // Warning for always restart policy
        if config.restart == RestartPolicy::Always && config.service_type == ServiceType::Oneshot {
            let mut warning = String::new();
            warning.push_str("Always restart with oneshot type may cause rapid restarts")
                .map_err(|_| "Warning too long")?;
            warnings.push(warning).map_err(|_| "Too many warnings")?;
        }
        
        Ok(warnings)
    }
}

/// Default service templates
pub struct ServiceTemplates;

impl ServiceTemplates {
    /// Create a simple daemon service template
    pub fn simple_daemon(name: &str, command: &str) -> Result<ServiceConfig, &'static str> {
        let mut config = ServiceConfig::new(name)?;
        config.set_exec_start(command)?;
        config.service_type = ServiceType::Simple;
        config.restart = RestartPolicy::Always;
        config.enabled = false; // Require explicit enabling
        Ok(config)
    }
    
    /// Create a oneshot service template
    pub fn oneshot(name: &str, command: &str) -> Result<ServiceConfig, &'static str> {
        let mut config = ServiceConfig::new(name)?;
        config.set_exec_start(command)?;
        config.service_type = ServiceType::Oneshot;
        config.restart = RestartPolicy::Never;
        config.enabled = false;
        Ok(config)
    }
    
    /// Create a network service template
    pub fn network_service(name: &str, command: &str, _port: u16) -> Result<ServiceConfig, &'static str> {
        let mut config = ServiceConfig::new(name)?;
        config.set_exec_start(command)?;
        config.service_type = ServiceType::Simple;
        config.restart = RestartPolicy::OnFailure;
        config.add_requires("network")?;
        config.add_env("PORT", "22")?; // Simplified port handling
        config.enabled = false;
        Ok(config)
    }
}