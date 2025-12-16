// Build automation tool - suppress style lints
#![allow(clippy::needless_borrow)]
#![allow(clippy::useless_format)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_find)]

use sha2::{Digest, Sha256};
use shell_escape::escape;
use std::borrow::Cow;
use std::env;
use std::fs;
use std::fs::read_dir;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::process::Command;

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[RUN] {:?}", cmd);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("command failed: {:?}", cmd);
    }
    Ok(())
}

fn run_sudo(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[RUN SUDO] {:?}", cmd);
    let status = Command::new("sudo")
        .args(cmd.get_program().to_str().unwrap().split_whitespace())
        .args(cmd.get_args())
        .status()?;
    if !status.success() {
        anyhow::bail!("sudo command failed: {:?}", cmd);
    }
    Ok(())
}

fn cargo() -> Command {
    Command::new("cargo")
}
fn rustup() -> Command {
    Command::new("rustup")
}

fn root() -> anyhow::Result<PathBuf> {
    // xtask runs in workspace: synthesis/
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir.parent().unwrap().to_path_buf())
}

fn ensure_dirs() -> anyhow::Result<(PathBuf, PathBuf)> {
    let r = root()?;
    let build = r.join("build");
    let dist = r.join("build/dist");
    fs::create_dir_all(&dist)?;
    Ok((build, dist))
}

fn task_fmt() -> anyhow::Result<()> {
    run(cargo().args(["fmt"]))
}

fn task_fmt_check() -> anyhow::Result<()> {
    run(cargo().args(["fmt", "--", "--check"]))
}

fn task_clippy() -> anyhow::Result<()> {
    run(cargo().args([
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ]))
}

fn task_test() -> anyhow::Result<()> {
    run(cargo().args(["test", "--lib"]))
}

fn task_check() -> anyhow::Result<()> {
    // Ensure generated code is up-to-date before checks
    let _ = task_mig();
    task_fmt_check()?;
    task_clippy()?;
    task_test()
}

fn task_book() -> anyhow::Result<()> {
    if !have("mdbook") {
        eprintln!("[WARN] mdbook not found; install with: cargo install mdbook");
        return Ok(());
    }
    let mut cmd = Command::new("mdbook");
    cmd.args(["build", "docs/book"]);
    run(&mut cmd)
}

fn task_docs() -> anyhow::Result<()> {
    run(cargo().args(["doc", "--no-deps"]))?;
    task_book()
}

fn task_qemu_fast() -> anyhow::Result<()> {
    task_qemu_with_args(&["--cpus".into(), "1".into(), "--mem".into(), "512M".into()])
}

fn task_qemu_dev() -> anyhow::Result<()> {
    task_qemu_with_args(&[
        "--cpus".into(),
        "4".into(),
        "--mem".into(),
        "2G".into(),
        "--".into(),
        "-d".into(),
        "guest_errors".into(),
    ])
}

fn build_kernel_for_target_profile(
    target: &str,
    profile: &str,
) -> anyhow::Result<(PathBuf, PathBuf)> {
    let (_build, dist) = ensure_dirs()?;
    let root_dir = root()?;
    let cargo_profile_flag = if profile == "release" {
        "--release"
    } else {
        ""
    };
    let final_profile_dir = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    eprintln!(
        "[BUILD] Building {} kernel for {} profile...",
        target, profile
    );

    let mut cmd = Command::new("cross");
    cmd.args(
        [
            "build",
            cargo_profile_flag,
            "--target",
            target,
            "--bin",
            "mach_r",
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>(),
    );
    run(&mut cmd)?;

    let elf_path_in_target =
        root_dir.join(format!("target/{}/{}/mach_r", target, final_profile_dir));
    let out_elf = dist.join(format!("mach_r_kernel_{}.elf", target));
    fs::copy(&elf_path_in_target, &out_elf)?;

    let out_bin = dist.join(format!("mach_r_kernel_{}.bin", target));
    if let Some(objcopy) = find_objcopy() {
        eprintln!(
            "[OBJCOPY] Using {} to create binary for {} -> {}",
            objcopy,
            target,
            out_bin.display()
        );
        let status = Command::new(objcopy)
            .args([
                "-O",
                "binary",
                elf_path_in_target.to_str().unwrap(),
                out_bin.to_str().unwrap(),
            ])
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                eprintln!(
                    "[WARN] objcopy failed for {}; copying ELF as bin fallback",
                    target
                );
                fs::copy(&elf_path_in_target, &out_bin)?;
            }
        }
    } else {
        eprintln!(
            "[WARN] objcopy not found; copying ELF as bin for {}",
            target
        );
        fs::copy(&elf_path_in_target, &out_bin)?;
    }

    let elf_size = out_elf.metadata()?.len();
    let bin_size = out_bin.metadata()?.len();
    eprintln!("[SIZE] {}.elf: {} bytes", target, elf_size);
    eprintln!("[SIZE] {}.bin: {} bytes", target, bin_size);
    eprintln!("[ARTIFACT] {}", out_elf.display());
    eprintln!("[ARTIFACT] {}", out_bin.display());

    Ok((out_elf, out_bin))
}

fn task_kernel() -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;

    let mut all_artifacts: Vec<(String, PathBuf)> = Vec::new();

    // Build for AArch664
    let (aarch64_elf, aarch64_bin) =
        build_kernel_for_target_profile("aarch64-unknown-none", "release")?;
    all_artifacts.push((
        format!("mach_r_kernel_{}.elf", "aarch64-unknown-none"),
        aarch64_elf,
    ));
    all_artifacts.push((
        format!("mach_r_kernel_{}.bin", "aarch64-unknown-none"),
        aarch64_bin,
    ));

    // Build for x86_64
    let (x86_64_elf, x86_64_bin) =
        build_kernel_for_target_profile("x86_64-unknown-none", "release")?;
    all_artifacts.push((
        format!("mach_r_kernel_{}.elf", "x86_64-unknown-none"),
        x86_64_elf,
    ));
    all_artifacts.push((
        format!("mach_r_kernel_{}.bin", "x86_64-unknown-none"),
        x86_64_bin,
    ));

    // Write SHA256SUMS
    let sums_path = dist.join("SHA256SUMS");
    let mut sums_content = String::new();
    for (name, path) in &all_artifacts {
        if path.exists() {
            sums_content.push_str(&format!("{}  {}\n", sha256_file(path)?, name));
        }
    }
    fs::write(&sums_path, sums_content)?;
    eprintln!("[ARTIFACT] {}", sums_path.display());

    // Update manifest with all generated artifacts
    let _ = write_manifest_with_artifacts(all_artifacts)?;

    Ok(())
}

fn task_bootloader(args: &[String]) -> anyhow::Result<()> {
    eprintln!("[BOOTLOADER] Building Mach_R bootloader...");

    let mut target = "aarch64-unknown-none"; // Default
    let mut profile = "debug"; // Default

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--target" if i + 1 < args.len() => {
                target = args[i + 1].as_str();
                i += 2;
            }
            "--release" => {
                profile = "release";
                i += 1;
            }
            _ => {
                // Ignore other arguments for now
                i += 1;
            }
        }
    }

    eprintln!(
        "[BOOTLOADER] Building for {} (profile: {})...",
        target, profile
    );

    run(rustup().args(["target", "add", target]))?;

    let mut cmd = cargo();
    cmd.arg("build").arg("--lib").arg("--target").arg(target);

    if profile == "release" {
        cmd.arg("--release");
    }

    // Add RUSTFLAGS for warnings as in the original script
    // This requires setting an environment variable for the command
    cmd.env("RUSTFLAGS", "-A warnings");

    run(&mut cmd)?;

    eprintln!(
        "[BOOTLOADER] Build successful for {} (profile: {})!",
        target, profile
    );
    Ok(())
}

fn task_utm() -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    let img_dir = build.join("images"); // QCOW2 is in build/images
    let qcow2 = img_dir.join("mach_r.qcow2");

    // Ensure QCOW2 exists
    if !qcow2.exists() {
        task_disk_image()?;
    }

    let utm_bundle_name = "Mach_R.utm";
    let utm_bundle_path = dist.join(utm_bundle_name);
    let utm_images_path = utm_bundle_path.join("Images");

    eprintln!("[UTM] Creating UTM bundle at {}", utm_bundle_path.display());

    fs::create_dir_all(&utm_images_path)?;
    fs::copy(&qcow2, utm_images_path.join("mach_r.qcow2"))?;

    // Create config.plist
    let config_plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>Name</key><string>Mach_R OS</string>
    <key>Architecture</key><string>aarch64</string>
    <key>Memory</key><integer>2048</integer>
    <key>Boot</key><string>kernel</string>
    <key>Kernel</key><string>mach_r_kernel.bin</string>
    <key>DiskImages</key>
    <array>
        <dict>
            <key>ImageName</key><string>mach_r.qcow2</string>
            <key>Interface</key><string>virtio</string>
        </dict>
    </array>
