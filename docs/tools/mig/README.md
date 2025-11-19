# MIG - Mach Interface Generator

*In the spirit of Lions' Commentary: Understanding interface definition and code generation*

## Introduction - The Problem MIG Solves

In a microkernel, services communicate via IPC. A client wants to call a function in a server, but they're in different address spaces.

Traditional approach:
```rust
// Client manually constructs message
let mut msg = Message::new();
msg.add_int(OPERATION_READ);
msg.add_int(offset);
msg.add_int(size);
port.send(msg)?;

// Server manually parses message
let msg = port.receive()?;
let operation = msg.read_int()?;
match operation {
    OPERATION_READ => {
        let offset = msg.read_int()?;
        let size = msg.read_int()?;
        // Perform read...
    }
    // ...
}
```

Problems:
- Tedious: Manual marshalling/unmarshalling
- Error-prone: Easy to mismatch types
- Not type-safe: Message is just bytes
- Duplicated code: Client and server both handle serialization

## The MIG Solution

MIG (Mach Interface Generator) automates this:

1. **Define interface** in `.defs` file (specification)
2. **Generate code** for client and server stubs
3. **Type safety** enforced at compile time

```
┌──────────────┐
│  file.defs   │  ← Interface definition
│  (human)     │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│     MIG      │  ← Code generator
│   (tool)     │
└──────┬───────┘
       │
       ├────────────────┬────────────────┐
       ▼                ▼                ▼
┌────────────┐   ┌────────────┐   ┌────────────┐
│ file.rs    │   │fileUser.rs │   │fileServer.rs│
│ (types)    │   │ (client)   │   │ (server)   │
└────────────┘   └────────────┘   └────────────┘
```

## MIG-Rust: Pure Rust Implementation

Mach_R includes **mig-rust**, a clean-room implementation of MIG in pure Rust.

### Why Not Use Original MIG?

The original MIG (from CMU/Apple):
- Written in C
- Generates C code
- Complex build process
- Platform-specific

MIG-Rust:
- Written in Rust
- Generates Rust code
- Cargo-based build
- Cross-platform (macOS, Linux, *BSD)

### Features

- ✅ Complete .defs parser
- ✅ Type system with layout-driven resolution
- ✅ Rust client stub generation
- ✅ Rust server stub generation
- ✅ Pure Rust (no C dependencies)
- ✅ Modern error handling
- ✅ Comprehensive testing

## Simple Example

### Step 1: Define Interface

Create `example.defs`:

```c
subsystem example 1000;

#include <mach/std_types.defs>

type data_t = array[*:1024] of char;

routine echo(
    server_port : mach_port_t;
    in message : data_t;
    out reply : data_t
);
```

This defines:
- Subsystem "example" (ID 1000)
- One routine "echo" that:
  - Takes a port and input data
  - Returns output data

### Step 2: Generate Code

```bash
cd tools/mig-rust
cargo run -- example.defs --output generated/
```

This generates:
- `generated/example.rs` - Type definitions
- `generated/exampleUser.rs` - Client stubs
- `generated/exampleServer.rs` - Server stubs

### Step 3: Client Code

```rust
use generated::example::*;

// Client calls the routine like a normal function
fn client_example(port: PortId) -> Result<(), Error> {
    let message = b"Hello, server!";

    // MIG-generated stub handles IPC
    let reply = echo(port, message)?;

    println!("Server replied: {:?}", reply);
    Ok(())
}
```

Behind the scenes:
1. `echo()` marshalls arguments into message
2. Sends message to server_port
3. Waits for reply
4. Unmarshalls reply
5. Returns result

### Step 4: Server Code

```rust
use generated::example::*;

// Server implements the routine
fn echo_impl(
    server_port: PortId,
    message: &[u8],
) -> Result<Vec<u8>, Error> {
    // Echo the message back
    Ok(message.to_vec())
}

// MIG-generated dispatcher
fn server_example(port: PortId) -> Result<(), Error> {
    loop {
        // Receive request
        let msg = port.receive()?;

        // MIG-generated demux dispatches to implementation
        example_server_demux(&msg, port, |msg_id, args| {
            match msg_id {
                MSG_ID_ECHO => {
                    // Unmarshal arguments
                    let (server_port, message) = unmarshal_echo(args)?;

                    // Call implementation
                    let reply = echo_impl(server_port, message)?;

                    // Marshal reply
                    marshal_echo_reply(reply)
                }
                _ => Err(Error::UnknownMessage),
            }
        })?;
    }
}
```

## Benefits

### Type Safety

```rust
// This won't compile:
echo(port, 42);  // Error: expected &[u8], got i32
```

The compiler catches type errors.

### Interface Documentation

