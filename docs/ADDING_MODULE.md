# How to Add a New Module

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

1) Create the module file
- Add `src/<name>.rs` and implement a minimal API.
- Keep public surface small; prefer `Result<T, E>`.

2) Wire it into the crate
- Add `pub mod <name>;` in `src/lib.rs` (near related modules).
- If it needs early init, add `<name>::init()` and call it in `lib::init()`.

3) Add unit tests
- In `src/<name>.rs` add:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basics() {
        // assertions here
    }
}
```
- Run: `cargo run -p xtask -- test`.

4) Lint and format
- `cargo run -p xtask -- fmt`
- `cargo run -p xtask -- clippy`

5) Update docs if needed
- Add a short section to `docs/ARCHITECTURE.md` if this is a core system.
- Mention new commands or flows in `README.md`.

6) Submit PR
- Use Conventional Commits (e.g., `feat: add <name> module`).
- Include a brief rationale and test notes.