</dict>
</plist>"#
    );
    fs::write(utm_bundle_path.join("config.plist"), config_plist_content)?;

    eprintln!("[UTM] Bundle created: {}", utm_bundle_path.display());
    // Update manifest
    let _ = write_manifest_with_artifacts(Vec::new())?;
    Ok(())
}

fn task_qemu_kernel(args: &[String]) -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    let mut qemu_target_arch_str = "aarch64-unknown-none".to_string(); // Default
    let mut qemu_system_cmd = "qemu-system-aarch64";
    let mut qemu_machine = "virt";
    let mut qemu_cpu = "cortex-a72";

    // Parse args for target architecture
    let mut parsed_args = args.to_vec();
    // Allow --target <arch> to specify the qemu system and kernel
    if let Some(pos) = parsed_args.iter().position(|a| a == "--target") {
        if let Some(target_val) = parsed_args.get(pos + 1) {
            qemu_target_arch_str = match target_val.as_str() {
                "aarch64" => "aarch64-unknown-none".to_string(),
                "x86_64" => "x86_64-unknown-none".to_string(),
                _ => anyhow::bail!("Unsupported QEMU target architecture: {}", target_val),
            };
            match qemu_target_arch_str.as_str() {
                "aarch64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-aarch64";
                    qemu_machine = "virt";
                    qemu_cpu = "cortex-a72";
                }
                "x86_64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-x86_64";
                    qemu_machine = "pc";
                    qemu_cpu = "host";
                }
                _ => unreachable!(), // Handled by bail above
            }
            parsed_args.remove(pos); // Remove --target
            parsed_args.remove(pos); // Remove <arch>
        }
    }
    let qemu_target_arch = qemu_target_arch_str.as_str();

    let kernel_elf_name = format!("mach_r_kernel_{}.elf", qemu_target_arch);
    let _kernel = dist.join(&kernel_elf_name);
    let kernel_bin_name = format!("mach_r_kernel_{}.bin", qemu_target_arch);
    let kernel_bin = dist.join(&kernel_bin_name);

    // Ensure kernel is built for the selected architecture
    if !kernel_bin.exists() {
        eprintln!(
            "[INFO] Kernel binary for {} not found, building...",
            qemu_target_arch
        );
        // Call task_kernel without arguments, as it builds both aarch64 and x86_64 in release.
        // It's not ideal if only one is needed, but simplifies the call for now.
        // A future enhancement could allow task_kernel to build specific targets.
        task_kernel()?;
        if !kernel_bin.exists() {
            anyhow::bail!(
                "Failed to build kernel binary {} for {}",
                kernel_bin.display(),
                qemu_target_arch
            );
        }
    }

    // Check if the appropriate QEMU system command exists
    if !have(qemu_system_cmd) {
        anyhow::bail!(
            "{} not available. Please install QEMU for {} systems.",
            qemu_system_cmd,
            qemu_target_arch
        );
    }

    eprintln!("[QEMU] Direct kernel boot for {}...", qemu_target_arch);
    let status = Command::new(qemu_system_cmd)
        .args([
            "-M",
            qemu_machine,
            "-cpu",
            qemu_cpu,
            "-smp",
            "4",
            "-m",
            "2G",
            "-kernel",
            kernel_bin.to_str().unwrap(), // Use the .bin file for direct kernel boot
            "-nographic",
            "-serial",
            "mon:stdio",
        ])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) | Err(_) => anyhow::bail!("{} failed or not available", qemu_system_cmd),
    }
}

fn have(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

fn find_objcopy() -> Option<&'static str> {
    for c in [
        "llvm-objcopy",
        "rust-objcopy",
        "aarch64-linux-gnu-objcopy",
        "aarch64-elf-objcopy",
        "objcopy",
    ] {
        if have(c) {
            return Some(c);
        }
    }
    None
}

fn sha256_file(path: &PathBuf) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

fn append_checksum(path: &PathBuf, name: &str) -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    let sums_path = dist.join("SHA256SUMS");
    let sum = sha256_file(path)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&sums_path)?;
    writeln!(f, "{}  {}", sum, name)?;
    eprintln!("[CHECKSUM] {}  {}", sum, name);
    Ok(())
}

fn print_version(cmd: &str) {
    let out = Command::new(cmd).arg("--version").output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            if s.is_empty() {
                s = String::from_utf8_lossy(&o.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
            }
            if s.is_empty() {
                s = "<no version output>".into();
            }
            eprintln!("[VER] {}: {}", cmd, s);
        }
        Err(_) => eprintln!("[VER] {}: not found", cmd),
    }
}

fn tool_version(cmd: &str) -> Option<String> {
    let out = Command::new(cmd).arg("--version").output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    if s.is_empty() {
        s = String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
    }
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn write_manifest_with_artifacts(extra_artifacts: Vec<(String, PathBuf)>) -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    let mut manifest = serde_json::json!({
        "versions": {
            "rustc": tool_version("rustc"),
            "cargo": tool_version("cargo"),
            "qemu-system-aarch64": tool_version("qemu-system-aarch64"),
            "qemu-img": tool_version("qemu-img"),
            "objcopy": find_objcopy(),
        },
        "artifacts": [],
        "meta": {
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs()),
            "xtask_version": env!("CARGO_PKG_VERSION"),
        }
    });
    let mut arts: Vec<(String, PathBuf)> = extra_artifacts; // Start with extra artifacts
    let build = root()?.join("build/images");

    // Add other fixed artifacts
    if dist.join("mach_r_kernel.elf").exists() {
        // Old naming convention
        arts.push(("mach_r_kernel.elf".into(), dist.join("mach_r_kernel.elf")));
        arts.push(("mach_r_kernel.bin".into(), dist.join("mach_r_kernel.bin")));
    }

    // Ensure all target specific kernel artifacts are included.
    // Assuming they are named mach_r_kernel_{target}.elf/bin
    let target_prefix = "mach_r_kernel_";
    for entry in fs::read_dir(&dist)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
            if file_name.starts_with(target_prefix)
                && (file_name.ends_with(".elf") || file_name.ends_with(".bin"))
            {
                if !arts.iter().any(|(name, _)| name == file_name) {
                    arts.push((file_name.to_string(), path));
                }
            }
        }
    }

    arts.push(("mach_r.qcow2".into(), build.join("mach_r.qcow2")));
    arts.push(("mach_r.iso".into(), build.join("mach_r.iso")));
    arts.push(("Mach_R.utm".into(), dist.join("Mach_R.utm")));
    let mut list = vec![];
    for (name, path) in arts {
        if path.exists() {
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            let sum = sha256_file(&path).ok();
            list.push(serde_json::json!({ "name": name, "path": path.to_string_lossy(), "bytes": size, "sha256": sum }));
        }
    }
    manifest["artifacts"] = serde_json::Value::Array(list);
    let out = dist.join("MANIFEST.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&manifest)?)?;
    eprintln!("[ARTIFACT] {}", out.display());
    Ok(())
}

#[derive(Default, Debug)]
struct QemuOpts {
    cpu: String,
    cpus: String,
    mem: String,
    display: Option<String>,
    gui: bool,
    extra: Vec<String>,
    vga: Option<String>,
    no_reboot: bool,
    debug_flags: Option<String>,
    logfile: Option<PathBuf>,
}

fn parse_qemu_opts(args: &[String]) -> QemuOpts {
    let mut q = read_qemu_config().unwrap_or(QemuOpts {
        cpu: "cortex-a72".into(),
        cpus: "4".into(),
        mem: "2G".into(),
        display: None,
        gui: false,
        extra: Vec::new(),
        vga: None,
        no_reboot: false,
        debug_flags: None,
        logfile: None,
    });
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cpu" if i + 1 < args.len() => {
                q.cpu = args[i + 1].clone();
                i += 2;
            }
            "--cpus" if i + 1 < args.len() => {
                q.cpus = args[i + 1].clone();
                i += 2;
            }
            "--mem" if i + 1 < args.len() => {
                q.mem = args[i + 1].clone();
                i += 2;
            }
            "--display" if i + 1 < args.len() => {
                q.display = Some(args[i + 1].clone());
                q.gui = true;
                i += 2;
            }
            "--gui" => {
                q.gui = true;
                i += 1;
            }
            "--vga" if i + 1 < args.len() => {
                q.vga = Some(args[i + 1].clone());
                i += 2;
            }
            "--no-reboot" => {
                q.no_reboot = true;
                i += 1;
            }
            "--debug-flags" if i + 1 < args.len() => {
                q.debug_flags = Some(args[i + 1].clone());
                i += 2;
            }
            "--logfile" if i + 1 < args.len() => {
                q.logfile = Some(PathBuf::from(args[i + 1].clone()));
                i += 2;
            }
            "--" => {
                q.extra.extend_from_slice(&args[i + 1..]);
                break;
            }
            other => {
                q.extra.push(other.to_string());
                i += 1;
            }
        }
    }
    q
}

