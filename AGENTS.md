# Repository Guidelines

## Project Structure
- Core kernel code: `src/` (Rust, `no_std`)
- Build automation: `xtask/` (cargo xtask)
- MIG tool: `tools/mig-rust/`
- Documentation: `docs/book/` (mdBook)
- Linker scripts: `linkers/`
- Archive: `archive/` (historical planning docs, research)

## Build Commands (xtask)
```bash
cargo xtask kernel          # Build AArch64 kernel
cargo xtask qemu            # Run under QEMU
cargo xtask qemu-debug      # Debug under QEMU
cargo xtask mig             # Generate MIG stubs
cargo xtask fmt             # Format code
cargo xtask clippy          # Lint code
cargo xtask test            # Run tests
cargo xtask check           # Full CI check
```

## Coding Style
- Rust 2021 edition; 4-space indentation
- Modules/files: `snake_case`; types: `CamelCase`; constants: `SCREAMING_SNAKE_CASE`
- Prefer `no_std`; minimize `unsafe`; return `Result` over panics
- Format before committing: `cargo fmt && cargo clippy -- -D warnings`

## Testing
- Unit tests: in-module with `#[cfg(test)] mod tests { ... }`
- Tests must be deterministic
- Run: `cargo xtask test`

## Commits
- Use Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `build:`
- Keep diffs focused and minimal

## Key Documentation
- Architecture: `docs/ARCHITECTURE.md`, `ARCHITECTURE.md`
- Implementation status: `TODO.md`
- Clean-room policy: `docs/CLEAN_ROOM.md`, `docs/MIG.md`
- Full docs: `docs/book/src/SUMMARY.md`

## Agent Instructions
- Default scope: `src/` and `docs/`
- Prefer xtask subcommands over shell scripts
- MIG is pure Rust clean-room implementation
- Archive is read-only reference material
