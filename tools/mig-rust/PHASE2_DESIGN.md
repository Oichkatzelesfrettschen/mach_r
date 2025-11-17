# MIG Phase 2: Design & Architecture

## Vision
Transform the MIG lexer/parser into a fully functional code generator that can:
1. Process real Mach .defs files
2. Generate C code compatible with legacy Mach builds
3. Generate modern Rust bindings
4. Pass all semantic validation checks

## Architecture Overview

```
┌─────────────┐
│ .defs File  │
└──────┬──────┘
       │
       v
┌──────────────┐
│    Lexer     │ (DONE ✓)
└──────┬───────┘
       │
       v
┌──────────────┐
│    Parser    │ (DONE ✓)
└──────┬───────┘
       │
       v
┌──────────────────┐
│ Semantic Analyzer│ ← Phase 2
└──────┬───────────┘
       │
       v
┌──────────────────┐
│  Code Generator  │ ← Phase 2
└──────┬───────────┘
       │
       v
┌─────────────────────────┐
│ C Headers/Stubs/Server  │
│ Rust Bindings (optional)│
└─────────────────────────┘
```

## Phase 2 Components

### 1. CLI Tool (`src/main.rs`)
**Purpose**: Command-line interface for processing .defs files

**Features**:
- Input: One or more .defs files
- Output: Generated C/Rust code
- Options:
  - `--user`: Generate user-side stubs
  - `--server`: Generate server-side stubs
  - `--header`: Generate headers
  - `--rust`: Generate Rust bindings
  - `--output-dir <DIR>`: Output directory
  - `--verbose`: Verbose output
  - `--check`: Syntax check only

**Example Usage**:
```bash
mig-rust mach.defs --user --server --header -o output/
mig-rust --rust vm.defs -o src/generated/
mig-rust --check *.defs
```

### 2. Semantic Analyzer (`src/analysis/`)
**Purpose**: Validate and enrich the AST with semantic information

**Components**:

#### a. Type Resolver (`type_resolver.rs`)
- Resolve all type references
- Build type dependency graph
- Detect circular dependencies
- Populate type size information

#### b. Routine Analyzer (`routine_analyzer.rs`)
- Analyze routine arguments
- Determine message layouts
- Calculate min/max message sizes
- Generate implicit arguments:
  - Count arguments for variable arrays
  - Polymorphic type arguments
  - Dealloc flag arguments
  - ServerCopy flag arguments

#### c. Message Layout (`message_layout.rs`)
- Calculate argument positions in messages
- Handle alignment (4 or 8 byte boundaries)
- Determine inline vs out-of-line data
- Calculate padding

**Data Structures**:
```rust
struct AnalyzedRoutine {
    routine: Routine,
    request_size: MessageSize,
    reply_size: MessageSize,
    implicit_args: Vec<ImplicitArgument>,
    arg_positions: HashMap<String, Position>,
}

struct MessageSize {
    min: usize,
    max: Option<usize>,  // None if unbounded
    alignment: usize,
}

struct Position {
    offset: usize,
    request: bool,  // vs reply
}
```

### 3. C Code Generator (Complete Implementation)

#### a. Type Mapping (`c_generator/types.rs`)
Map MIG types to C types:
- `integer_32` → `uint32_t`
- `boolean_t` → `boolean_t`
- `mach_port_t` → `mach_port_t`
- Arrays → C arrays or pointers
- Structs → C structs

#### b. Header Generation (`c_generator/header.rs`)
```c
#ifndef _SUBSYSTEM_H_
#define _SUBSYSTEM_H_

#include <mach/kern_return.h>
#include <mach/port.h>
#include <mach/message.h>

/* User function prototypes */
kern_return_t routine_name(
    mach_port_t port,
    int32_t arg1,
    int32_t *arg2);

/* Server function prototypes */
kern_return_t server_routine_name(
    mach_port_t port,
    int32_t arg1,
    int32_t *arg2);

/* Server demux */
boolean_t subsystem_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP);

#endif
```