fn read_qemu_config() -> Option<QemuOpts> {
    // Simple ~/.mach_r_qemu.toml
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".mach_r_qemu.toml");
    if !path.exists() {
        return None;
    }
    let s = std::fs::read_to_string(path).ok()?;
    let v: toml::Value = toml::from_str(&s).ok()?;
    let mut o = QemuOpts::default();
    if let Some(cpu) = v.get("cpu").and_then(|x| x.as_str()) {
        o.cpu = cpu.to_string();
    }
    if let Some(cpus) = v.get("cpus").and_then(|x| x.as_integer()) {
        o.cpus = cpus.to_string();
    }
    if let Some(mem) = v.get("mem").and_then(|x| x.as_str()) {
        o.mem = mem.to_string();
    }
    if let Some(gui) = v.get("gui").and_then(|x| x.as_bool()) {
        o.gui = gui;
    }
    if let Some(display) = v.get("display").and_then(|x| x.as_str()) {
        o.display = Some(display.to_string());
    }
    if let Some(vga) = v.get("vga").and_then(|x| x.as_str()) {
        o.vga = Some(vga.to_string());
    }
    if let Some(no_reboot) = v.get("no_reboot").and_then(|x| x.as_bool()) {
        o.no_reboot = no_reboot;
    }
    if let Some(debug_flags) = v.get("debug_flags").and_then(|x| x.as_str()) {
        o.debug_flags = Some(debug_flags.to_string());
    }
    if let Some(logfile) = v.get("logfile").and_then(|x| x.as_str()) {
        o.logfile = Some(PathBuf::from(logfile.to_string()));
    }
    if let Some(arr) = v.get("extra").and_then(|x| x.as_array()) {
        o.extra = arr
            .iter()
            .filter_map(|e| e.as_str().map(|s| s.to_string()))
            .collect();
    }
    Some(o)
}

fn task_filesystem() -> anyhow::Result<()> {
    let (build, _dist) = ensure_dirs()?;
    let sysroot = build.join("sysroot");
    for d in [
        "bin", "dev", "etc", "lib", "proc", "sys", "tmp", "usr", "var",
    ] {
        fs::create_dir_all(sysroot.join(d))?;
    }
    fs::write(sysroot.join("etc/issue"), "Mach_R 0.1.0\n")?;
    fs::write(sysroot.join("etc/hostname"), "mach_r\n")?;
    eprintln!("[FS] Created sysroot at {}", sysroot.display());
    Ok(())
}

