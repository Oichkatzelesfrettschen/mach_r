# Cross-Platform .defs Design

## Vision

Create modern, portable Mach IPC definitions that work across multiple platforms while maintaining type safety and zero-copy performance.

## Design Principles

### 1. Platform Abstraction
- Abstract away platform-specific types
- Provide uniform IPC semantics
- Enable conditional compilation for platform-specific features

### 2. Type Safety
- Use explicit, sized types (i32, i64) instead of platform-dependent types (int, long)
- Separate logical types from wire format
- Compile-time port right validation

### 3. Modern Features
- Async-first API design
- Zero-copy where possible
- Structured error handling
- Capability-based security

## Type System

### Core Types

```
// Portable integer types (always use explicit sizes)
type int32 = i32;
type int64 = i64;
type uint32 = u32;
type uint64 = u64;

// Port types (abstracted across platforms)
type port_t = capability;           // Generic port
type port_send_t = send_capability; // Send right
type port_recv_t = recv_capability; // Receive right

// Arrays
type byte_array_t = array[*] of u8;
type int_array_t = array[*] of i32;

// Strings (with explicit encoding)
type utf8_string_t = array[*] of u8;  // UTF-8 encoded
```

### Platform-Specific Mappings

**macOS/Darwin**:
```
port_t          → mach_port_t
port_send_t     → mach_port_t (MACH_MSG_TYPE_COPY_SEND)
port_recv_t     → mach_port_t (MACH_MSG_TYPE_MAKE_SEND)
```

**Linux (with mig-rust runtime)**:
```
port_t          → ChannelHandle
port_send_t     → SendHandle
port_recv_t     → RecvHandle
```

**BSD**:
```
port_t          → CapabilityHandle
port_send_t     → SendCap
port_recv_t     → RecvCap
```

## Example: Modern File Server

```mig
// Modern cross-platform file server interface
// Uses portable types and modern patterns

subsystem file_server 5000;

// Use portable types only
type file_handle_t = uint64;
type file_size_t = uint64;
type file_offset_t = uint64;
type error_code_t = int32;

// UTF-8 strings
type path_string_t = array[*:4096] of uint8;
type data_buffer_t = array[*] of uint8;

// File metadata
type file_info_t = struct {
    size: file_size_t;
    created: uint64;    // Unix timestamp
    modified: uint64;
    mode: uint32;
};

// Open a file and return a capability
routine open_file(
        server_port : port_send_t;
    in  path        : path_string_t;
    in  flags       : uint32;
    out file_handle : file_handle_t;
    out error       : error_code_t
);

// Read from file (zero-copy where possible)
routine read_file(
        server_port : port_send_t;
    in  handle      : file_handle_t;
    in  offset      : file_offset_t;
    in  count       : uint32;
    out data        : data_buffer_t;
    out bytes_read  : uint32;
    out error       : error_code_t
);

// Write to file
routine write_file(
        server_port : port_send_t;
    in  handle      : file_handle_t;
    in  offset      : file_offset_t;
    in  data        : data_buffer_t;
    out bytes_written : uint32;
    out error       : error_code_t
);

// Get file information
routine get_file_info(
        server_port : port_send_t;
    in  handle      : file_handle_t;
    out info        : file_info_t;
    out error       : error_code_t
);

// Close file (simpleroutine - no response needed)
simpleroutine close_file(
        server_port : port_send_t;
    in  handle      : file_handle_t
);
```

## Example: Modern RPC Service

```mig
// Modern cross-platform RPC service
// Demonstrates async patterns and structured errors

subsystem rpc_service 6000;

type request_id_t = uint64;
type service_name_t = array[*:256] of uint8;
type rpc_data_t = array[*:65536] of uint8;

// Error codes
type rpc_error_t = struct {
    code: int32;
    message: array[*:1024] of uint8;
};

// Submit async RPC request (returns immediately)
routine submit_rpc(
        server_port : port_send_t;
    in  service     : service_name_t;
    in  request     : rpc_data_t;
    out request_id  : request_id_t
);

// Poll for RPC result (non-blocking)
routine poll_rpc(
        server_port : port_send_t;
    in  request_id  : request_id_t;
    out complete    : uint32;          // 0 = pending, 1 = done
    out response    : rpc_data_t;
    out error       : rpc_error_t
);

// Cancel pending RPC
simpleroutine cancel_rpc(
        server_port : port_send_t;
    in  request_id  : request_id_t
);
```

## Platform-Specific Features