#### c. User Stub Generation (`c_generator/user.rs`)
```c
kern_return_t routine_name(
    mach_port_t port,
    int32_t arg1,
    int32_t *arg2)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t arg1Type;
        int32_t arg1;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t arg2Type;
        int32_t arg2;
    } Reply;

    union {
        Request In;
        Reply Out;
    } Mess;

    Request *InP = &Mess.In;
    Reply *OutP = &Mess.Out;

    // Initialize request
    InP->Head.msgh_bits = MACH_MSGH_BITS(
        MACH_MSG_TYPE_COPY_SEND,
        MACH_MSG_TYPE_MAKE_SEND_ONCE);
    InP->Head.msgh_remote_port = port;
    InP->Head.msgh_local_port = mig_get_reply_port();
    InP->Head.msgh_id = BASE + OFFSET;
    InP->Head.msgh_size = sizeof(Request);

    // Pack arguments
    InP->arg1Type = (mach_msg_type_t) {
        .msgt_name = MACH_MSG_TYPE_INTEGER_32,
        .msgt_size = 32,
        .msgt_number = 1,
        .msgt_inline = TRUE,
        .msgt_longform = FALSE,
        .msgt_deallocate = FALSE,
    };
    InP->arg1 = arg1;

    // Send message
    mach_msg_return_t msg_result = mach_msg(
        &InP->Head,
        MACH_SEND_MSG|MACH_RCV_MSG,
        sizeof(Request),
        sizeof(Reply),
        InP->Head.msgh_local_port,
        MACH_MSG_TIMEOUT_NONE,
        MACH_PORT_NULL);

    if (msg_result != MACH_MSG_SUCCESS)
        return msg_result;

    // Unpack reply
    *arg2 = OutP->arg2;

    return OutP->RetCode;
}
```

#### d. Server Stub Generation (`c_generator/server.rs`)
```c
kern_return_t _Xroutine_name(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t arg1Type;
        int32_t arg1;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t arg2Type;
        int32_t arg2;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    // Type check
    if (In0P->arg1Type.msgt_name != MACH_MSG_TYPE_INTEGER_32)
        return MIG_BAD_ARGUMENTS;

    // Call server function
    int32_t arg2;
    OutP->RetCode = server_routine_name(
        In0P->Head.msgh_request_port,
        In0P->arg1,
        &arg2);

    if (OutP->RetCode != KERN_SUCCESS)
        return KERN_SUCCESS;

    // Pack reply
    OutP->arg2Type = (mach_msg_type_t) {
        .msgt_name = MACH_MSG_TYPE_INTEGER_32,
        .msgt_size = 32,
        .msgt_number = 1,
        .msgt_inline = TRUE,
        .msgt_longform = FALSE,
        .msgt_deallocate = FALSE,
    };
    OutP->arg2 = arg2;

    OutP->Head.msgh_size = sizeof(Reply);

    return KERN_SUCCESS;
}
```

#### e. Demux Generation (`c_generator/demux.rs`)
```c
boolean_t subsystem_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    mig_routine_t routine;

    OutHeadP->msgh_bits = MACH_MSGH_BITS(
        MACH_MSGH_BITS_REPLY(InHeadP->msgh_bits), 0);
    OutHeadP->msgh_remote_port = InHeadP->msgh_reply_port;
    OutHeadP->msgh_local_port = MACH_PORT_NULL;
    OutHeadP->msgh_seqno = 0;
    OutHeadP->msgh_id = InHeadP->msgh_id + 100;

    int msgh_id = InHeadP->msgh_id - BASE;
    if (msgh_id < 0 || msgh_id >= NUM_ROUTINES)
        return FALSE;

    routine = subsystem_routines[msgh_id];
    if (routine == NULL)
        return FALSE;

    (*routine)(InHeadP, OutHeadP);
    return TRUE;
}

mig_routine_t subsystem_routines[NUM_ROUTINES] = {
    _Xroutine1,
    _Xroutine2,
    // ...
};
```

### 4. Test Suite

#### Test Files
1. **simple.defs** - Minimal test case
```c
subsystem simple 1000;

type int32_t = integer_32;

routine add(
        server : mach_port_t;
    in  a : int32_t;
    in  b : int32_t;
    out result : int32_t);
```