fn task_env_check() -> anyhow::Result<()> {
    // Check rustup components
    eprintln!("[INFO] Checking rustup components...");
    run(rustup()
        .args(["component", "add", "rust-src"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()))?;
    run(rustup()
        .args(["component", "add", "rustfmt"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()))?;
    run(rustup()
        .args(["component", "add", "clippy"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()))?;
    eprintln!("[INFO] Rustup components checked/installed.");

    // Check external tools
    let mut missing_tools = Vec::new();

    // Check for qemu-system-aarch64
    if !have("qemu-system-aarch64") {
        missing_tools.push("qemu-system-aarch64");
        eprintln!("[WARN] qemu-system-aarch64 not found. Install QEMU for ARM systems.");
        eprintln!("[INFO] On macOS: `brew install qemu`");
        eprintln!("[INFO] On Ubuntu/Debian: `sudo apt install qemu-system-arm`");
    }

    // Check for qemu-img
    if !have("qemu-img") {
        missing_tools.push("qemu-img");
        eprintln!("[WARN] qemu-img not found. Install QEMU utilities.");
        eprintln!("[INFO] On macOS: `brew install qemu`");
        eprintln!("[INFO] On Ubuntu/Debian: `sudo apt install qemu-utils`");
    }

    // Check for dd
    if !have("dd") {
        missing_tools.push("dd");
        eprintln!("[WARN] dd not found. This is usually part of coreutils. On macOS: `brew install coreutils`");
    }

    // Check for grub-mkrescue/grub2-mkrescue
    if !have("grub-mkrescue") && !have("grub2-mkrescue") {
        missing_tools.push("grub-mkrescue/grub2-mkrescue");
        eprintln!(
            "[WARN] grub-mkrescue or grub2-mkrescue not found. Used for x86_64 ISO creation."
        );
        eprintln!("[INFO] On Ubuntu/Debian: `sudo apt install grub-pc-bin xorriso`");
        eprintln!("[INFO] On Fedora: `sudo dnf install grub2-tools-extra xorriso`");
    }

    // Check for xorriso
    if !have("xorriso") {
        missing_tools.push("xorriso");
        eprintln!("[WARN] xorriso not found. Used for ISO creation.");
        eprintln!("[INFO] On macOS: `brew install xorriso`");
        eprintln!("[INFO] On Ubuntu/Debian: `sudo apt install xorriso`");
        eprintln!("[INFO] On Fedora: `sudo dnf install xorriso`");
    }

    // Check for docker-compose
    if !have("docker-compose") {
        missing_tools.push("docker-compose");
        eprintln!("[WARN] docker-compose not found. Needed for containerized builds.");
        eprintln!("[INFO] Install: `brew install docker-compose` (macOS) or `sudo apt install docker-compose` (Linux)");
    }

    // Check for macOS-specific tools
    #[cfg(target_os = "macos")]
    {
        if !have("hdiutil") {
            missing_tools.push("hdiutil");
            eprintln!("[WARN] hdiutil not found. This is a macOS utility, ensure Xcode Command Line Tools are installed: `xcode-select --install`");
        }
        // Check for Rosetta 2 on ARM Macs
        let arch = std::env::var("UNAME_MACHINE").unwrap_or_default();
        if arch == "arm64" {
            let output = Command::new("pgrep").arg("oahd").output();
            match output {
                Ok(out) if out.stdout.is_empty() => {
                    eprintln!("[INFO] Rosetta 2 not running. Attempting to install/enable...");
                    let install_output = Command::new("softwareupdate")
                        .args(["--install-rosetta", "--agree-to-license"])
                        .output();
                    if let Ok(install_res) = install_output {
                        if install_res.status.success() {
                            eprintln!("[INFO] Rosetta 2 installation/enablement successful.");
                        } else {
                            eprintln!("[WARN] Rosetta 2 installation/enablement failed. This may impact x86_64 emulation.");
                            eprintln!("Stdout: {}", String::from_utf8_lossy(&install_res.stdout));
                            eprintln!("Stderr: {}", String::from_utf8_lossy(&install_res.stderr));
                        }
                    } else {
                        eprintln!("[WARN] Failed to run softwareupdate for Rosetta 2. This may impact x86_64 emulation.");
                    }
                }
                Err(_) => {
                    eprintln!("[WARN] Could not check pgrep for oahd (Rosetta 2 daemon). Proceeding without explicit check.");
                }
                _ => { /* oahd is running */ }
            }
        }
    }

    // Check for Linux-specific tools
    #[cfg(not(target_os = "macos"))]
    {
        if !have("genisoimage") {
            missing_tools.push("genisoimage");
            eprintln!("[WARN] genisoimage not found. Used for ISO creation.");
            eprintln!("[INFO] On Ubuntu/Debian: `sudo apt install genisoimage`");
        }
    }

    if !missing_tools.is_empty() {
        eprintln!("[ERROR] Some external tools are missing. Please install them to proceed with all xtask functionalities.");
        anyhow::bail!("Missing external tools: {}", missing_tools.join(", "));
    }

    // Print tool versions
    print_version("rustc");
    print_version("cargo");
    print_version("qemu-system-aarch64");
    print_version("qemu-img");
    if let Some(obj) = find_objcopy() {
        eprintln!("[OBJCOPY] using {}", obj);
        print_version(obj);
    } else {
        eprintln!("[OBJCOPY] none found (will fallback to copying ELF)");
    }
    // Check target
    let out = Command::new("rustup")
        .args(["target", "list", "--installed"]) // ok to fail softly
        .output()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if !s.contains("aarch64-unknown-none") {
        eprintln!("[INFO] Installing rust target aarch64-unknown-none...");
        run(rustup().args(["target", "add", "aarch64-unknown-none"]))?;
    }
    if !s.contains("x86_64-unknown-none") {
        eprintln!("[INFO] Installing rust target x86_64-unknown-none...");
        run(rustup().args(["target", "add", "x86_64-unknown-none"]))?;
    }
    eprintln!("[OK] Environment looks good.");
    Ok(())
}

fn task_disk_image() -> anyhow::Result<()> {
    task_disk_image_with_args(&[])
}

fn task_disk_image_with_args(args: &[String]) -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    let img_dir = build.join("images");
    fs::create_dir_all(&img_dir)?;

    let mut kernel_bin_name = "mach_r_kernel_x86_64-unknown-none.bin".to_string(); // Default x86_64 kernel
    let mut output_img_name = "mach_r.img".to_string();
    let mut output_qcow2_name = "mach_r.qcow2".to_string();
    let mut img_size_mb = 128; // Default image size

    // Parse arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--kernel" if i + 1 < args.len() => {
                kernel_bin_name = args[i + 1].clone();
                i += 2;
            }
            "--output-img" if i + 1 < args.len() => {
                output_img_name = args[i + 1].clone();
                i += 2;
            }
            "--output-qcow2" if i + 1 < args.len() => {
                output_qcow2_name = args[i + 1].clone();
                i += 2;
            }
            "--size" if i + 1 < args.len() => {
                img_size_mb = args[i + 1].parse()?;
                i += 2;
            }
            _ => {
                eprintln!("[WARN] Unknown argument for disk-image: {}", args[i]);
                i += 1;
            }
        }
    }

    let kernel_bin_path = dist.join(&kernel_bin_name);
    let output_img_path = img_dir.join(&output_img_name);
    let output_qcow2_path = img_dir.join(&output_qcow2_name);

    // Ensure kernel exists
    if !kernel_bin_path.exists() {
        eprintln!(
            "[INFO] Kernel binary {} not found, building for x86_64-unknown-none release...",
            kernel_bin_path.display()
        );
        // Assume x86_64-unknown-none release for disk image
        build_kernel_for_target_profile("x86_64-unknown-none", "release")?;
        if !kernel_bin_path.exists() {
            anyhow::bail!(
                "Failed to build kernel binary {} for disk image.",
                kernel_bin_path.display()
            );
        }
    }

    eprintln!(
        "[DISK] Creating bootable x86_64 disk image at {} ({}MB)...",
        output_img_path.display(),
        img_size_mb
    );

    // Step 1: Create raw disk image
    eprintln!(
        "[DISK] Step 1: Creating raw disk image ({}MB)...",
        img_size_mb
    );
    let _ = fs::remove_file(&output_img_path); // Remove if exists
    run(Command::new("dd")
        .arg(format!("if=/dev/zero"))
        .arg(format!("of={}", output_img_path.display()))
        .arg("bs=1M")
        .arg(format!("count={}", img_size_mb))
        .arg("status=progress"))?;

    // Step 2: Create partition table
    eprintln!("[DISK] Step 2: Creating partition table...");
    run_sudo(
        &mut Command::new("parted")
            .arg(output_img_path.to_str().unwrap())
            .arg("-s")
            .arg("mklabel")
            .arg("msdos"),
    )?;
    run_sudo(
        &mut Command::new("parted")
            .arg(output_img_path.to_str().unwrap())
            .arg("-s")
            .arg("mkpart")
            .arg("primary")
            .arg("ext2")
            .arg("1MiB")
            .arg("100%"),
    )?;
    run_sudo(
        &mut Command::new("parted")
            .arg(output_img_path.to_str().unwrap())
            .arg("-s")
            .arg("set")
            .arg("1")
            .arg("boot")
            .arg("on"),
    )?;

    // Step 3: Format partition and install GRUB
    eprintln!("[DISK] Step 3: Formatting partition and installing GRUB...");

    // Check for losetup
    if !have("losetup") {
        anyhow::bail!("losetup not found. Install util-linux.");
    }
    let loop_device_output = Command::new("sudo").arg("losetup").arg("-f").output()?;
    let loop_device = String::from_utf8_lossy(&loop_device_output.stdout)
        .trim()
        .to_string();
    if loop_device.is_empty() {
        anyhow::bail!("No free loop device available. Cannot create bootable disk image.");
    }

    run_sudo(
        &mut Command::new("losetup")
            .arg("-P")
            .arg(&loop_device)
            .arg(output_img_path.to_str().unwrap()),
    )?;

    // Check for mkfs.ext2
    if !have("mkfs.ext2") {
        anyhow::bail!("mkfs.ext2 not found. Install e2fsprogs.");
    }
    run_sudo(&mut Command::new("mkfs.ext2").arg(format!("{}p1", loop_device)))?; // Partition 1

    let mount_point = build.join(format!("mnt_boot_{}", std::process::id()));
    fs::create_dir_all(&mount_point)?;

    run_sudo(
        &mut Command::new("mount")
            .arg(format!("{}p1", loop_device))
            .arg(mount_point.to_str().unwrap()),
    )?;

    // Create GRUB directory and copy kernel
    let grub_dir = mount_point.join("boot/grub");
    run_sudo(
        &mut Command::new("mkdir")
            .arg("-p")
            .arg(grub_dir.to_str().unwrap()),
    )?;
    run_sudo(
        &mut Command::new("cp")
            .arg(kernel_bin_path.to_str().unwrap())
            .arg(mount_point.join("boot/kernel.bin").to_str().unwrap()),
    )?;

    // Create GRUB configuration
    let grub_cfg_content = r#"set timeout=0
set default=0

menuentry "Mach_R Microkernel" {
    multiboot2 /boot/kernel.bin
    boot
}
"#;
    let escaped_content = escape(grub_cfg_content.into());
    let grub_cfg_file = grub_dir.join("grub.cfg");
    let escaped_path = escape(Cow::Borrowed(grub_cfg_file.to_str().unwrap()));
    run_sudo(
        &mut Command::new("bash")
            .arg("-c")
            .arg(format!("echo {} | tee {}", escaped_content, escaped_path)),
    )?;

    // Install GRUB bootloader
    eprintln!("[DISK] Installing GRUB to MBR...");
    if !have("grub-install") {
        anyhow::bail!("grub-install not found. Install grub-pc-bin.");
    }

    run_sudo(
        &mut Command::new("grub-install")
            .arg(format!("--target=i386-pc"))
            .arg(format!(
                "--boot-directory={}",
                mount_point.join("boot").display()
            ))
            .arg("--modules=part_msdos ext2 multiboot2")
            .arg(&loop_device),
    )?;

    // Cleanup
    eprintln!("[DISK] Step 4: Cleaning up...");
    run_sudo(&mut Command::new("umount").arg(mount_point.to_str().unwrap()))?;
    run_sudo(&mut Command::new("losetup").arg("-d").arg(&loop_device))?;
    fs::remove_dir_all(&mount_point)?;

    // Step 5: Convert to QCOW2
    eprintln!("[DISK] Step 5: Converting to QCOW2...");
    if have("qemu-img") {
        let _ = fs::remove_file(&output_qcow2_path); // Remove if exists
        run(Command::new("qemu-img").args([
            "convert",
            "-f",
            "raw",
            "-O",
            "qcow2",
            output_img_path.to_str().unwrap(),
            output_qcow2_path.to_str().unwrap(),
        ]))?;
        append_checksum(&output_qcow2_path, &output_qcow2_name)?;
    } else {
        eprintln!("[WARN] qemu-img not found; QCOW2 image will not be created.");
    }

    append_checksum(&output_img_path, &output_img_name)?;
    eprintln!("[ARTIFACT] {}", output_img_path.display());
    eprintln!("[ARTIFACT] {}", output_qcow2_path.display());

    let _ = write_manifest_with_artifacts(Vec::new())?; // Update manifest

    eprintln!("[DISK] Bootable disk image creation complete.");
    Ok(())
}

