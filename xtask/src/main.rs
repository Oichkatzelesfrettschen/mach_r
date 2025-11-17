use std::env;
use std::fs;
use std::path::{PathBuf};
use std::process::Command;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use sha2::{Sha256, Digest};
use std::fs::read_dir;

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[RUN] {:?}", cmd);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("command failed: {:?}", cmd);
    }
    Ok(())
}

fn cargo() -> Command { Command::new("cargo") }
fn rustup() -> Command { Command::new("rustup") }

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
    run(cargo().args(["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]))
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
    task_qemu_with_args(&["--cpus".into(), "4".into(), "--mem".into(), "2G".into(), "--".into(), "-d".into(), "guest_errors".into()])
}

fn task_kernel() -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    // Ensure target
    run(rustup().args(["target", "add", "aarch64-unknown-none"]))?;
    // Build kernel binary
    run(cargo().args([
        "build",
        "--target",
        "aarch64-unknown-none",
        "--bin",
        "mach_r",
    ]))?;

    // Copy ELF to dist
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let elf = root()?.join(format!(
        "target/aarch64-unknown-none/{}/mach_r",
        profile
    ));
    let out_elf = dist.join("mach_r_kernel.elf");
    fs::copy(&elf, &out_elf)?;
    // Create a .bin artifact using objcopy if available, else copy ELF
    let out_bin = dist.join("mach_r_kernel.bin");
    if let Some(objcopy) = find_objcopy() {
        eprintln!("[OBJCOPY] Using {} -> {}", objcopy, out_bin.display());
        let status = Command::new(objcopy)
            .args(["-O", "binary", elf.to_str().unwrap(), out_bin.to_str().unwrap()])
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                eprintln!("[WARN] objcopy failed; copying ELF as bin fallback");
                fs::copy(&elf, &out_bin)?;
            }
        }
    } else {
        eprintln!("[WARN] objcopy not found; copying ELF as bin");
        fs::copy(&elf, &out_bin)?;
    }
    // Write SHA256SUMS
    let sums_path = dist.join("SHA256SUMS");
    let mut sums = String::new();
    sums.push_str(&format!("{}  mach_r_kernel.elf\n", sha256_file(&out_elf)?));
    sums.push_str(&format!("{}  mach_r_kernel.bin\n", sha256_file(&out_bin)?));
    fs::write(&sums_path, sums)?;
    eprintln!("[ARTIFACT] {}", out_elf.display());
    eprintln!("[ARTIFACT] {}", out_bin.display());
    eprintln!("[ARTIFACT] {}", sums_path.display());
    // Update manifest
    let _ = write_manifest();
    Ok(())
}

fn task_qemu_kernel() -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    let kernel = dist.join("mach_r_kernel.elf");
    if !kernel.exists() {
        task_kernel()?;
    }
    // Minimal QEMU boot similar to Makefile's qemu-kernel
    let status = Command::new("qemu-system-aarch64")
        .args([
            "-M", "virt",
            "-cpu", "cortex-a72",
            "-smp", "4",
            "-m", "2G",
            "-kernel", kernel.to_str().unwrap(),
            "-nographic",
            "-serial", "mon:stdio",
        ])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) | Err(_) => anyhow::bail!("qemu-system-aarch64 not available or failed"),
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
        if have(c) { return Some(c); }
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
    let mut f = OpenOptions::new().create(true).append(true).open(&sums_path)?;
    writeln!(f, "{}  {}", sum, name)?;
    eprintln!("[CHECKSUM] {}  {}", sum, name);
    Ok(())
}

fn print_version(cmd: &str) {
    let out = Command::new(cmd).arg("--version").output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").to_string();
            if s.is_empty() { s = String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").to_string(); }
            if s.is_empty() { s = "<no version output>".into(); }
            eprintln!("[VER] {}: {}", cmd, s);
        }
        Err(_) => eprintln!("[VER] {}: not found", cmd),
    }
}

fn tool_version(cmd: &str) -> Option<String> {
    let out = Command::new(cmd).arg("--version").output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").to_string();
    if s.is_empty() {
        s = String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").to_string();
    }
    if s.is_empty() { None } else { Some(s) }
}

