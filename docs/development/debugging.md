# GDB Debugging Quickstart

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

- Start QEMU with GDB stub:
  - `make qemu-debug` or `cargo run -p xtask -- qemu-kernel` (then attach)

- Connect with GDB:
```gdb
file target/aarch64-unknown-none/debug/mach_r
set arch aarch64
set pagination off
set confirm off
set disassemble-next-line on
# If using qemu -s -S, connect to :1234 and continue
# target remote :1234
# continue
```

Tips
- Use `layout asm` for instruction view.
- `break *_start` or set breakpoints in Rust with symbol names if available.
- Prefer direct-kernel boot for early-boot debugging.