fn task_iso_image_with_args(args: &[String]) -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    let iso_path = build.join("images/mach_r.iso");
    fs::create_dir_all(build.join("images"))?;

    let mut kernel_target = "aarch64-unknown-none"; // Default
    let mut parsed_args = args.to_vec();

    if let Some(pos) = parsed_args.iter().position(|a| a == "--target") {
        if let Some(target_val) = parsed_args.get(pos + 1) {
            kernel_target = match target_val.as_str() {
                "aarch64" => "aarch64-unknown-none",
                "x86_64" => "x86_64-unknown-none",
                _ => anyhow::bail!("Unsupported ISO target architecture: {}", target_val),
            };
            parsed_args.remove(pos); // Remove --target
            parsed_args.remove(pos); // Remove <arch>
        }
    }

    if kernel_target == "x86_64-unknown-none" {
        // --- x86_64 with GRUB2 Multiboot2 ---
        eprintln!("[ISO] Creating bootable ISO for x86_64 with GRUB2...");
        let iso_root = build.join("iso_x86_64");
        let grub_dir = iso_root.join("boot/grub");
        fs::create_dir_all(&grub_dir)?;

        // Ensure kernel is built
        let kernel_bin_name = format!("mach_r_kernel_{}.bin", kernel_target);
        let kernel_bin_path = dist.join(&kernel_bin_name);
        if !kernel_bin_path.exists() {
            eprintln!("[INFO] x86_64 kernel binary not found, building...");
            task_kernel()?;
            if !kernel_bin_path.exists() {
                anyhow::bail!("Failed to build x86_64 kernel binary for ISO.");
            }
        }
        fs::copy(&kernel_bin_path, iso_root.join("boot/mach_r.bin"))?;

        // Create GRUB configuration
        let grub_cfg_content = r#"set timeout=0
set default=0

menuentry "Mach_R x86_64 Kernel" {
    multiboot2 /boot/mach_r.bin
    boot
}
"#;
        fs::write(grub_dir.join("grub.cfg"), grub_cfg_content)?;

        // Try grub-mkrescue or xorriso
        if have("grub-mkrescue") {
            eprintln!("[ISO] Using grub-mkrescue...");
            run(Command::new("grub-mkrescue").args([
                "-o",
                iso_path.to_str().unwrap(),
                iso_root.to_str().unwrap(),
            ]))?;
        } else if have("grub2-mkrescue") {
            // Some systems use grub2-mkrescue
            eprintln!("[ISO] Using grub2-mkrescue...");
            run(Command::new("grub2-mkrescue").args([
                "-o",
                iso_path.to_str().unwrap(),
                iso_root.to_str().unwrap(),
            ]))?;
        } else if have("xorriso") {
            eprintln!("[ISO] Using xorriso...");
            run(Command::new("xorriso").args([
                "-as",
                "mkisofs",
                "-o",
                iso_path.to_str().unwrap(),
                "-b",
                "boot/grub/grub.cfg", // GRUB EFI
                "-no-emul-boot",
                "-boot-load-size",
                "4",
                "-boot-info-table",
                iso_root.to_str().unwrap(),
            ]))?;
        } else {
            anyhow::bail!("No suitable ISO creation tool found (grub-mkrescue, grub2-mkrescue, xorriso). Please install one.");
        }
    } else {
        // --- AArch64 or unspecified target (direct boot) ---
        eprintln!("[ISO] Creating direct-boot ISO for AArch64...");
        let iso_root = build.join("iso_aarch64");
        let iso_boot = iso_root.join("boot");
        fs::create_dir_all(&iso_boot)?;

        let kernel_bin_name = format!("mach_r_kernel_{}.bin", kernel_target);
        let kernel_bin_path = dist.join(&kernel_bin_name);
        if !kernel_bin_path.exists() {
            eprintln!("[INFO] AArch64 kernel binary not found, building...");
            task_kernel()?;
            if !kernel_bin_path.exists() {
                anyhow::bail!("Failed to build AArch64 kernel binary for ISO.");
            }
        }
        fs::copy(&kernel_bin_path, iso_boot.join("kernel.bin"))?;

        let readme = iso_root.join("README.txt");
        fs::write(
            &readme,
            b"Mach_R ISO\n\nThis ISO contains the kernel (boot/kernel.bin).\nUse QEMU or your bootloader tooling to load the kernel.\n\nExample (direct kernel boot):\n  qemu-system-aarch64 -M virt -cpu cortex-a72 -m 2G -kernel boot/kernel.bin -nographic -serial mon:stdio\n",
        )?;

        #[cfg(target_os = "macos")]
        {
            if have("hdiutil") {
                run(Command::new("hdiutil").args([
                    "makehybrid",
                    "-o",
                    iso_path.to_str().unwrap(),
                    iso_root.to_str().unwrap(),
                ]))?;
            } else {
                eprintln!("[WARN] hdiutil not found; skipping ISO creation on macOS.");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if have("genisoimage") {
                run(Command::new("genisoimage").args([
                    "-R",
                    "-J",
                    "-o",
                    iso_path.to_str().unwrap(),
                    iso_root.to_str().unwrap(),
                ]))?;
            } else {
                eprintln!("[WARN] genisoimage not found; skipping ISO creation.");
            }
        }
    }

    if iso_path.exists() {
        append_checksum(&iso_path, "mach_r.iso")?;
    }
    eprintln!("[ARTIFACT] {}", iso_path.display());
    let _ = write_manifest_with_artifacts(Vec::new())?;
    Ok(())
}

fn task_qemu_with_args(args: &[String]) -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    let qcow2 = build.join("images/mach_r.qcow2");

    let opts = parse_qemu_opts(args);

    let mut qemu_system_cmd = "qemu-system-aarch64";
    let mut qemu_machine = "virt";
    let _qemu_cpu = &opts.cpu; // Use parsed CPU
    let mut kernel_target = "aarch64-unknown-none";

    // Determine QEMU system command and parameters based on target architecture,
    // which can be passed via --target argument to xtask
    let temp_args = args.to_vec(); // Use a mutable copy of args for parsing
    if let Some(pos) = temp_args.iter().position(|a| a == "--target") {
        if let Some(target_val) = temp_args.get(pos + 1) {
            kernel_target = match target_val.as_str() {
                "aarch64" => "aarch64-unknown-none",
                "x86_64" => "x86_64-unknown-none",
                _ => anyhow::bail!("Unsupported QEMU target architecture: {}", target_val),
            };
            match kernel_target {
                "aarch64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-aarch64";
                    qemu_machine = "virt";
                    // qemu_cpu is already from opts, so no change here unless specific to arch
                }
                "x86_64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-x86_64";
                    qemu_machine = "pc";
                    // qemu_cpu is already from opts, so no change here unless specific to arch
                }
                _ => unreachable!(), // Handled by bail above
            }
        }
    }

    // Ensure kernel is built for the selected architecture
    let kernel_bin_name = format!("mach_r_kernel_{}.bin", kernel_target);
    let kernel_bin = dist.join(&kernel_bin_name);

    if !kernel_bin.exists() {
        eprintln!(
            "[INFO] Kernel binary for {} not found, building...",
            kernel_target
        );
        task_kernel()?; // Builds all kernels
        if !kernel_bin.exists() {
            anyhow::bail!(
                "Failed to build kernel binary {} for {}",
                kernel_bin.display(),
                kernel_target
            );
        }
    }

    if !have(qemu_system_cmd) {
        anyhow::bail!(
            "{} not found. Please install QEMU for {} systems.",
            qemu_system_cmd,
            kernel_target
        );
    }

    let mut cmd = Command::new(qemu_system_cmd);

    cmd.args(["-M", qemu_machine])
        .args(["-cpu", &opts.cpu])
        .args(["-smp", &opts.cpus])
        .args(["-m", &opts.mem]);

    if let Some(vga) = &opts.vga {
        cmd.args(["-vga", vga]);
    }

    if opts.no_reboot {
        cmd.arg("-no-reboot");
    }

    if let Some(debug_flags) = &opts.debug_flags {
        cmd.args(["-d", debug_flags]);
    }

    if let Some(logfile) = &opts.logfile {
        cmd.args(["-D", logfile.to_str().unwrap()]);
    }

    cmd.args([
        "-drive",
        &format!("if=virtio,format=qcow2,file={}", qcow2.display()),
    ])
    .args(["-kernel", kernel_bin.to_str().unwrap()])
    .args(["-device", "virtio-net-pci,netdev=net0"])
    .args(["-netdev", "user,id=net0"]);

    if opts.gui {
        if let Some(d) = &opts.display {
            cmd.args(["-display", d]);
        }
    } else {
        cmd.arg("-nographic");
    }
    cmd.args(["-serial", "mon:stdio"]);
    if !opts.extra.is_empty() {
        cmd.args(opts.extra);
    }
    run(&mut cmd)
}