2. **array.defs** - Array handling
```c
subsystem array 2000;

type int32_t = integer_32;

routine sum_array(
        server : mach_port_t;
    in  data : array[*:1024] of int32_t;
    out total : int32_t);
```

3. **port.defs** - Port rights
```c
subsystem ports 3000;

routine create_port(
        server : mach_port_t;
    out new_port : mach_port_t);
```

#### Validation
- Parse each .defs file successfully
- Generate all output files
- Compile generated C code with gcc/clang
- No warnings with `-Wall -Wextra`
- Match structure of GNU MIG output (for comparison)

### 5. Implementation Order

**Week 1**: CLI & Basic Infrastructure
1. Create main.rs with clap CLI
2. Wire up lexer → parser → output
3. Basic file I/O and error handling

**Week 2**: Semantic Analysis
4. Implement type resolver
5. Implement routine analyzer
6. Implement message layout calculator
7. Test with simple.defs

**Week 3**: C Code Generation
8. Implement type mapping
9. Implement header generation
10. Implement user stub generation
11. Test with simple.defs

**Week 4**: Advanced C Generation
12. Implement server stub generation
13. Implement demux generation
14. Test with array.defs and port.defs
15. Test with real mach.defs

**Week 5**: Rust Generation (Stretch)
16. Design Rust API
17. Implement Rust type generation
18. Implement Rust client traits
19. Test Rust codegen

### 6. Success Criteria

- ✅ CLI tool processes .defs files end-to-end
- ✅ Semantic analyzer validates all constructs
- ✅ Generated C code compiles without warnings
- ✅ Can process simple.defs, array.defs, port.defs
- ✅ Can process real mach.defs from reference/sources/
- ✅ Generated code structure matches GNU MIG output
- ✅ All tests pass
- ✅ Documentation complete

### 7. Non-Goals (Phase 3)

- Actually running the generated IPC code (needs kernel)
- Full compatibility with all GNU MIG features
- Performance optimization
- Rust async runtime integration

## File Structure

```
tools/mig-rust/
├── src/
│   ├── main.rs              # CLI tool ← NEW
│   ├── lib.rs               # Library exports
│   ├── lexer/               # ✓ DONE
│   ├── parser/              # ✓ DONE
│   ├── analysis/            # ← NEW
│   │   ├── mod.rs
│   │   ├── type_resolver.rs
│   │   ├── routine_analyzer.rs
│   │   └── message_layout.rs
│   ├── types/               # ← EXPAND
│   │   ├── mod.rs
│   │   ├── builtin.rs       # Built-in Mach types
│   │   └── validation.rs
│   ├── codegen/
│   │   ├── mod.rs
│   │   ├── c_generator/     # ← EXPAND
│   │   │   ├── mod.rs
│   │   │   ├── types.rs
│   │   │   ├── header.rs
│   │   │   ├── user.rs
│   │   │   ├── server.rs
│   │   │   └── demux.rs
│   │   └── rust_generator/  # ← EXPAND
│   │       ├── mod.rs
│   │       └── traits.rs
│   └── error.rs             # ← NEW (unified error handling)
├── tests/
│   ├── simple.defs          # ← NEW
│   ├── array.defs           # ← NEW
│   ├── port.defs            # ← NEW
│   └── integration_test.rs  # ← NEW
└── examples/
    └── generate_simple.rs   # ← NEW
```

## Phase 2 Deliverables

1. **Functional CLI tool** - Can be invoked from command line
2. **Semantic analyzer** - Full validation and analysis
3. **C code generator** - Headers, user stubs, server stubs, demux
4. **Test suite** - 3+ test .defs files with validation
5. **Documentation** - Usage guide, API docs
6. **Integration tests** - End-to-end compilation tests
7. **Real .defs processing** - Can handle actual Mach .defs files

## Timeline

**Total**: 4-5 weeks for complete Phase 2
**Minimum Viable**: 2 weeks for basic functionality

Let's start execution!
