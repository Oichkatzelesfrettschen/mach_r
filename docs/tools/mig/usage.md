# MIG Usage Guide

*Practical guide to using MIG for interface definition and code generation*

## Installation

MIG-Rust is included in the Mach_R repository:

```bash
cd tools/mig-rust

# Build MIG tool
cargo build --release

# Run tests
cargo test

# Install (optional)
cargo install --path .
```

## Quick Start

### 1. Create a `.defs` File

Create `hello.defs`:

```c
subsystem hello 1000;

#include <mach/std_types.defs>

routine greet(
    server_port : mach_port_t;
    in name : string_t;
    out greeting : string_t
);
```

### 2. Generate Code

```bash
# Using cargo run
cargo run --bin mig-rust -- hello.defs --output src/generated/

# Or if installed
mig-rust hello.defs --output src/generated/
```

### 3. Use Generated Code

**Client:**
```rust
use generated::hello::*;

fn main() -> Result<()> {
    let server_port = connect_to_server()?;

    let name = "Alice";
    let greeting = greet(server_port, name)?;

    println!("{}", greeting);  // "Hello, Alice!"
    Ok(())
}
```

**Server:**
```rust
use generated::hello::*;

// Implement the routine
fn greet_impl(
    _server_port: PortId,
    name: &str,
) -> Result<String, Error> {
    Ok(format!("Hello, {}!", name))
}

// Server main loop
fn main() -> Result<()> {
    let port = create_server_port()?;

    loop {
        let msg = port.receive()?;
        hello_server_demux(&msg, port, greet_impl)?;
    }
}
```

## Interface Definition Language

### Subsystem Declaration

```c
subsystem NAME ID;
```

- `NAME`: Subsystem name (used for generated code)
- `ID`: Unique numeric identifier (1000-999999)

Example:
```c
subsystem file_system 2000;
```

### Type Definitions

#### Built-in Types

```c
int                    // 32-bit signed integer
unsigned int           // 32-bit unsigned integer
long                   // 64-bit signed integer
unsigned long          // 64-bit unsigned integer
short                  // 16-bit signed integer
char                   // 8-bit character
boolean_t              // Boolean (32-bit)
mach_port_t           // Port identifier
```

#### String Type

```c
type string_t = array[256] of char;  // Fixed-size string
```

#### Arrays

```c
// Fixed-size array
type buffer_t = array[1024] of char;

// Variable-size array
type data_t = array[*:4096] of char;
```

#### Structures

```c
type point_t = struct {
    x : int;
    y : int;
};

type rect_t = struct {
    origin : point_t;
    width : int;
    height : int;
};
```

#### Type Aliases

```c
type file_descriptor_t = int;
type size_t = unsigned long;
```

### Routine Definitions

#### Basic Routine

```c
routine operation_name(
    server_port : mach_port_t;
    in param1 : int;
    out result : int
);
```

#### Parameter Directions

```c
in      // Input only (client → server)
out     // Output only (server → client)
inout   // Input and output (bidirectional)
```

Example:
```c
routine modify(
    port : mach_port_t;
    inout value : int  // Read and written
);
```

#### SimpleRoutine (No Reply)

```c
simpleroutine notify(
    notify_port : mach_port_t;
    event_type : int;
    data : data_t
);  // Server doesn't send reply
```

Use for:
- Notifications
- One-way messages
- Fire-and-forget operations

## Real-World Examples

### Example 1: File Server

```c
subsystem file_server 3000;

#include <mach/std_types.defs>

type file_data_t = array[*:65536] of char;
type file_name_t = array[256] of char;

// Open a file
routine file_open(
    server_port : mach_port_t;
    in path : file_name_t;
    in mode : int;
    out file_port : mach_port_t
);

// Read from file
routine file_read(
    file_port : mach_port_t;
    in offset : long;
    in count : int;
    out data : file_data_t
);

// Write to file
routine file_write(
    file_port : mach_port_t;
    in offset : long;
    in data : file_data_t;
    out bytes_written : int
);

// Close file
routine file_close(
    file_port : mach_port_t
);
```

Client usage:
```rust
// Open file
let file_port = file_open(server, "/etc/passwd", O_RDONLY)?;

// Read contents
let data = file_read(file_port, 0, 4096)?;

// Close
file_close(file_port)?;
```

### Example 2: Memory Server

```c
subsystem memory_server 4000;

#include <mach/std_types.defs>

type vm_address_t = unsigned long;
type vm_size_t = unsigned long;
type vm_prot_t = int;

// Allocate memory
routine vm_allocate(
    task_port : mach_port_t;
    inout address : vm_address_t;  // IN: hint, OUT: actual
    in size : vm_size_t;
    in anywhere : boolean_t  // TRUE = kernel chooses address
);

// Deallocate memory
routine vm_deallocate(
    task_port : mach_port_t;
    in address : vm_address_t;
    in size : vm_size_t
);

// Change protection
routine vm_protect(
    task_port : mach_port_t;
    in address : vm_address_t;
    in size : vm_size_t;
    in set_maximum : boolean_t;
    in new_protection : vm_prot_t
);
```

### Example 3: Task Server

```c
subsystem task_server 5000;

#include <mach/std_types.defs>

type task_info_t = struct {
    pid : int;
    parent_pid : int;
    state : int;
    priority : int;
};

// Create new task
routine task_create(
    parent_port : mach_port_t;
    in inherit_memory : boolean_t;
    out child_port : mach_port_t
);

// Terminate task
routine task_terminate(
    task_port : mach_port_t
);

// Get task info
routine task_info(
    task_port : mach_port_t;
    out info : task_info_t
);

// Suspend task
routine task_suspend(
    task_port : mach_port_t
);

// Resume task
routine task_resume(
    task_port : mach_port_t
);
```