fn write_manifest() -> anyhow::Result<()> {
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
    let mut arts: Vec<(String, PathBuf)> = vec![
        ("mach_r_kernel.elf".into(), dist.join("mach_r_kernel.elf")),
        ("mach_r_kernel.bin".into(), dist.join("mach_r_kernel.bin")),
    ];
    let build = root()?.join("build/images");
    arts.push(("mach_r.qcow2".into(), build.join("mach_r.qcow2")));
    arts.push(("mach_r.iso".into(), build.join("mach_r.iso")));
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
}

fn parse_qemu_opts(args: &[String]) -> QemuOpts {
    let mut q = read_qemu_config().unwrap_or(QemuOpts {
        cpu: "cortex-a72".into(),
        cpus: "4".into(),
        mem: "2G".into(),
        display: None,
        gui: false,
        extra: Vec::new(),
    });
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cpu" if i + 1 < args.len() => { q.cpu = args[i+1].clone(); i += 2; }
            "--cpus" if i + 1 < args.len() => { q.cpus = args[i+1].clone(); i += 2; }
            "--mem" if i + 1 < args.len() => { q.mem = args[i+1].clone(); i += 2; }
            "--display" if i + 1 < args.len() => { q.display = Some(args[i+1].clone()); q.gui = true; i += 2; }
            "--gui" => { q.gui = true; i += 1; }
            "--" => { q.extra.extend_from_slice(&args[i+1..]); break; }
            other => { q.extra.push(other.to_string()); i += 1; }
        }
    }
    q
}