### Feature Detection

Use preprocessor directives for platform-specific features:

```mig
subsystem platform_aware 7000;

#if MACOS
    type native_port_t = mach_port_t;
#elif LINUX
    type native_port_t = uint32;  // File descriptor
#elif BSD
    type native_port_t = uint32;  // Capability handle
#else
    type native_port_t = uint64;  // Generic handle
#endif

routine get_native_handle(
        server_port : port_send_t;
    out handle      : native_port_t
);
```

### Optional Features

```mig
subsystem optional_features 8000;

// Out-of-line memory (macOS only, emulated elsewhere)
#if SUPPORTS_OOL_MEMORY
type ool_memory_t = array[*] of uint8, outofline;
#else
type ool_memory_t = array[*:1048576] of uint8;  // 1MB inline max
#endif

routine transfer_data(
        server_port : port_send_t;
    in  data        : ool_memory_t
);
```

## Best Practices

### 1. Always Use Explicit Types
```mig
// BAD - platform-dependent
routine bad_example(
    in value : int;      // Size varies!
    in size  : long;     // 32-bit or 64-bit?
);

// GOOD - explicit and portable
routine good_example(
    in value : int32;    // Always 32-bit
    in size  : uint64;   // Always 64-bit
);
```

### 2. Design for Async
```mig
// Prefer non-blocking patterns
routine async_operation(
        server_port : port_send_t;
    out operation_id : uint64      // Return immediately
);

routine check_result(
        server_port : port_send_t;
    in  operation_id : uint64;
    out complete     : uint32;
    out result       : int32
);
```

### 3. Structured Errors
```mig
// Define error structure
type detailed_error_t = struct {
    code: int32;
    category: uint32;
    message: array[*:512] of uint8;
    details: array[*:2048] of uint8;
};

routine operation(
        server_port : port_send_t;
    out error       : detailed_error_t
);
```

### 4. Version Your Interfaces
```mig
// Include version in subsystem ID
subsystem file_service_v2 5001;  // Increment for breaking changes

// Or use explicit version field
routine get_version(
        server_port : port_send_t;
    out major       : uint32;
    out minor       : uint32;
    out patch       : uint32
);
```

## Rust Code Generation Enhancements

### Generated Async API

```rust
// Generated from modern .defs
pub async fn submit_rpc(
    port: &AsyncPort,
    service: &str,
    request: &[u8],
) -> Result<u64, RpcError> {
    let request_msg = SubmitRpcRequest::new(
        service.as_bytes(),
        request,
    )?;

    let reply = port.send_recv(request_msg).await?;
    Ok(reply.request_id)
}
```

### Type-Safe Port Rights

```rust
// Capability-based API
pub struct SendPort(PortName);
pub struct RecvPort(PortName);

impl SendPort {
    pub fn send<T: Message>(&self, msg: T) -> Result<(), IpcError> {
        // Can only send, not receive
    }
}

impl RecvPort {
    pub fn recv<T: Message>(&self) -> Result<T, IpcError> {
        // Can only receive, not send
    }
}
```

### Zero-Copy Arrays

```rust
// Use Bytes crate for zero-copy
pub struct ReadFileReply {
    pub header: MachMsgHeader,
    pub data: Bytes,  // Zero-copy buffer
    pub bytes_read: u32,
}
```

## Migration Path

### 1. Legacy Compatibility
- Support reading old .defs files
- Generate both old and new APIs
- Provide compatibility shims

### 2. Gradual Adoption
- Start with new interfaces for modern code
- Wrap legacy interfaces with new types
- Deprecate old patterns over time

### 3. Cross-Platform Runtime
- Implement mach_r runtime for all platforms
- Use native IPC where available (Mach, kdbus, etc.)
- Fall back to userspace IPC (channels, sockets)

## Implementation Status

- [x] Basic type system design
- [x] Platform abstraction concepts
- [ ] Extended type definitions
- [ ] Cross-platform runtime implementation
- [ ] Platform-specific code generation
- [ ] Async API integration
- [ ] Zero-copy optimization
- [ ] Capability-based security model

## Future Work

1. **Extended Type System**
   - Sum types (enums)
   - Option types
   - Result types with structured errors

2. **Security**
   - Capability tokens
   - Type-state for port rights
   - Compile-time access control

3. **Performance**
   - Shared memory optimization
   - RDMA support
   - Batched operations

4. **Tooling**
   - .defs linter
   - Version compatibility checker
   - Performance profiler
   - IPC fuzzer
