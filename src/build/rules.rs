//! Default build rules for common operations
//! Includes rules for compilation, linking, and other build tasks

use super::{BuildSystem, Rule};

/// Add default build rules to the system
pub fn add_default_rules(build_system: &mut BuildSystem) -> Result<(), &'static str> {
    // C compilation rule
    let mut cc_rule = Rule::new("cc", "$cc -o $out -c $in $cflags")?;
    cc_rule.set_description("CC $out")?;
    cc_rule.set_variable("cc", "gcc")?;
    cc_rule.set_variable("cflags", "-Wall -O2")?;
    build_system.add_rule(cc_rule)?;
    
    // C++ compilation rule
    let mut cxx_rule = Rule::new("cxx", "$cxx -o $out -c $in $cxxflags")?;
    cxx_rule.set_description("CXX $out")?;
    cxx_rule.set_variable("cxx", "g++")?;
    cxx_rule.set_variable("cxxflags", "-Wall -O2 -std=c++17")?;
    build_system.add_rule(cxx_rule)?;
    
    // Rust compilation rule
    let mut rustc_rule = Rule::new("rustc", "$rustc --edition 2021 -o $out $in $rustflags")?;
    rustc_rule.set_description("RUSTC $out")?;
    rustc_rule.set_variable("rustc", "rustc")?;
    rustc_rule.set_variable("rustflags", "-O -C panic=abort")?;
    build_system.add_rule(rustc_rule)?;
    
    // Cargo build rule
    let mut cargo_rule = Rule::new("cargo", "cd $workdir && cargo build $cargoflags")?;
    cargo_rule.set_description("CARGO $out")?;
    cargo_rule.set_variable("cargoflags", "--release")?;
    cargo_rule.set_variable("workdir", ".")?;
    build_system.add_rule(cargo_rule)?;
    
    // Linking rule
    let mut link_rule = Rule::new("link", "$ld -o $out $in $ldflags $libs")?;
    link_rule.set_description("LINK $out")?;
    link_rule.set_variable("ld", "gcc")?;
    link_rule.set_variable("ldflags", "")?;
    link_rule.set_variable("libs", "")?;
    build_system.add_rule(link_rule)?;
    
    // Archive rule (static library)
    let mut ar_rule = Rule::new("ar", "$ar rcs $out $in")?;
    ar_rule.set_description("AR $out")?;
    ar_rule.set_variable("ar", "ar")?;
    build_system.add_rule(ar_rule)?;
    
    // Assembly rule
    let mut as_rule = Rule::new("as", "$as -o $out $in $asflags")?;
    as_rule.set_description("AS $out")?;
    as_rule.set_variable("as", "as")?;
    as_rule.set_variable("asflags", "")?;
    build_system.add_rule(as_rule)?;
    
    // Copy rule
    let mut cp_rule = Rule::new("cp", "cp $in $out")?;
    cp_rule.set_description("CP $out")?;
    build_system.add_rule(cp_rule)?;
    
    // Shell command rule
    let mut shell_rule = Rule::new("shell", "$command")?;
    shell_rule.set_description("SHELL $out")?;
    build_system.add_rule(shell_rule)?;
    
    // Make directory rule
    let mut mkdir_rule = Rule::new("mkdir", "mkdir -p $out")?;
    mkdir_rule.set_description("MKDIR $out")?;
    build_system.add_rule(mkdir_rule)?;
    
    // Remove rule
    let mut rm_rule = Rule::new("rm", "rm -f $in")?;
    rm_rule.set_description("RM $in")?;
    build_system.add_rule(rm_rule)?;
    
    // Touch rule (create empty file)
    let mut touch_rule = Rule::new("touch", "touch $out")?;
    touch_rule.set_description("TOUCH $out")?;
    build_system.add_rule(touch_rule)?;
    
    // QEMU rule for testing
    let mut qemu_rule = Rule::new("qemu", "qemu-system-aarch64 -M virt -cpu cortex-a53 -kernel $in $qemuflags")?;
    qemu_rule.set_description("QEMU $in")?;
    qemu_rule.set_variable("qemuflags", "-nographic -serial stdio -monitor none")?;
    build_system.add_rule(qemu_rule)?;
    
    // Disk image creation rule
    let mut mkimg_rule = Rule::new("mkimg", "dd if=/dev/zero of=$out bs=1M count=$size && mkfs.ext4 $out")?;
    mkimg_rule.set_description("MKIMG $out")?;
    mkimg_rule.set_variable("size", "64")?;
    build_system.add_rule(mkimg_rule)?;
    
    Ok(())
}