The .defs file documents the interface:
```c
/*
 * Read bytes from file at given offset.
 *
 * Arguments:
 *   file_port - Port to file server
 *   offset - Byte offset to read from
 *   size - Number of bytes to read
 *
 * Returns:
 *   data - Bytes read from file
 */
routine read(
    file_port : mach_port_t;
    offset : int;
    size : int;
    out data : data_t
);
```

### Versioning

```c
subsystem file 2000;  // Version 2.0

// v2.0 adds new parameter
routine read(
    file_port : mach_port_t;
    offset : int;
    size : int;
    flags : int;  // New in v2.0
    out data : data_t
);
```

Clients using old interface get compile error, not runtime failure.

## MIG .defs Syntax

### Basic Structure

```c
subsystem NAME ID;

#include <header.defs>

type custom_t = struct { ... };

routine operation_name(
    arguments...
);
```

### Type Definitions

```c
// Built-in types
int                     // 32-bit integer
long                    // 64-bit integer
mach_port_t            // Port identifier

// Arrays
array[SIZE] of TYPE            // Fixed size
array[*:MAX] of TYPE           // Variable size, max MAX

// Structs
struct {
    field1 : int;
    field2 : array[10] of char;
}

// Custom types
type my_type_t = int;
type buffer_t = array[*:4096] of char;
```

### Routine Parameters

```c
routine operation(
    // Input parameter
    in param1 : int;

    // Output parameter
    out param2 : int;

    // Input/output parameter
    inout param3 : int;

    // Port parameter (sends port right)
    port : mach_port_t;
);
```

### Advanced Features

```c
// Server reply port
simpleroutine notify(
    notify_port : mach_port_t;
    event : int
);  // No reply expected

// Skip number (for ABI compatibility)
skip;  // Reserve slot for future routine

// Import other defs
#include <mach/mach_types.defs>
```

## MIG-Rust Architecture

```
Input: .defs file
       │
       ▼
┌──────────────┐
│    Lexer     │ ← Tokenize
└──────┬───────┘
       │ Tokens
       ▼
┌──────────────┐
│    Parser    │ ← Build AST
└──────┬───────┘
       │ AST
       ▼
┌──────────────┐
│ Type System  │ ← Resolve types
└──────┬───────┘
       │ Typed AST
       ▼
┌──────────────┐
│   Codegen    │ ← Generate Rust
└──────┬───────┘
       │
       ├───────────────┬───────────────┐
       ▼               ▼               ▼
    types.rs      client.rs       server.rs
```

### Lexer

Breaks input into tokens:
```c
routine read(file : int);
```

Becomes:
```
ROUTINE, IDENT("read"), LPAREN, IDENT("file"), COLON, TYPE("int"), RPAREN, SEMICOLON
```

### Parser

Builds Abstract Syntax Tree:
```
Routine {
    name: "read",
    params: [
        Parameter {
            name: "file",
            direction: In,
            type: Int,
        }
    ],
}
```

### Type System

Resolves types and layouts:
```rust
struct TypeInfo {
    name: String,
    size: usize,      // Size in bytes
    alignment: usize,
    layout: Layout,
}
```

Handles complex cases:
- Typedef chains: `type A = B; type B = C; type C = int;`
- Struct layouts: Padding, alignment
- Array sizes: Fixed vs. variable

### Code Generator

Produces Rust code:

```rust
// Input
routine read(file : int; out data : buffer_t);

// Output (simplified)
pub fn read(
    port: PortId,
    file: i32,
) -> Result<Vec<u8>, Error> {
    let mut msg = Message::new();
    msg.write_i32(file);

    let reply = port.send_receive(msg)?;
    let data = reply.read_bytes()?;

    Ok(data)
}
```

## Current Status

As of 2025-01-19:

- ✅ Lexer: Complete
- ✅ Parser: Complete
- ✅ Type system: Complete
- ✅ Rust client stubs: Complete
- ✅ Rust server stubs: Complete
- ✅ C code generation: For validation
- ✅ Cross-platform: macOS, Linux, *BSD
- 🚧 Advanced features: In progress (port arrays, etc.)

## Future Enhancements

- [ ] Asynchronous stubs (async/await)
- [ ] Zero-copy optimization for large messages
- [ ] Interface versioning support
- [ ] Documentation generation from .defs
- [ ] IDE integration (syntax highlighting, completion)

## Summary

MIG bridges the gap between:
- **What we want**: Call remote functions like local functions
- **What we have**: Low-level message passing

By generating boilerplate, MIG provides:
- Type safety
- Less code to write
- Fewer bugs
- Better documentation

MIG-Rust brings this to the Rust ecosystem with:
- Pure Rust implementation
- Modern error handling
- Cross-platform support
- Integration with Cargo

---

**See Also:**
- [MIG Usage Guide](usage.md) - How to use MIG
- [IPC System](../../architecture/ipc-system.md) - Underlying IPC mechanism
- [tools/mig-rust/](../../../tools/mig-rust/) - MIG source code