## Advanced Features

### Port Arrays

```c
type port_array_t = array[*:32] of mach_port_t;

routine get_ports(
    server_port : mach_port_t;
    out ports : port_array_t
);
```

### Out-of-Line Data

For large data transfers:

```c
type large_data_t = array[*:1048576] of char;  // Up to 1 MB

routine transfer_file(
    server_port : mach_port_t;
    in data : large_data_t  // Sent out-of-line automatically
);
```

MIG automatically uses out-of-line memory for arrays larger than inline limit (typically 256 bytes).

### Port Right Transfer

```c
routine transfer_port(
    server_port : mach_port_t;
    in send_right : mach_port_t;  // Transfers send right
    out receive_right : mach_port_t  // Transfers receive right
);
```

### Multiple Includes

```c
subsystem my_service 6000;

#include <mach/std_types.defs>
#include <mach/mach_types.defs>
#include "custom_types.defs"

// ... routines ...
```

## Build Integration

### With Cargo

Add to `build.rs`:

```rust
use std::process::Command;

fn main() {
    // Run MIG on .defs files
    Command::new("mig-rust")
        .args(&[
            "interfaces/file.defs",
            "--output", "src/generated/",
        ])
        .status()
        .expect("MIG failed");

    // Rerun if .defs files change
    println!("cargo:rerun-if-changed=interfaces/file.defs");
}
```

### With Makefiles

```makefile
# Generate code from .defs
generated/%.rs: interfaces/%.defs
	mig-rust $< --output generated/

# Build depends on generated code
build: generated/file.rs generated/memory.rs
	cargo build
```

## Debugging Generated Code

### Inspect Generated Files

```bash
# Generate code
mig-rust interface.defs --output generated/

# View client stubs
cat generated/interfaceUser.rs

# View server stubs
cat generated/interfaceServer.rs
```

### Enable Debug Output

```bash
# MIG with debug logging
RUST_LOG=debug mig-rust interface.defs --output generated/
```

### Test Generated Code

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_client() {
        // Create mock port
        let port = create_mock_port();

        // Call generated stub
        let result = my_routine(port, 42);

        assert!(result.is_ok());
    }
}
```

## Common Patterns

### Error Handling

```c
// Return error codes
routine operation(
    port : mach_port_t;
    in value : int;
    out result : int;
    out error : int  // 0 = success, non-zero = error code
);
```

Client:
```rust
let (result, error) = operation(port, value)?;
if error != 0 {
    return Err(Error::from_code(error));
}
```

### Async Operations

```c
// Start operation (returns immediately)
routine start_operation(
    server_port : mach_port_t;
    in request_id : int;
    in data : data_t
);

// Check status
routine check_status(
    server_port : mach_port_t;
    in request_id : int;
    out completed : boolean_t;
    out result : data_t
);
```

### Callbacks

```c
// Register callback port
routine register_callback(
    server_port : mach_port_t;
    in callback_port : mach_port_t
);

// Server calls back
simpleroutine callback(
    callback_port : mach_port_t;
    event : int;
    data : data_t
);
```

## Troubleshooting

### "Type not found"

```
Error: Unknown type 'my_type_t'
```

**Solution:** Ensure type is defined before use:
```c
type my_type_t = int;  // Define first

routine use_type(
    port : mach_port_t;
    in value : my_type_t  // Then use
);
```

### "Circular type dependency"

```
Error: Circular dependency in type definitions
```

**Solution:** Break the cycle using forward declarations or pointers.

### "Port name conflict"

```
Error: Port name 'server_port' conflicts with system port
```

**Solution:** Use unique port names:
```c
routine operation(
    my_server_port : mach_port_t;  // Not 'server_port'
    // ...
);
```

## Best Practices

### 1. Use Meaningful Names

```c
// Bad
routine op1(port : mach_port_t; in x : int; out y : int);

// Good
routine calculate_sum(
    math_server : mach_port_t;
    in operand1 : int;
    in operand2 : int;
    out sum : int
);
```

### 2. Document Interfaces

```c
/*
 * Calculate the sum of two integers.
 *
 * Arguments:
 *   math_server - Port to math server
 *   operand1 - First operand
 *   operand2 - Second operand
 *
 * Returns:
 *   sum - Result of operand1 + operand2
 */
routine calculate_sum(
    math_server : mach_port_t;
    in operand1 : int;
    in operand2 : int;
    out sum : int
);
```

### 3. Version Subsystems

```c
subsystem my_service_v2 2001;  // v2.0 (ID changed)

// Keep v1 for compatibility
// subsystem my_service_v1 2000;
```

### 4. Keep Interfaces Small

Break large interfaces into multiple .defs files:
```
interfaces/
├── file_basic.defs      # Basic file operations
├── file_advanced.defs   # Advanced file operations
└── file_admin.defs      # Admin operations
```

### 5. Use Type Safety

```c
// Instead of generic int
type file_mode_t = int;
type permissions_t = int;

routine open_file(
    server : mach_port_t;
    in path : string_t;
    in mode : file_mode_t;      // Type documents purpose
    in perms : permissions_t
);
```

## Summary

MIG workflow:
1. **Define** interface in .defs file
2. **Generate** code with mig-rust
3. **Implement** server logic
4. **Call** from client using generated stubs

Benefits:
- Type safety
- Less boilerplate
- Self-documenting
- Easy to maintain

---

**See Also:**
- [MIG Overview](README.md) - Introduction to MIG
- [IPC System](../../architecture/ipc-system.md) - Underlying mechanisms
- [tools/mig-rust/](../../../tools/mig-rust/) - MIG source code
- [Examples](../../../tools/mig-rust/examples/) - More examples