/// Create a Mach_R specific build rule set
pub fn add_mach_r_rules(build_system: &mut BuildSystem) -> Result<(), &'static str> {
    // Kernel compilation rule
    let mut kernel_rule = Rule::new("kernel", "$rustc --target $target --edition 2021 -o $out $in $rustflags")?;
    kernel_rule.set_description("KERNEL $out")?;
    kernel_rule.set_variable("target", "aarch64-unknown-none")?;
    kernel_rule.set_variable("rustflags", "-C panic=abort -C link-arg=-Tlinker.ld")?;
    build_system.add_rule(kernel_rule)?;
    
    // Bootloader rule
    let mut boot_rule = Rule::new("bootloader", "$as -o $out $in $asflags")?;
    boot_rule.set_description("BOOT $out")?;
    boot_rule.set_variable("asflags", "-march=armv8-a")?;
    build_system.add_rule(boot_rule)?;
    
    // Disk image with filesystem rule
    let mut disk_rule = Rule::new("disk", "hdiutil create -size $size -fs APFS -volname MACH_R_OS $out")?;
    disk_rule.set_description("DISK $out")?;
    disk_rule.set_variable("size", "4g")?;
    build_system.add_rule(disk_rule)?;
    
    // UTM configuration rule
    let mut utm_rule = Rule::new("utm", "cp $template $out && sed -i'' -e 's|DISK_PATH|$disk|g' $out")?;
    utm_rule.set_description("UTM $out")?;
    utm_rule.set_variable("template", "utm_template.json")?;
    build_system.add_rule(utm_rule)?;
    
    Ok(())
}

/// Create minimal build rules for embedded targets
pub fn add_embedded_rules(build_system: &mut BuildSystem) -> Result<(), &'static str> {
    // Cross-compilation rule
    let mut cross_rule = Rule::new("cross", "$cc --target=$target -o $out -c $in $cflags")?;
    cross_rule.set_description("CROSS $out")?;
    cross_rule.set_variable("target", "aarch64-none-elf")?;
    cross_rule.set_variable("cflags", "-nostdlib -nostartfiles -ffreestanding")?;
    build_system.add_rule(cross_rule)?;
    
    // Linker script rule
    let mut lds_rule = Rule::new("lds", "cpp -P -o $out $in $ldscript_flags")?;
    lds_rule.set_description("LDS $out")?;
    lds_rule.set_variable("ldscript_flags", "-DRAM_SIZE=64M")?;
    build_system.add_rule(lds_rule)?;
    
    // Binary extraction rule
    let mut objcopy_rule = Rule::new("objcopy", "$objcopy -O binary $in $out")?;
    objcopy_rule.set_description("OBJCOPY $out")?;
    objcopy_rule.set_variable("objcopy", "aarch64-none-elf-objcopy")?;
    build_system.add_rule(objcopy_rule)?;
    
    Ok(())
}

/// Add test and verification rules
pub fn add_test_rules(build_system: &mut BuildSystem) -> Result<(), &'static str> {
    // Unit test rule
    let mut test_rule = Rule::new("test", "cd $workdir && cargo test $testflags")?;
    test_rule.set_description("TEST $out")?;
    test_rule.set_variable("testflags", "--release")?;
    build_system.add_rule(test_rule)?;
    
    // Integration test rule
    let mut itest_rule = Rule::new("itest", "cd $workdir && cargo test --test $testname $testflags")?;
    itest_rule.set_description("ITEST $testname")?;
    itest_rule.set_variable("testflags", "--release")?;
    build_system.add_rule(itest_rule)?;
    
    // Benchmark rule
    let mut bench_rule = Rule::new("bench", "cd $workdir && cargo bench $benchflags")?;
    bench_rule.set_description("BENCH $out")?;
    bench_rule.set_variable("benchflags", "")?;
    build_system.add_rule(bench_rule)?;
    
    // Documentation rule
    let mut doc_rule = Rule::new("doc", "cd $workdir && cargo doc $docflags")?;
    doc_rule.set_description("DOC $out")?;
    doc_rule.set_variable("docflags", "--no-deps")?;
    build_system.add_rule(doc_rule)?;
    
    // Clippy rule
    let mut clippy_rule = Rule::new("clippy", "cd $workdir && cargo clippy $clippyflags")?;
    clippy_rule.set_description("CLIPPY $out")?;
    clippy_rule.set_variable("clippyflags", "-- -D warnings")?;
    build_system.add_rule(clippy_rule)?;
    
    // Format check rule
    let mut fmt_rule = Rule::new("fmt", "cd $workdir && cargo fmt --check")?;
    fmt_rule.set_description("FMT $out")?;
    build_system.add_rule(fmt_rule)?;
    
    Ok(())
}