fn task_qemu_debug(args: &[String]) -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;

    let opts = parse_qemu_opts(args); // Parse args again for debug options

    let mut qemu_target_arch_str = "aarch64-unknown-none".to_string(); // Default
    let mut qemu_system_cmd = "qemu-system-aarch64";
    let mut qemu_machine = "virt";
    let qemu_cpu = &opts.cpu;

    // Parse args for target architecture
    let temp_args = args.to_vec();
    if let Some(pos) = temp_args.iter().position(|a| a == "--target") {
        if let Some(target_val) = temp_args.get(pos + 1) {
            qemu_target_arch_str = match target_val.as_str() {
                "aarch64" => "aarch64-unknown-none".to_string(),
                "x86_64" => "x86_64-unknown-none".to_string(),
                _ => anyhow::bail!("Unsupported QEMU target architecture: {}", target_val),
            };
            match qemu_target_arch_str.as_str() {
                "aarch64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-aarch64";
                    qemu_machine = "virt";
                }
                "x86_64-unknown-none" => {
                    qemu_system_cmd = "qemu-system-x86_64";
                    qemu_machine = "pc";
                }
                _ => unreachable!(),
            }
        }
    }
    let qemu_target_arch = qemu_target_arch_str.as_str();

    let kernel_elf_name = format!("mach_r_kernel_{}.elf", qemu_target_arch);
    let _kernel_elf = dist.join(&kernel_elf_name);
    let kernel_bin_name = format!("mach_r_kernel_{}.bin", qemu_target_arch);
    let kernel_bin = dist.join(&kernel_bin_name);

    // Ensure kernel is built for the selected architecture
    if !kernel_bin.exists() {
        eprintln!(
            "[INFO] Kernel binary for {} not found, building...",
            qemu_target_arch
        );
        task_kernel()?;
        if !kernel_bin.exists() {
            anyhow::bail!(
                "Failed to build kernel binary {} for {}",
                kernel_bin.display(),
                qemu_target_arch
            );
        }
    }

    // Check if the appropriate QEMU system command exists
    if !have(qemu_system_cmd) {
        anyhow::bail!(
            "{} not available. Please install QEMU for {} systems.",
            qemu_system_cmd,
            qemu_target_arch
        );
    }

    eprintln!(
        "[QEMU] Starting with GDB server for {}...",
        qemu_target_arch
    );
    let mut cmd = Command::new(qemu_system_cmd);
    cmd.args([
        "-M",
        qemu_machine,
        "-cpu",
        qemu_cpu,
        "-smp",
        &opts.cpus,
        "-m",
        &opts.mem,
        "-kernel",
        kernel_bin.to_str().unwrap(),
        "-nographic",
        "-serial",
        "mon:stdio",
        "-s",
        "-S",
    ]);

    // Apply general QEMU options from opts
    if let Some(vga) = &opts.vga {
        cmd.args(["-vga", vga]);
    }
    if opts.no_reboot {
        cmd.arg("-no-reboot");
    }
    if let Some(debug_flags) = &opts.debug_flags {
        cmd.args(["-d", debug_flags]);
    }
    if let Some(logfile) = &opts.logfile {
        cmd.args(["-D", logfile.to_str().unwrap()]);
    }
    if !opts.extra.is_empty() {
        cmd.args(&opts.extra);
    }

    run(&mut cmd)
}

fn print_help() {
    eprintln!(
        "xtask commands:\n  fmt | fmt-check | clippy | test | check | env-check | clean | all\n  docs | book\n  kernel | bootloader | filesystem | disk-image | iso-image | utm\n  qemu | qemu-fast | qemu-dev | qemu-kernel | qemu-debug\n  mig   # generate MIG stubs (e.g., name_server)\n\nContainer commands:\n  setup-container-env   # Generate Dockerfile and docker-compose.yml\n  build-in-container [--target <target>] [--release]   # Build kernel(s) inside container\n  test-in-container     # Run tests inside container\n  shell-in-container    # Open a bash shell inside container\n  clean-in-container    # Run cargo clean inside container\n  rebuild-container-image  # Rebuild docker container image with --no-cache\n  qemu-in-container     # Run QEMU inside container (x86_64 release build only) \n\nQEMU options (before --): --cpu <name> | --cpus <n> | --mem <size> | --display <mode> | --gui | --vga <type> | --no-reboot | --debug-flags <flags> | --logfile <file> | -- <extra qemu args>\n\nExamples:\n  cargo run -p xtask -- fmt\n  cargo run -p xtask -- kernel\n  cargo run -p xtask -- disk-image
  cargo run -p xtask -- iso-image --target x86_64\n  cargo run -p xtask -- qemu --mem 1G --cpus 2 -- -d guest_errors\n  cargo run -p xtask -- qemu --gui --display default\n  cargo run -p xtask -- qemu-kernel --target x86_64\n  cargo run -p xtask -- qemu-debug --target x86_64\n  cargo run -p xtask -- mig\n  cargo run -p xtask -- utm\n  cargo run -p xtask -- clean\n  cargo run -p xtask -- all\n  cargo run -p xtask -- verify-targets\n  cargo run -p xtask -- test-boot\n  cargo run -p xtask -- setup-container-env\n  cargo run -p xtask -- build-in-container --target x86_64 --release\n  cargo run -p xtask -- test-in-container\n  cargo run -p xtask -- shell-in-container\n  cargo run -p xtask -- clean-in-container\n  cargo run -p xtask -- rebuild-container-image\n  cargo run -p xtask -- qemu-in-container"
    );
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    match cmd.as_str() {
        "fmt" => task_fmt(),
        "fmt-check" => task_fmt_check(),
        "clippy" => task_clippy(),
        "test" => task_test(),
        "check" => task_check(),
        "book" => task_book(),
        "docs" => task_docs(),
        "kernel" => task_kernel(),
        "bootloader" => {
            let rest: Vec<String> = args.collect();
            task_bootloader(&rest)
        }
        "filesystem" => task_filesystem(),
        "disk-image" => {
            let rest: Vec<String> = args.collect();
            task_disk_image_with_args(&rest)
        }
        "iso-image" => {
            let rest: Vec<String> = args.collect();
            task_iso_image_with_args(&rest)
        }
        "qemu-fast" => task_qemu_fast(),
        "qemu-dev" => task_qemu_dev(),
        "qemu" => {
            let rest: Vec<String> = args.collect();
            task_qemu_with_args(&rest)
        }
        "qemu-kernel" => {
            let rest: Vec<String> = args.collect();
            task_qemu_kernel(&rest)
        }
        "qemu-debug" => {
            let rest: Vec<String> = args.collect();
            task_qemu_debug(&rest)
        }
        "env-check" => task_env_check(),
        "mig" => task_mig(),
        "utm" => task_utm(),
        "clean" => task_clean(),
        "all" => task_all(),

        _ => {
            print_help();
            Ok(())
        }
    }
}

fn task_clean() -> anyhow::Result<()> {
    eprintln!("[CLEAN] Cleaning build artifacts...");
    // Run cargo clean
    run(cargo().args(["clean"]))?;

    // Remove the build directory
    let root_dir = root()?;
    let build_dir = root_dir.join("build");
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir)?;
        eprintln!("[CLEAN] Removed build directory: {}", build_dir.display());
    } else {
        eprintln!("[CLEAN] Build directory not found, skipping removal.");
    }
    Ok(())
}

fn task_all() -> anyhow::Result<()> {
    eprintln!("[BUILD ALL] Starting full build process...");
    task_env_check()?;
    task_kernel()?;
    task_filesystem()?;
    task_disk_image()?;
    task_iso_image_with_args(&[])?;
    task_utm()?;
    eprintln!("[BUILD ALL] Full build process completed successfully!");
    Ok(())
}

#[allow(dead_code)]
fn build_lib_for_target(target: &str) -> anyhow::Result<()> {
    eprintln!("[BUILD LIB] Building library for {}...", target);
    run(rustup()
        .args(["target", "add", target])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()))?;
    let status = cargo()
        .args(["build", "--lib", "--target", target])
        .status()?;

    if status.success() {
        eprintln!("[BUILD LIB] ✅ {} library build successful", target);
        Ok(())
    } else {
        eprintln!("[BUILD LIB] ❌ {} library build failed", target);
        anyhow::bail!("Failed to build library for {}", target);
    }
}

