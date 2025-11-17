//! Build target management
//! Handles target creation, dependency resolution, and build ordering

use super::{BuildSystem, Target};
use heapless::{String, Vec};
use alloc::format;

/// Create a standard compilation target
pub fn create_compile_target(
    build_system: &mut BuildSystem,
    source: &str,
    output: &str,
    rule: &str,
) -> Result<(), &'static str> {
    let mut target = Target::new(output, rule)?;
    target.add_input(source)?;
    build_system.add_target(target)?;
    Ok(())
}

/// Create a linking target with multiple object files
pub fn create_link_target(
    build_system: &mut BuildSystem,
    objects: &[&str],
    output: &str,
    libs: &[&str],
) -> Result<(), &'static str> {
    let mut target = Target::new(output, "link")?;
    
    for obj in objects {
        target.add_input(obj)?;
    }
    
    if !libs.is_empty() {
        let mut lib_string = String::<512>::new();
        for (i, lib) in libs.iter().enumerate() {
            if i > 0 {
                lib_string.push(' ').map_err(|_| "Library list too long")?;
            }
            lib_string.push_str(lib).map_err(|_| "Library list too long")?;
        }
        target.set_variable("libs", &lib_string)?;
    }
    
    build_system.add_target(target)?;
    Ok(())
}

/// Create a Rust compilation target
pub fn create_rust_target(
    build_system: &mut BuildSystem,
    source: &str,
    output: &str,
    target_triple: Option<&str>,
) -> Result<(), &'static str> {
    let mut target = Target::new(output, "rustc")?;
    target.add_input(source)?;
    
    if let Some(triple) = target_triple {
        target.set_variable("target", triple)?;
    }
    
    build_system.add_target(target)?;
    Ok(())
}

/// Create a cargo build target
pub fn create_cargo_target(
    build_system: &mut BuildSystem,
    manifest_path: &str,
    output: &str,
    profile: Option<&str>,
) -> Result<(), &'static str> {
    let mut target = Target::new(output, "cargo")?;
    target.add_input(manifest_path)?;
    
    if let Some(prof) = profile {
        let cargo_flags = match prof {
            "release" => "--release",
            "dev" => "",
            _ => "",
        };
        target.set_variable("cargoflags", cargo_flags)?;
    }
    
    build_system.add_target(target)?;
    Ok(())
}

/// Create a copy target
pub fn create_copy_target(
    build_system: &mut BuildSystem,
    source: &str,
    dest: &str,
) -> Result<(), &'static str> {
    let mut target = Target::new(dest, "cp")?;
    target.add_input(source)?;
    build_system.add_target(target)?;
    Ok(())
}

/// Create a directory creation target
pub fn create_mkdir_target(
    build_system: &mut BuildSystem,
    dir_path: &str,
) -> Result<(), &'static str> {
    let target = Target::new(dir_path, "mkdir")?;
    build_system.add_target(target)?;
    Ok(())
}

/// Create a test target
pub fn create_test_target(
    build_system: &mut BuildSystem,
    test_name: &str,
    workdir: &str,
) -> Result<(), &'static str> {
    let mut target = Target::new(&format!("test_{}", test_name), "test")?;
    target.set_variable("workdir", workdir)?;
    build_system.add_target(target)?;
    Ok(())
}

/// Create a documentation target
pub fn create_doc_target(
    build_system: &mut BuildSystem,
    package: &str,
    workdir: &str,
) -> Result<(), &'static str> {
    let mut target = Target::new(&format!("doc_{}", package), "doc")?;
    target.set_variable("workdir", workdir)?;
    build_system.add_target(target)?;
    Ok(())
}

/// Create a clean target that removes build artifacts
pub fn create_clean_target(
    build_system: &mut BuildSystem,
    artifacts: &[&str],
) -> Result<(), &'static str> {
    let mut target = Target::new("clean", "rm")?;
    
    for artifact in artifacts {
        target.add_input(artifact)?;
    }
    
    build_system.add_target(target)?;
    Ok(())
}

/// Create targets for a typical C project
pub fn create_c_project_targets(
    build_system: &mut BuildSystem,
    sources: &[&str],
    output: &str,
    libs: &[&str],
) -> Result<(), &'static str> {
    let mut objects = Vec::<String<512>, 32>::new();
    
    // Create compilation targets for each source file
    for source in sources {
        let obj_path = source.replace(".c", ".o");
        let mut obj_string = String::new();
        obj_string.push_str(&obj_path).map_err(|_| "Object path too long")?;
        objects.push(obj_string).map_err(|_| "Too many objects")?;
        
        create_compile_target(build_system, source, &obj_path, "cc")?;
    }
    
    // Create linking target
    let obj_strs: Vec<&str, 32> = objects.iter().map(|s| s.as_str()).collect();
    create_link_target(build_system, &obj_strs, output, libs)?;
    
    Ok(())
}

/// Create targets for a typical Rust project
pub fn create_rust_project_targets(
    build_system: &mut BuildSystem,
    manifest_path: &str,
    bin_name: &str,
) -> Result<(), &'static str> {
    // Create cargo build target
    let output = format!("target/release/{}", bin_name);
    create_cargo_target(build_system, manifest_path, &output, Some("release"))?;
    
    // Create test target
    create_test_target(build_system, "all", ".")?;
    
    // Create doc target
    create_doc_target(build_system, bin_name, ".")?;
    
    Ok(())
}

/// Create Mach_R kernel build targets
pub fn create_mach_r_targets(build_system: &mut BuildSystem) -> Result<(), &'static str> {
    // Kernel binary target
    create_rust_target(
        build_system,
        "src/main.rs",
        "target/aarch64-unknown-none/release/mach_r",
        Some("aarch64-unknown-none"),
    )?;
    
    // Disk image target
    let mut disk_target = Target::new("mach_r.img", "disk")?;
    disk_target.add_input("target/aarch64-unknown-none/release/mach_r")?;
    build_system.add_target(disk_target)?;
    
    // QEMU test target
    let mut qemu_target = Target::new("test_qemu", "qemu")?;
    qemu_target.add_input("target/aarch64-unknown-none/release/mach_r")?;
    build_system.add_target(qemu_target)?;
    
    // UTM configuration target
    let mut utm_target = Target::new("Mach_R.utm", "utm")?;
    utm_target.add_input("mach_r.img")?;
    build_system.add_target(utm_target)?;
    
    Ok(())
}