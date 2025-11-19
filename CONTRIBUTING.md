# Contributing to Mach_R

Thank you for your interest in contributing to Mach_R! This document provides guidelines and best practices for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Documentation](#documentation)
- [Clean Room Development](#clean-room-development)

## Code of Conduct

We are committed to providing a welcoming and inclusive environment. All contributors are expected to:

- Be respectful and considerate
- Welcome newcomers and help them learn
- Focus on technical merit and constructive feedback
- Respect different viewpoints and experiences

## Getting Started

### Finding Something to Work On

- Check [GitHub Issues](https://github.com/YOUR_USERNAME/Synthesis/issues) for open tasks
- Look for issues labeled `good-first-issue` or `help-wanted`
- Review the [ROADMAP.md](ROADMAP.md) for planned features
- Ask questions in [GitHub Discussions](https://github.com/YOUR_USERNAME/Synthesis/discussions)

### Before You Start

1. **Check for duplicates** - Search existing issues and PRs
2. **Discuss major changes** - Open an issue for significant features or architectural changes
3. **Read the docs** - Familiarize yourself with [ARCHITECTURE.md](ARCHITECTURE.md) and [docs/](docs/)
4. **Understand clean-room** - Review [docs/development/clean-room.md](docs/development/clean-room.md) for our clean-room development policy

## Development Setup

### Prerequisites

- **Rust:** 1.70 or later (`rustup update`)
- **Rust Source:** `rustup component add rust-src`
- **Targets:**
  ```bash
  rustup target add aarch64-unknown-none
  rustup target add x86_64-unknown-none
  ```
- **QEMU:** For testing kernel boot
  ```bash
  # macOS
  brew install qemu

  # Linux
  apt-get install qemu-system-aarch64 qemu-system-x86
  ```
- **Development Tools:**
  ```bash
  rustup component add rustfmt clippy
  ```

### Initial Setup

```bash
# Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/Synthesis.git
cd Synthesis

# Create a feature branch
git checkout -b feature/your-feature-name

# Verify build works
cargo build --lib
cargo test --lib

# Check environment
cargo run -p xtask -- env-check
```

## Project Structure

```
Synthesis/
├── src/                    # Mach_R kernel implementation
│   ├── lib.rs              # Library entry point
│   ├── port.rs             # Port and IPC system
│   ├── message.rs          # Message passing
│   ├── task.rs             # Task management
│   ├── memory.rs           # Memory management
│   ├── scheduler.rs        # Thread scheduler
│   ├── arch/               # Architecture-specific code
│   ├── boot/               # Boot sequence
│   └── mig/                # MIG-generated code
├── tools/
│   └── mig-rust/           # Mach Interface Generator
├── real_os/                # Bootable OS implementation
├── tests/                  # Integration tests
├── docs/                   # Documentation
├── archive/                # Historical reference (DO NOT MODIFY)
└── xtask/                  # Build automation

Key Locations:
- Core kernel: src/
- Architecture code: src/arch/{aarch64,x86_64}/
- Build system: xtask/src/main.rs
- Tests: tests/ and src/**/tests.rs
- Documentation: docs/
```

### What NOT to Modify

- **`archive/c-reference/`** - Historical CMU Mach code (reference only)
- **`archive/osfmk/`** - OSFMK reference code (reference only)
- **Generated code** - Files in `src/mig/generated/` (regenerate with `cargo run -p xtask -- mig`)

## Coding Standards

### Rust Style

Follow standard Rust conventions:

- **Edition:** Rust 2021
- **Indentation:** 4 spaces (no tabs)
- **Line length:** 100 characters (soft limit)
- **Naming:**
  - Modules/files: `snake_case`
  - Types/traits: `CamelCase`
  - Constants: `SCREAMING_SNAKE_CASE`
  - Functions/variables: `snake_case`

### Code Quality

```bash
# Format code (required before commit)
cargo fmt

# Run linter (must pass)
cargo clippy -- -D warnings

# Check without building
cargo check
```

### Best Practices

- **Prefer `no_std`** - Keep kernel code `no_std` compatible
- **Minimize `unsafe`** - Use safe abstractions where possible
- **Error handling** - Return `Result<T, E>` over panicking
- **Documentation** - Add doc comments to public APIs
- **Type safety** - Leverage Rust's type system for safety

### Example Code Style

```rust
/// Represents a Mach port with capability-based security.
///
/// Ports are unidirectional communication endpoints used for IPC.
/// Each port has associated rights that control access.
pub struct Port {
    id: PortId,
    state: Mutex<PortState>,
    messages: MessageQueue,
}

impl Port {
    /// Creates a new port owned by the specified task.
    ///
    /// # Arguments
    ///
    /// * `receiver` - TaskId of the receiving task
    ///
    /// # Returns
    ///
    /// An Arc-wrapped Port ready for use.
    pub fn new(receiver: TaskId) -> Arc<Self> {
        Arc::new(Self {
            id: PortId::generate(),
            state: Mutex::new(PortState::Active),
            messages: MessageQueue::new(),
        })
    }
}
```

## Testing Guidelines

### Unit Tests

Place unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_creation() {
        let task_id = TaskId::new(1);
        let port = Port::new(task_id);
        assert_eq!(port.state(), PortState::Active);
    }
}
```

### Integration Tests

Create integration tests in `tests/`:

```rust
// tests/test_ipc.rs
use mach_r::{Port, Message, TaskId};

#[test]
fn test_message_send_receive() {
    let sender = TaskId::new(1);
    let receiver = TaskId::new(2);
    let port = Port::new(receiver);

    // Test implementation
}
```

### Running Tests

```bash
# All tests
cargo test --lib

# Specific test
cargo test test_port_creation

# With output
cargo test -- --nocapture

# Via xtask
cargo run -p xtask -- test
```

### Test Requirements

- **Deterministic** - No timing dependencies
- **Isolated** - No shared global state
- **Fast** - Should complete quickly
- **Clear** - Good error messages
- **Documented** - Explain what's being tested

## Commit Guidelines

### Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

#### Types

- `feat:` - New feature
- `fix:` - Bug fix
- `refactor:` - Code restructuring
- `docs:` - Documentation changes
- `test:` - Test additions/modifications
- `build:` - Build system changes
- `chore:` - Maintenance tasks

#### Examples

```
feat(ipc): implement port death notifications

Add support for no-senders notifications when all send rights
to a port are deallocated. This enables resource cleanup and
connection monitoring.

Closes #123
```

```
fix(memory): correct page allocation alignment

Page allocator was not enforcing 4K alignment on all architectures.
This fixes memory corruption on x86_64.

Fixes #456
```

```
docs(architecture): update IPC system documentation

Clarify port rights transfer semantics and add sequence diagrams
for common message passing patterns.
```

## Pull Request Process

### Before Submitting

1. **Update from main**
   ```bash
   git checkout main
   git pull upstream main
   git checkout your-branch
   git rebase main
   ```

2. **Run quality checks**
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test --lib
   cargo run -p xtask -- test
   ```

3. **Update documentation**
   - Update relevant .md files
   - Add doc comments to new public APIs
   - Update CHANGELOG.md if applicable

### PR Template

```markdown
## Description
Brief description of changes and motivation.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Refactoring
- [ ] Documentation
- [ ] Tests

## Testing
Describe testing performed:
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing in QEMU

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] Tests pass
- [ ] No new warnings

## Related Issues
Closes #123
Related to #456
```

### Review Process

1. **Automated checks** - CI must pass
2. **Code review** - At least one approval required
3. **Testing** - Reviewer may test locally
4. **Documentation** - Verify docs are updated
5. **Merge** - Squash and merge with clean history

## Documentation

### What to Document

- **Public APIs** - All public functions, types, and modules
- **Architecture** - Design decisions and patterns
- **Tutorials** - How to use features
- **Examples** - Code samples for common tasks

### Documentation Style

```rust
/// Brief one-line description.
///
/// More detailed explanation of what this does, how it works,
/// and when to use it.
///
/// # Arguments
///
/// * `param` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Examples
///
/// ```
/// let port = Port::new(task_id);
/// ```
///
/// # Errors
///
/// When errors can occur and why
///
/// # Safety
///
/// Safety requirements for unsafe functions
pub fn example() { }
```

### Documentation Files

- **`docs/architecture/`** - Architecture and design
- **`docs/development/`** - Development guides
- **`docs/tools/`** - Tool documentation
- **`README.md`** - Project overview
- **`ARCHITECTURE.md`** - System architecture
- **`ROADMAP.md`** - Development roadmap

## Clean Room Development

Mach_R is a **clean-room implementation** - we implement Mach concepts from published papers and documentation, not by copying or translating existing code.

### Guidelines

1. **Use published sources:**
   - Research papers (Rashid et al., Accetta et al.)
   - Textbooks and documentation
   - Interface specifications

2. **Do NOT:**
   - Copy code from CMU Mach, GNU Mach, or XNU
   - Translate C code line-by-line to Rust
   - Look at implementation details from other Mach kernels

3. **Archive is reference only:**
   - Historical code in `archive/` is for understanding concepts
   - Do not copy algorithms or data structures directly
   - Implement fresh in Rust using Mach semantics

4. **When in doubt, ask:**
   - Open an issue to discuss implementation approach
   - Document sources of inspiration
   - Keep implementation independent

See [docs/development/clean-room.md](docs/development/clean-room.md) for details.

## Build System

### Using xtask

Prefer `xtask` for build tasks:

```bash
# Build kernel
cargo run -p xtask -- kernel

# Generate MIG stubs
cargo run -p xtask -- mig

# Run in QEMU
cargo run -p xtask -- qemu

# Debug in QEMU
cargo run -p xtask -- qemu-debug

# Format code
cargo run -p xtask -- fmt

# Run linter
cargo run -p xtask -- clippy

# Run tests
cargo run -p xtask -- test
```

### Adding New xtask Commands

Edit `xtask/src/main.rs` to add build automation:

```rust
// xtask/src/main.rs
fn main() {
    match args.subcommand() {
        Some(("your-command", sub_args)) => {
            // Implementation
        }
        _ => { /* help */ }
    }
}
```

## Areas Needing Contribution

### High Priority

- 🔴 **Scheduler** - Thread scheduling and priority management
- 🔴 **External Pagers** - Async pager framework
- 🔴 **Device Drivers** - Driver abstraction layer
- 🔴 **Testing** - Expand test coverage

### Medium Priority

- 🟡 **POSIX Layer** - Syscall compatibility
- 🟡 **Filesystem** - VFS and file server
- 🟡 **Network Stack** - TCP/IP implementation
- 🟡 **Documentation** - Guides and examples

### Good First Issues

- 🟢 **Documentation** - Fix typos, add examples
- 🟢 **Tests** - Add unit tests for existing code
- 🟢 **Code Quality** - Address clippy warnings
- 🟢 **Examples** - Create usage examples

## Getting Help

- **Documentation:** [docs/INDEX.md](docs/INDEX.md)
- **Issues:** [GitHub Issues](https://github.com/YOUR_USERNAME/Synthesis/issues)
- **Discussions:** [GitHub Discussions](https://github.com/YOUR_USERNAME/Synthesis/discussions)
- **Architecture:** [ARCHITECTURE.md](ARCHITECTURE.md)

## License

By contributing to Mach_R, you agree that your contributions will be licensed under the MIT License. See [LICENSE](LICENSE) for details.

---

Thank you for contributing to Mach_R! Your efforts help build a safer, more modern microkernel for the future.