fn read_qemu_config() -> Option<QemuOpts> {
    // Simple ~/.mach_r_qemu.toml
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".mach_r_qemu.toml");
    if !path.exists() { return None; }
    let s = std::fs::read_to_string(path).ok()?;
    let v: toml::Value = toml::from_str(&s).ok()?;
    let mut o = QemuOpts::default();
    if let Some(cpu) = v.get("cpu").and_then(|x| x.as_str()) { o.cpu = cpu.to_string(); }
    if let Some(cpus) = v.get("cpus").and_then(|x| x.as_integer()) { o.cpus = cpus.to_string(); }
    if let Some(mem) = v.get("mem").and_then(|x| x.as_str()) { o.mem = mem.to_string(); }
    if let Some(gui) = v.get("gui").and_then(|x| x.as_bool()) { o.gui = gui; }
    if let Some(display) = v.get("display").and_then(|x| x.as_str()) { o.display = Some(display.to_string()); }
    if let Some(arr) = v.get("extra").and_then(|x| x.as_array()) {
        o.extra = arr.iter().filter_map(|e| e.as_str().map(|s| s.to_string())).collect();
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
    let mut missing = Vec::new();
    for c in [
        "rustup",
        "cargo",
        "qemu-system-aarch64",
        "qemu-img",
        "dd",
    ] {
        if !have(c) { missing.push(c); }
    }
    if !missing.is_empty() {
        anyhow::bail!("Missing tools: {}", missing.join(", "));
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
    let raw = img_dir.join("mach_r.img");
    let qcow2 = img_dir.join("mach_r.qcow2");

    // Ensure kernel present to pair with image
    let kernel = dist.join("mach_r_kernel.bin");
    if !kernel.exists() { task_kernel()?; }
    // Ensure sysroot exists
    task_filesystem()?;

    // Create raw image with dd if available
    if !have("dd") { anyhow::bail!("dd not found"); }
    eprintln!("[DISK] Creating raw image {}", raw.display());
    let _ = fs::remove_file(&raw);
    run(Command::new("dd").args(["if=/dev/zero", &format!("of={}", raw.display()), "bs=1M", "count=256"]))?;

    // Optionally populate sysroot into image (Linux only)
    if args.iter().any(|a| a == "--with-sysroot") {
        #[cfg(target_os = "linux")]
        {
            if have("mkfs.ext4") && have("sudo") {
                let mnt = build.join("mnt");
                let _ = fs::create_dir_all(&mnt);
                run(Command::new("mkfs.ext4").args(["-F", raw.to_str().unwrap()]))?;
                run(Command::new("sudo").args(["mount", "-o", "loop", raw.to_str().unwrap(), mnt.to_str().unwrap()]))?;
                // Copy sysroot contents
                let sysroot = build.join("sysroot");
                if sysroot.exists() {
                    run(Command::new("sudo").args(["cp", "-a", format!("{}{}", sysroot.to_string_lossy(), "/." ).as_str(), mnt.to_str().unwrap()]))?;
                }
                run(Command::new("sudo").args(["umount", mnt.to_str().unwrap()]))?;
            } else {
                eprintln!("[WARN] mkfs.ext4 or sudo not available; skipping sysroot population");
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            eprintln!("[WARN] --with-sysroot supported on Linux only; skipping");
        }
    }

    // Convert to qcow2 if qemu-img exists
    if have("qemu-img") {
        eprintln!("[DISK] Converting to qcow2 {}", qcow2.display());
        let _ = fs::remove_file(&qcow2);
        run(Command::new("qemu-img").args(["convert", "-f", "raw", "-O", "qcow2", "-c", raw.to_str().unwrap(), qcow2.to_str().unwrap()]))?;
        // Append checksum for qcow2
        append_checksum(&qcow2, "mach_r.qcow2")?;
    } else {
        eprintln!("[WARN] qemu-img not found; qcow2 will not be created");
    }
    let _ = write_manifest();
    Ok(())
}

fn task_iso_image() -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    // Ensure kernel present to include on ISO
    let kernel_bin = dist.join("mach_r_kernel.bin");
    if !kernel_bin.exists() { task_kernel()?; }
    let iso_root = build.join("iso");
    let iso_boot = iso_root.join("boot");
    fs::create_dir_all(&iso_boot)?;
    // Place kernel under boot/
    fs::copy(&kernel_bin, iso_boot.join("kernel.bin"))?;
    // Add README with instructions
    let readme = iso_root.join("README.txt");
    fs::write(
        &readme,
        b"Mach_R ISO\n\nThis ISO contains the kernel (boot/kernel.bin).\nUse QEMU or your bootloader tooling to load the kernel.\n\nExample (direct kernel boot):\n  qemu-system-aarch64 -M virt -cpu cortex-a72 -m 2G -kernel boot/kernel.bin -nographic -serial mon:stdio\n",
    )?;
    let iso_path = build.join("images/mach_r.iso");
    fs::create_dir_all(build.join("images"))?;
    // macOS hdiutil, otherwise try genisoimage
    #[cfg(target_os = "macos")]
    {
        run(Command::new("hdiutil").args([
            "makehybrid", "-o",
            iso_path.to_str().unwrap(),
            iso_root.to_str().unwrap(),
        ]))?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        if have("genisoimage") {
            run(Command::new("genisoimage").args([
                "-R", "-J",
                "-o", iso_path.to_str().unwrap(),
                iso_root.to_str().unwrap(),
            ]))?;
        } else {
            eprintln!("[WARN] genisoimage not found; skipping ISO creation");
        }
    }
    if iso_path.exists() { append_checksum(&iso_path, "mach_r.iso")?; }
    eprintln!("[ARTIFACT] {}", iso_path.display());
    let _ = write_manifest();
    Ok(())
}

fn task_qemu_with_args(args: &[String]) -> anyhow::Result<()> {
    let (build, dist) = ensure_dirs()?;
    let qcow2 = build.join("images/mach_r.qcow2");
    let kernel = dist.join("mach_r_kernel.bin");
    if !qcow2.exists() { task_disk_image()?; }
    if !kernel.exists() { task_kernel()?; }
    if !have("qemu-system-aarch64") { anyhow::bail!("qemu-system-aarch64 not found"); }
    let opts = parse_qemu_opts(args);
    let mut cmd = Command::new("qemu-system-aarch64");
    cmd.args(["-M", "virt"])
        .args(["-cpu", &opts.cpu])
        .args(["-smp", &opts.cpus])
        .args(["-m", &opts.mem])
        .args(["-drive", &format!("if=virtio,format=qcow2,file={}", qcow2.display())])
        .args(["-kernel", kernel.to_str().unwrap()])
        .args(["-device", "virtio-net-pci,netdev=net0"]) 
        .args(["-netdev", "user,id=net0"]);
    if opts.gui {
        if let Some(d) = &opts.display { cmd.args(["-display", d]); }
    } else {
        cmd.arg("-nographic");
    }
    cmd.args(["-serial", "mon:stdio"]);
    if !opts.extra.is_empty() { cmd.args(opts.extra); }
    run(&mut cmd)
}

fn task_qemu_debug() -> anyhow::Result<()> {
    let (_build, dist) = ensure_dirs()?;
    let kernel = dist.join("mach_r_kernel.bin");
    if !kernel.exists() { task_kernel()?; }
    run(Command::new("qemu-system-aarch64").args([
        "-M", "virt",
        "-cpu", "cortex-a72",
        "-smp", "4",
        "-m", "2G",
        "-kernel", kernel.to_str().unwrap(),
        "-nographic", "-serial", "mon:stdio", "-s", "-S",
    ]))
}

fn print_help() {
    eprintln!(
        "xtask commands:\n  fmt | fmt-check | clippy | test | check | env-check\n  docs | book\n  kernel | filesystem | disk-image | iso-image\n  qemu | qemu-fast | qemu-dev | qemu-kernel | qemu-debug\n  mig   # generate MIG stubs (e.g., name_server)\n\nQEMU options (before --): --cpu <name> | --cpus <n> | --mem <size> | --display <mode> | --gui | -- <extra qemu args>\n\nExamples:\n  cargo run -p xtask -- fmt\n  cargo run -p xtask -- kernel\n  cargo run -p xtask -- disk-image\n  cargo run -p xtask -- qemu --mem 1G --cpus 2 -- -d guest_errors\n  cargo run -p xtask -- qemu --gui --display default\n  cargo run -p xtask -- qemu-kernel\n  cargo run -p xtask -- mig"
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
        "filesystem" => task_filesystem(),
        "disk-image" => {
            let rest: Vec<String> = args.collect();
            task_disk_image_with_args(&rest)
        },
        "iso-image" => task_iso_image(),
        "qemu-fast" => task_qemu_fast(),
        "qemu-dev" => task_qemu_dev(),
        "qemu" => {
            let rest: Vec<String> = args.collect();
            task_qemu_with_args(&rest)
        },
        "qemu-kernel" => task_qemu_kernel(),
        "qemu-debug" => task_qemu_debug(),
        "env-check" => task_env_check(),
        "mig" => task_mig(),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn task_mig() -> anyhow::Result<()> {
    // Generate MIG stubs from TOML specs in mig/specs/
    let root_dir = root()?;
    let specs_dir = root_dir.join("mig/specs");
    let gen_dir = root_dir.join("src/mig/generated");
    std::fs::create_dir_all(&gen_dir)?;

    if !specs_dir.exists() {
        eprintln!("[MIG] No specs found at {} — creating example spec", specs_dir.display());
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
        if path.extension().and_then(|s| s.to_str()) != Some("toml") { continue; }
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
    if mod_rs_content.is_empty() { mod_rs_content.push_str("// no specs\n"); }
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
    inputs: Vec<MigField>,
    #[serde(default)]
    outputs: Vec<MigField>,
}

#[derive(serde::Deserialize)]
struct MigField {
    name: String,
    #[serde(rename = "type")]
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
        s.push_str(&format!("pub const {}_ID: u32 = {};\n", r.name.to_uppercase(), r.id));
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
    if spec.name == "name_server" { // only for name_server module
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
        s.push_str("    pub fn deallocate_call(&self, addr: u64, size: u64, reply: &Arc<Port>) -> i32 {\n");
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
                s.push_str("                    out.extend_from_slice(&(pid.0 as u64).to_le_bytes());\n");
                s.push_str("                    return Some(Message::new_out_of_line(reply_to, out));\n");
                s.push_str("                }\n");
                s.push_str("                Err(e) => {\n");
                s.push_str("                    let mut out = alloc::vec::Vec::new();\n");
                s.push_str("                    out.extend_from_slice(&(e as i32).to_le_bytes());\n");
                s.push_str("                    return Some(Message::new_out_of_line(reply_to, out));\n");
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
                s.push_str("            let res = svc.page_request(object_id, offset, size, prot);\n");
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