fn task_mig() -> anyhow::Result<()> {
    // Generate MIG stubs from TOML specs in mig/specs/
    let root_dir = root()?;
    let specs_dir = root_dir.join("mig/specs");
    let gen_dir = root_dir.join("src/mig/generated");
    std::fs::create_dir_all(&gen_dir)?;

    if !specs_dir.exists() {
        eprintln!(
            "[MIG] No specs found at {} — creating example spec",
            specs_dir.display()
        );
        std::fs::create_dir_all(&specs_dir)?;
        std::fs::write(
            specs_dir.join("name_server.toml"),
            r#"name = "name_server"
subsystem = 1000

[[routines]]
name = "register"
id = 1000
[[routines.inputs]]
name = "name"
type = "string"
[[routines.inputs]]
name = "port"
type = "port"

[[routines]]
name = "lookup"
id = 1001
[[routines.inputs]]
name = "name"
type = "string"
[[routines.outputs]]
name = "port"
type = "port"

[[routines]]
name = "unregister"
id = 1002
[[routines.inputs]]
name = "name"
type = "string"
"#,
        )?;
    }

    let mut modules: Vec<String> = Vec::new();
    for entry in read_dir(&specs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let spec_src = std::fs::read_to_string(&path)?;
        let spec: MigSpec = toml::from_str(&spec_src)?;
        let module_name = spec.name.clone();
        let out_rs = gen_dir.join(format!("{}.rs", module_name));
        std::fs::write(&out_rs, render_spec(&spec))?;
        eprintln!("[MIG] Generated {} -> {}", path.display(), out_rs.display());
        modules.push(module_name);
    }

    // Write mod.rs
    let mut mod_rs_content = String::new();
    for m in &modules {
        mod_rs_content.push_str(&format!("pub mod {};\n", m));
    }
    if mod_rs_content.is_empty() {
        mod_rs_content.push_str("// no specs\n");
    }
    std::fs::write(gen_dir.join("mod.rs"), mod_rs_content)?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct MigSpec {
    name: String,
    #[allow(dead_code)]
    subsystem: Option<u32>,
    routines: Vec<MigRoutine>,
}

#[derive(serde::Deserialize)]
struct MigRoutine {
    name: String,
    id: u32,
    #[serde(default)]
    #[allow(dead_code)]
    inputs: Vec<MigField>,
    #[serde(default)]
    #[allow(dead_code)]
    outputs: Vec<MigField>,
}

#[derive(serde::Deserialize)]
struct MigField {
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    typ: String,
}

fn render_spec(spec: &MigSpec) -> String {
    let mut s = String::new();
    s.push_str("// @generated by xtask mig\n");
    s.push_str(&format!("// module: {}\n\n", spec.name));
    s.push_str("use crate::types::PortId;\n");
    s.push_str("use crate::port::Port;\n");
    s.push_str("use alloc::sync::Arc;\n");
    s.push_str("use crate::message::Message;\n\n");
    for r in &spec.routines {
        s.push_str(&format!(
            "pub const {}_ID: u32 = {};\n",
            r.name.to_uppercase(),
            r.id
        ));
    }
    s.push('\n');
    s.push_str("pub trait NameService {\n");
    for r in &spec.routines {
        let sig = render_trait_sig(r);
        s.push_str(&format!("    {}\n", sig));
    }
    s.push_str("}\n\n");
    s.push_str("pub struct NameClient { server: Arc<Port> }\n\n");
    s.push_str("impl NameClient {\n");
    s.push_str("    pub fn new(server: Arc<Port>) -> Self { Self { server } }\n");
    for r in &spec.routines {
        let (sig, body) = render_client_method(r);
        if sig.trim_start().starts_with("// unsupported") {
            s.push_str(&format!("    {}\n", sig));
        } else {
            s.push_str(&format!("    {} {{\n{}    }}\n\n", sig, body));
        }
    }
    if spec.name == "name_server" {
        // only for name_server module
        // sync call variants for simple round-trips
        s.push_str("    pub fn register_call(&self, name: &str, port: PortId, reply: &Arc<Port>) -> i32 {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(REGISTER_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(name.as_bytes());\n");
        s.push_str("        data.extend_from_slice(&(port.0 as u64).to_le_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return -1; }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d = rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); return i32::from_le_bytes(rc);} }\n");
        s.push_str("        -1\n    }\n");
        s.push_str("    pub fn lookup_call(&self, name: &str, reply: &Arc<Port>) -> Result<PortId, i32> {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(LOOKUP_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(name.as_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return Err(-1); }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d = rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); let code=i32::from_le_bytes(rc); if code==0 { if d.len()>=12 { let mut pb=[0u8;8]; pb.copy_from_slice(&d[4..12]); return Ok(PortId(u64::from_le_bytes(pb))); } else { return Err(-1); } } else { return Err(code); } } }\n");
        s.push_str("        Err(-1)\n    }\n");
        s.push_str("    pub fn unregister_call(&self, name: &str, reply: &Arc<Port>) -> i32 {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(UNREGISTER_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(name.as_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return -1; }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d = rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); return i32::from_le_bytes(rc);} }\n");
        s.push_str("        -1\n    }\n");
    }
    if spec.name == "vm" {
        s.push_str("    pub fn allocate_call(&self, size: u64, prot: u32, reply: &Arc<Port>) -> Result<u64, i32> {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(ALLOCATE_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&size.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&prot.to_le_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return Err(-1); }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d = rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); let code=i32::from_le_bytes(rc); if code==0 { if d.len()>=12 { let mut ab=[0u8;8]; ab.copy_from_slice(&d[4..12]); return Ok(u64::from_le_bytes(ab)); } else { return Err(-1);} } else { return Err(code);} } }\n");
        s.push_str("        Err(-1)\n    }\n");
        s.push_str(
            "    pub fn deallocate_call(&self, addr: u64, size: u64, reply: &Arc<Port>) -> i32 {\n",
        );
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(DEALLOCATE_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&addr.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&size.to_le_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return -1; }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d=rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); return i32::from_le_bytes(rc);} }\n");
        s.push_str("        -1\n    }\n");
        s.push_str("    pub fn protect_call(&self, addr: u64, size: u64, prot: u32, reply: &Arc<Port>) -> i32 {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(PROTECT_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&addr.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&size.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&prot.to_le_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return -1; }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d=rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); return i32::from_le_bytes(rc);} }\n");
        s.push_str("        -1\n    }\n");
    }
    if spec.name == "pager" {
        s.push_str("    pub fn page_request_call(&self, object_id: u64, offset: u64, size: u32, prot: u32, reply: &Arc<Port>) -> Result<u64, i32> {\n");
        s.push_str("        let mut data = alloc::vec::Vec::new();\n");
        s.push_str("        data.extend_from_slice(&(PAGE_REQUEST_ID as u32).to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&object_id.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&offset.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&size.to_le_bytes());\n");
        s.push_str("        data.extend_from_slice(&prot.to_le_bytes());\n");
        s.push_str("        let msg = Message::new_out_of_line(self.server.id(), data).with_reply_port(reply.id());\n");
        s.push_str("        if self.server.send(msg).is_err() { return Err(-1); }\n");
        s.push_str("        if let Some(rep) = reply.receive() { let d = rep.data(); if d.len()>=4 { let mut rc=[0;4]; rc.copy_from_slice(&d[0..4]); let code=i32::from_le_bytes(rc); if code==0 { if d.len()>=12 { let mut pb=[0u8;8]; pb.copy_from_slice(&d[4..12]); return Ok(u64::from_le_bytes(pb)); } else { return Err(-1); } } else { return Err(code); } } }\n");
        s.push_str("        Err(-1)\n    }\n");
    }
    s.push_str("}\n\n");

    // server-side dispatch
    s.push_str("pub fn dispatch<T: NameService>(svc: &T, msg: &Message) -> Option<Message> {\n");
    s.push_str("    let data = msg.data();\n");
    s.push_str("    if data.len() < 4 { return None; }\n");
    s.push_str("    let mut idb = [0u8;4]; idb.copy_from_slice(&data[0..4]);\n");
    s.push_str("    let msg_id = u32::from_le_bytes(idb);\n");
    s.push_str("    let mut off = 4usize;\n");
    s.push_str("    let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());\n");
    s.push_str("    match msg_id {\n");
    // cases for known routines
    for r in &spec.routines {
        match r.name.as_str() {
            "register" => {
                s.push_str("        REGISTER_ID => {\n");
                s.push_str("            if data.len() < off + 4 { return None; }\n");
                s.push_str("            let mut lb=[0u8;4]; lb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let nlen = u32::from_le_bytes(lb) as usize;\n");
                s.push_str("            if data.len() < off + nlen + 8 { return None; }\n");
                s.push_str("            let name = core::str::from_utf8(&data[off..off+nlen]).ok()?; off+=nlen;\n");
                s.push_str("            let mut pb=[0u8;8]; pb.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let port = PortId(u64::from_le_bytes(pb));\n");
                s.push_str("            let rc = svc.register(name, port);\n");
                s.push_str("            let mut out = alloc::vec::Vec::new();\n");
                s.push_str("            out.extend_from_slice(&(rc as i32).to_le_bytes());\n");
                s.push_str("            out.extend_from_slice(&(port.0 as u64).to_le_bytes());\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            "lookup" => {
                s.push_str("        LOOKUP_ID => {\n");
                s.push_str("            if data.len() < off + 4 { return None; }\n");
                s.push_str("            let mut lb=[0u8;4]; lb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let nlen = u32::from_le_bytes(lb) as usize;\n");
                s.push_str("            if data.len() < off + nlen { return None; }\n");
                s.push_str("            let name = core::str::from_utf8(&data[off..off+nlen]).ok()?; off+=nlen;\n");
                s.push_str("            match svc.lookup(name) {\n");
                s.push_str("                Ok(pid) => {\n");
                s.push_str("                    let mut out = alloc::vec::Vec::new();\n");
                s.push_str("                    out.extend_from_slice(&(0i32).to_le_bytes());\n");
                s.push_str(
                    "                    out.extend_from_slice(&(pid.0 as u64).to_le_bytes());\n",
                );
                s.push_str(
                    "                    return Some(Message::new_out_of_line(reply_to, out));\n",
                );
                s.push_str("                }\n");
                s.push_str("                Err(e) => {\n");
                s.push_str("                    let mut out = alloc::vec::Vec::new();\n");
                s.push_str(
                    "                    out.extend_from_slice(&(e as i32).to_le_bytes());\n",
                );
                s.push_str(
                    "                    return Some(Message::new_out_of_line(reply_to, out));\n",
                );
                s.push_str("                }\n");
                s.push_str("            }\n");
                s.push_str("        }\n");
            }
            "unregister" => {
                s.push_str("        UNREGISTER_ID => {\n");
                s.push_str("            if data.len() < off + 4 { return None; }\n");
                s.push_str("            let mut lb=[0u8;4]; lb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let nlen = u32::from_le_bytes(lb) as usize;\n");
                s.push_str("            if data.len() < off + nlen { return None; }\n");
                s.push_str("            let name = core::str::from_utf8(&data[off..off+nlen]).ok()?; off+=nlen;\n");
                s.push_str("            let rc = svc.unregister(name);\n");
                s.push_str("            let mut out = alloc::vec::Vec::new();\n");
                s.push_str("            out.extend_from_slice(&(rc as i32).to_le_bytes());\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            "allocate" => {
                s.push_str("        ALLOCATE_ID => {\n");
                s.push_str("            if data.len() < off + 12 { return None; }\n");
                s.push_str("            let mut sb=[0u8;8]; sb.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let size = u64::from_le_bytes(sb);\n");
                s.push_str("            let mut pb=[0u8;4]; pb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let prot = u32::from_le_bytes(pb);\n");
                s.push_str("            let res = svc.allocate(size, prot);\n");
                s.push_str("            let mut out = alloc::vec::Vec::new();\n");
                s.push_str("            match res { Ok(addr) => { out.extend_from_slice(&(0i32).to_le_bytes()); out.extend_from_slice(&addr.to_le_bytes()); }, Err(e) => { out.extend_from_slice(&(e as i32).to_le_bytes()); } }\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            "deallocate" => {
                s.push_str("        DEALLOCATE_ID => {\n");
                s.push_str("            if data.len() < off + 16 { return None; }\n");
                s.push_str("            let mut ab=[0u8;8]; ab.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let addr = u64::from_le_bytes(ab);\n");
                s.push_str("            let mut sb=[0u8;8]; sb.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let size = u64::from_le_bytes(sb);\n");
                s.push_str("            let rc = svc.deallocate(addr, size);\n");
                s.push_str("            let mut out = alloc::vec::Vec::new(); out.extend_from_slice(&(rc as i32).to_le_bytes());\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            "protect" => {
                s.push_str("        PROTECT_ID => {\n");
                s.push_str("            if data.len() < off + 20 { return None; }\n");
                s.push_str("            let mut ab=[0u8;8]; ab.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let addr = u64::from_le_bytes(ab);\n");
                s.push_str("            let mut sb=[0u8;8]; sb.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let size = u64::from_le_bytes(sb);\n");
                s.push_str("            let mut pb=[0u8;4]; pb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let prot = u32::from_le_bytes(pb);\n");
                s.push_str("            let rc = svc.protect(addr, size, prot);\n");
                s.push_str("            let mut out = alloc::vec::Vec::new(); out.extend_from_slice(&(rc as i32).to_le_bytes());\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            "page_request" => {
                s.push_str("        PAGE_REQUEST_ID => {\n");
                s.push_str("            if data.len() < off + 24 { return None; }\n");
                s.push_str("            let mut ob=[0u8;8]; ob.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let object_id = u64::from_le_bytes(ob);\n");
                s.push_str("            let mut offb=[0u8;8]; offb.copy_from_slice(&data[off..off+8]); off+=8;\n");
                s.push_str("            let offset = u64::from_le_bytes(offb);\n");
                s.push_str("            let mut sb=[0u8;4]; sb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let size = u32::from_le_bytes(sb);\n");
                s.push_str("            let mut pb=[0u8;4]; pb.copy_from_slice(&data[off..off+4]); off+=4;\n");
                s.push_str("            let prot = u32::from_le_bytes(pb);\n");
                s.push_str(
                    "            let res = svc.page_request(object_id, offset, size, prot);\n",
                );
                s.push_str("            let mut out = alloc::vec::Vec::new();\n");
                s.push_str("            match res { Ok(pa) => { out.extend_from_slice(&(0i32).to_le_bytes()); out.extend_from_slice(&pa.to_le_bytes()); }, Err(e) => { out.extend_from_slice(&(e as i32).to_le_bytes()); } }\n");
                s.push_str("            return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("        }\n");
            }
            _ => {}
        }
    }
    s.push_str("        _ => None,\n");
    s.push_str("    }\n}");
    s
}

fn render_trait_sig(r: &MigRoutine) -> String {
    match r.name.as_str() {
        "register" => "fn register(&self, name: &str, port: PortId) -> i32;".into(),
        "lookup" => "fn lookup(&self, name: &str) -> Result<PortId, i32>;".into(),
        "unregister" => "fn unregister(&self, name: &str) -> i32;".into(),
        "allocate" => "fn allocate(&self, size: u64, protection: u32) -> Result<u64, i32>;".into(),
        "deallocate" => "fn deallocate(&self, addr: u64, size: u64) -> i32;".into(),
        "protect" => "fn protect(&self, addr: u64, size: u64, protection: u32) -> i32;".into(),
        "page_request" => "fn page_request(&self, object_id: u64, offset: u64, size: u32, protection: u32) -> Result<u64, i32>;".into(),
        _ => format!("// unsupported routine {}", r.name),
    }
}

fn render_client_method(r: &MigRoutine) -> (String, String) {
    match r.name.as_str() {
        "register" => (
            "pub fn register(&self, name: &str, port: PortId) -> Result<(), i32>".into(),
            format!(
                "        let mut data = alloc::vec::Vec::new();\n        data.extend_from_slice(&({}_ID as u32).to_le_bytes());\n        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n        data.extend_from_slice(name.as_bytes());\n        data.extend_from_slice(&(port.0 as u64).to_le_bytes());\n        let msg = Message::new_out_of_line(self.server.id(), data);\n        self.server.send(msg).map_err(|_| -1)?;\n        Ok(())\n",
                r.name.to_uppercase()
            ),
        ),
        "lookup" => (
            "pub fn lookup(&self, name: &str) -> Result<PortId, i32>".into(),
            format!(
                "        let mut data = alloc::vec::Vec::new();\n        data.extend_from_slice(&({}_ID as u32).to_le_bytes());\n        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n        data.extend_from_slice(name.as_bytes());\n        let msg = Message::new_out_of_line(self.server.id(), data);\n        self.server.send(msg).map_err(|_| -1)?;\n        Ok(PortId(0))\n",
                r.name.to_uppercase()
            ),
        ),
        "unregister" => (
            "pub fn unregister(&self, name: &str) -> Result<(), i32>".into(),
            format!(
                "        let mut data = alloc::vec::Vec::new();\n        data.extend_from_slice(&({}_ID as u32).to_le_bytes());\n        data.extend_from_slice(&(name.len() as u32).to_le_bytes());\n        data.extend_from_slice(name.as_bytes());\n        let msg = Message::new_out_of_line(self.server.id(), data);\n        self.server.send(msg).map_err(|_| -1)?;\n        Ok(())\n",
                r.name.to_uppercase()
            ),
        ),
        _ => (format!("// unsupported routine {}", r.name), String::new()),
    }
}
