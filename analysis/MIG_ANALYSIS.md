# Comprehensive MIG (Mach Interface Generator) Analysis

*Analysis Date: 2025-11-16*
*Sources: GNU MIG, OSF MIG 0.90, GNU OSF MIG*

## Executive Summary

MIG is a ~7,806 line C RPC stub generator for Mach microkernel IPC. It takes interface definition files (.defs) and generates client/server stubs and headers. This analysis provides everything needed to implement MIG in pure Rust while maintaining compatibility with legacy Mach systems.

## 1. Overview and Purpose

MIG generates:
- Client-side stubs (user code)
- Server-side stubs (server dispatch code)
- Header files with type definitions and function declarations

### Key Characteristics
- Message-based RPC over Mach ports
- Type-safe serialization/deserialization
- Support for complex types (arrays, structs, ports)
- Out-of-line data transfer for large objects
- Automatic generation of count, type, and dealloc arguments

## 2. .DEFS File Format

### Basic Structure
```c
subsystem [KernelUser] [KernelServer] <name> <base_number>;

import <file>;          // C header to import
uimport <file>;         // User-side only import
simport <file>;         // Server-side only import

type <name> = <type_spec> [annotations];

routine <name>(<args>);
simpleroutine <name>(<args>);  // One-way, no reply
skip;                          // Reserve a message ID
```

### Subsystem Declaration
- `subsystem name base_number;` - Basic form
- Modifiers: `KernelUser`, `KernelServer` - Changes port type handling
- Base number is added to all routine numbers for unique message IDs

### Type System

#### Basic IPC Types
```c
MACH_MSG_TYPE_UNSTRUCTURED
MACH_MSG_TYPE_BIT
MACH_MSG_TYPE_BOOLEAN      // 32 bits
MACH_MSG_TYPE_INTEGER_8/16/32/64
MACH_MSG_TYPE_CHAR
MACH_MSG_TYPE_BYTE
MACH_MSG_TYPE_REAL
MACH_MSG_TYPE_STRING
MACH_MSG_TYPE_POLYMORPHIC
```

#### Port Types
```c
MACH_MSG_TYPE_MOVE_RECEIVE     // Transfer receive right
MACH_MSG_TYPE_MOVE_SEND        // Transfer send right
MACH_MSG_TYPE_MOVE_SEND_ONCE   // Transfer send-once right
MACH_MSG_TYPE_COPY_SEND        // Copy send right
MACH_MSG_TYPE_MAKE_SEND        // Make send right
MACH_MSG_TYPE_MAKE_SEND_ONCE   // Make send-once right
MACH_MSG_TYPE_PORT_NAME        // Just the port name (int)
MACH_MSG_TYPE_PORT_RECEIVE     // Kernel receive pointer
MACH_MSG_TYPE_PORT_SEND        // Kernel send pointer
MACH_MSG_TYPE_PORT_SEND_ONCE   // Kernel send-once pointer
```

#### Compound Types
```c
array[<size>] of <type>        // Fixed-size array
array[] of <type>              // Variable-size (out-of-line)
array[*] of <type>             // Variable-size
array[*:<max>] of <type>       // Variable with maximum
^array[] of <type>             // Pointer to array
struct[<count>] of <type>      // Struct array
struct { <type> <name>; ... }  // Struct definition
c_string[<size>]               // C string
c_string[*:<max>]              // Variable C string
```

### Type Annotations
```c
type new_name = basic_type
    [ctype: <c_type>]              // C type name
    [cusertype: <user_c_type>]     // User-side C type
    [cservertype: <server_c_type>] // Server-side C type
    [intran: <trans_type> <func>(<server_type>)]    // Server translation
    [intranpayload: <trans_type> <func>]             // Alternative intran
    [outtran: <server_type> <func>(<trans_type>)]   // Reverse translation
    [destructor: <func>(<trans_type>)];              // Cleanup function
```

### Argument Syntax
```c
routine name(
    direction name : type [flags];
    ...
);
```

#### Argument Directions
- `in` - Input parameter (client → server)
- `out` - Output parameter (server → client)
- `inout` - Both input and output
- `requestport` - The request destination port
- `replyport` - The reply port
- `sreplyport` - Server reply port
- `ureplyport` - User reply port
- `waittime` - Timeout value (mach_msg_timeout_t)
- `msgoption` - Message options (mach_msg_option_t)
- `msgseqno` - Message sequence number

#### IPC Flags
- `IsLong` / `IsNotLong` - Force long/short msg type format
- `Dealloc` / `NotDealloc` - Deallocate memory after send
- `Dealloc[]` - Variable deallocation based on array
- `ServerCopy` - Server should copy rather than deallocate
- `CountInOut` - Count parameter is both in and out

### Example
```c
subsystem mach 2000;

#include <mach/std_types.defs>
#include <mach/mach_types.defs>

type task_t = mach_port_t
    ctype: mach_port_t
    intran: task_t convert_port_to_task(mach_port_t)
    outtran: mach_port_t convert_task_to_port(task_t)
    destructor: task_deallocate(task_t);

routine task_create(
        target_task    : task_t;
        ledger_ports   : ledger_port_array_t;
        inherit_memory : boolean_t;
    out child_task     : task_t);

simpleroutine memory_object_data_unavailable(
        memory_control : memory_object_control_t;
        offset         : vm_offset_t;
        size           : vm_size_t);
```

## 3. Architecture

### Processing Phases

1. **Initialization**
   - Parse command-line arguments
   - Initialize type system with built-in types
   - Set up global state

2. **Lexical Analysis**
   - Tokenize input file
   - Track line numbers and filenames
   - Handle preprocessor directives (`#line`)
   - Case-insensitive keyword matching

3. **Parsing**
   - Build Abstract Syntax Tree (AST)
   - Create symbol table for types
   - Validate syntax
   - Build statement list

4. **Semantic Analysis**
   - Check routine validity
   - Compute message sizes
   - Determine argument kinds
   - Calculate padding and alignment
   - Resolve implicit arguments
   - Set argument positions in messages

5. **Code Generation**
   - Generate user header
   - Generate user implementation (client stubs)
   - Generate server header
   - Generate server implementation (server stubs + dispatch)

## 4. Key Data Structures

### IPC Type Structure
```c
typedef struct ipc_type {
    identifier_t itName;           // Type name
    struct ipc_type *itNext;       // Symbol table link

    // Size information
    u_int itTypeSize;              // Size in bytes
    u_int itPadSize;               // Padding needed
    u_int itMinTypeSize;           // Minimum size
    u_int itAlignment;             // Alignment requirement

    // IPC type information
    u_int itInName;                // msgt_name sending
    u_int itOutName;               // msgt_name receiving
    u_int itSize;                  // Element size in bits
    u_int itNumber;                // Number of elements (0 = variable)
    bool itInLine;                 // Inline vs out-of-line
    bool itLongForm;               // Use long msg_type format
    dealloc_t itDeallocate;        // Deallocation policy

    // String representations
    const_string_t itInNameStr;    // String form
    const_string_t itOutNameStr;   // String form

    ipc_flags_t itFlags;           // User flags

    // Type characteristics
    bool itStruct;                 // Pass by value (struct)
    bool itString;                 // Null-terminated string
    bool itVarArray;               // Variable-sized array
    bool itIndefinite;             // May be inline or OOL
    bool itUserlandPort;           // Port right (userland)
    bool itKernelPort;             // Port pointer (kernel)

    struct ipc_type *itElement;    // Array element type

    // Translation functions
    identifier_t itUserType;       // C type for user stub
    identifier_t itServerType;     // C type for server stub
    identifier_t itTransType;      // C type for server function
    identifier_t itInTrans;        // ServerType → TransType
    identifier_t itInTransPayload; // Alternative intran
    identifier_t itOutTrans;       // TransType → ServerType
    identifier_t itDestructor;     // Cleanup function
} ipc_type_t;
```

### Routine Structure
```c
typedef struct routine {
    identifier_t rtName;
    routine_kind_t rtKind;         // rkRoutine or rkSimpleRoutine
    argument_t *rtArgs;            // Linked list of arguments
    u_int rtNumber;                // Message ID offset from base

    identifier_t rtUserName;       // With UserPrefix
    identifier_t rtServerName;     // With ServerPrefix

    bool rtOneWay;                 // True for SimpleRoutine

    // Message characteristics
    bool rtSimpleFixedRequest;
    bool rtSimpleSendRequest;
    bool rtSimpleCheckRequest;
    bool rtSimpleReceiveRequest;

    bool rtSimpleFixedReply;
    bool rtSimpleSendReply;
    bool rtSimpleCheckReply;
    bool rtSimpleReceiveReply;

    u_int rtRequestSize;           // Min request message size
    u_int rtReplySize;             // Min reply message size

    int rtNumRequestVar;           // Variable args in request
    int rtNumReplyVar;             // Variable args in reply

    int rtMaxRequestPos;           // Max argRequestPos
    int rtMaxReplyPos;             // Max argReplyPos

    bool rtNoReplyArgs;            // No reply args beyond RetCode

    // Distinguished arguments (always present)
    argument_t *rtRequestPort;     // Request destination
    argument_t *rtUReplyPort;      // User reply port
    argument_t *rtSReplyPort;      // Server reply port
    argument_t *rtReturn;          // Return value (user side)
    argument_t *rtServerReturn;    // Return value (server side)
    argument_t *rtRetCode;         // Return code (kern_return_t)
    argument_t *rtWaitTime;        // Timeout (optional)
    argument_t *rtMsgOption;       // Message options
    argument_t *rtMsgSeqno;        // Sequence number (optional)
} routine_t;
```

### Argument Structure
```c
typedef struct argument {
    identifier_t argName;
    struct argument *argNext;

    arg_kind_t argKind;            // Base kind + bit flags
    ipc_type_t *argType;

    // Generated names
    const_string_t argVarName;     // Variable name in code
    const_string_t argMsgField;    // Message field name
    const_string_t argTTName;      // msg_type field name
    const_string_t argPadName;     // Padding field name

    ipc_flags_t argFlags;          // User flags
    dealloc_t argDeallocate;       // Overrides type
    bool argLongForm;              // Overrides type
    bool argServerCopy;
    bool argCountInOut;

    struct routine *argRoutine;    // Parent routine

    // Associated implicit arguments
    struct argument *argCount;     // Count argument
    struct argument *argCInOut;    // CountInOut argument
    struct argument *argPoly;      // Polymorphic type arg
    struct argument *argDealloc;   // Dealloc flag arg
    struct argument *argSCopy;     // ServerCopy arg
    struct argument *argParent;    // For implicit args, the parent

    int argMultiplier;             // Count multiplier

    // Position tracking
    int argRequestPos;             // Request message position
    int argReplyPos;               // Reply message position

    // Reference semantics
    bool argByReferenceUser;       // Pass by ref on user side
    bool argByReferenceServer;     // Pass by ref on server side
} argument_t;
```

### Argument Kinds

Base kinds (bits 0-5):
```c
akeNone         = 0   // No special meaning
akeNormal       = 1   // User-defined argument
akeRequestPort  = 2   // Request port
akeWaitTime     = 3   // Wait timeout
akeReplyPort    = 4   // Reply port
akeMsgOption    = 5   // Message options
akeMsgSeqno     = 6   // Sequence number
akeRetCode      = 7   // Return code
akeReturn       = 8   // Return value
akeCount        = 9   // Count for parent arg
akePoly         = 10  // Polymorphic type for parent
akeDealloc      = 11  // Dealloc flag for parent
akeServerCopy   = 12  // Server copy flag
akeCountInOut   = 13  // Count in/out
```

Bit flags (bits 6+):
```c
akbRequest      = 0x40      // Has msg_type in request
akbReply        = 0x80      // Has msg_type in reply
akbUserArg      = 0x100     // User-side argument
akbServerArg    = 0x200     // Server-side argument
akbSend         = 0x400     // Carried in request
akbSendBody     = 0x800     // In request body
akbSendSnd      = 0x1000    // Stuffed into request
akbSendRcv      = 0x2000    // Pulled from request
akbReturn       = 0x4000    // Carried in reply
akbReturnBody   = 0x8000    // In reply body
akbReturnSnd    = 0x10000   // Stuffed into reply
akbReturnRcv    = 0x20000   // Pulled from reply
akbReplyInit    = 0x40000   // Must init msg_type
akbRequestQC    = 0x80000   // Quick check possible
akbReplyQC      = 0x100000  // Quick check possible
akbReplyCopy    = 0x200000  // Copy from request to reply
akbVarNeeded    = 0x400000  // Needs local variable
akbDestroy      = 0x800000  // Call destructor
akbVariable     = 0x1000000 // Variable size inline
akbIndefinite   = 0x2000000 // Variable inline or OOL
akbPointer      = 0x4000000 // Server gets pointer
```

## 5. Code Generation Patterns

### User Stub Template
```c
kern_return_t routine_name(
    mach_port_t port,
    in_type in_arg,
    out_type *out_arg)
{
    // Request message structure
    struct {
        mach_msg_header_t Head;
        mach_msg_type_t in_argType;
        in_type in_arg;
    } Request;

    // Reply message structure
    struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t out_argType;
        out_type out_arg;
    } Reply;

    // Initialize request header
    Request.Head.msgh_bits =
        MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE);
    Request.Head.msgh_remote_port = port;
    Request.Head.msgh_local_port = mig_get_reply_port();
    Request.Head.msgh_id = BASE + NUMBER;
    Request.Head.msgh_size = sizeof(Request);

    // Pack input arguments
    Request.in_argType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    Request.in_argType.msgt_size = 32;
    Request.in_argType.msgt_number = 1;
    Request.in_argType.msgt_inline = TRUE;
    Request.in_argType.msgt_longform = FALSE;
    Request.in_argType.msgt_deallocate = FALSE;
    Request.in_arg = in_arg;

    // Send request, receive reply
    mach_msg_return_t msg_result = mach_msg(
        &Request.Head,
        MACH_SEND_MSG | MACH_RCV_MSG,
        sizeof(Request),
        sizeof(Reply),
        Request.Head.msgh_local_port,
        MACH_MSG_TIMEOUT_NONE,
        MACH_PORT_NULL);

    if (msg_result != MACH_MSG_SUCCESS)
        return msg_result;

    // Type check reply
    if (Reply.out_argType != expected_type)
        return MIG_TYPE_ERROR;

    // Unpack output arguments
    *out_arg = Reply.out_arg;

    return Reply.RetCode;
}
```

### Server Stub Template
```c
kern_return_t _Xroutine_name(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t in_argType;
        in_type in_arg;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t out_argType;
        out_type out_arg;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    // Type checking
    if (In0P->in_argType.msgt_name != MACH_MSG_TYPE_INTEGER_32 ||
        In0P->in_argType.msgt_size != 32 ||
        In0P->in_argType.msgt_number != 1 ||
        !In0P->in_argType.msgt_inline ||
        In0P->in_argType.msgt_longform ||
        In0P->in_argType.msgt_deallocate)
        return MIG_BAD_ARGUMENTS;

    // Call server function
    OutP->RetCode = server_routine_name(
        In0P->Head.msgh_request_port,
        In0P->in_arg,
        &OutP->out_arg);

    if (OutP->RetCode != KERN_SUCCESS)
        return KERN_SUCCESS;  // Still send reply

    // Set up reply message types
    OutP->out_argType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->out_argType.msgt_size = 32;
    OutP->out_argType.msgt_number = 1;
    OutP->out_argType.msgt_inline = TRUE;
    OutP->out_argType.msgt_longform = FALSE;
    OutP->out_argType.msgt_deallocate = FALSE;

    OutP->Head.msgh_size = sizeof(Reply);

    return KERN_SUCCESS;
}
```

### Server Demux Template
```c
mig_routine_t subsystem_routines[] = {
    _Xroutine1,
    _Xroutine2,
    0,  // skipped
    _Xroutine4,
    // ...
};

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
    if (msgh_id < 0 || msgh_id >= MAX_ROUTINES)
        return FALSE;

    routine = subsystem_routines[msgh_id];
    if (routine == 0)
        return FALSE;

    (*routine)(InHeadP, OutHeadP);
    return TRUE;
}
```

## 6. Algorithms

### Message Size Calculation
1. Start with header size: `sizeof(mach_msg_header_t)`
2. For each argument:
   - Add `sizeof(mach_msg_type_t)` or `sizeof(mach_msg_type_long_t)`
   - Add argument data size
   - Add padding to next 4-byte boundary
3. For variable arguments, use minimum size

### Type Checking Code Generation
MIG generates optimized type checking:

**Quick Check (Fast Path):**
```c
if (*(int *)&In0P->argType != 0xDEADBEEF)  // Precalculated value
    return MIG_BAD_ARGUMENTS;
```

**Slow Check (Detailed):**
```c
if (In0P->argType.msgt_name != expected_name ||
    In0P->argType.msgt_size != expected_size ||
    In0P->argType.msgt_number != expected_number ||
    In0P->argType.msgt_inline != TRUE ||
    In0P->argType.msgt_longform != FALSE ||
    In0P->argType.msgt_deallocate != FALSE)
    return MIG_BAD_ARGUMENTS;
```

### Out-of-Line Data
For large or variable data:
1. Data pointer in message
2. Actual data in separate VM region
3. Kernel handles VM transfer automatically
4. Receiver gets pointer to mapped region

## 7. Rust Implementation Architecture

### Recommended Structure
```rust
// src/lexer/mod.rs
pub enum Token {
    Keyword(Keyword),
    Identifier(String),
    Number(u32),
    String(String),
    Symbol(char),
    // ...
}

pub enum Keyword {
    Subsystem,
    Routine,
    SimpleRoutine,
    Type,
    Import,
    In,
    Out,
    InOut,
    // ...
}

// src/parser/ast.rs
pub struct Subsystem {
    pub name: String,
    pub base: u32,
    pub modifiers: Vec<SubsystemMod>,
    pub statements: Vec<Statement>,
}

pub enum Statement {
    TypeDecl(TypeDecl),
    Routine(Routine),
    Import { kind: ImportKind, file: String },
    Skip,
}

pub struct TypeDecl {
    pub name: String,
    pub spec: TypeSpec,
    pub annotations: TypeAnnotations,
}

pub enum TypeSpec {
    Basic(BasicType),
    Array {
        size: ArraySize,
        element: Box<TypeSpec>,
    },
    Pointer(Box<TypeSpec>),
    Struct(Vec<StructField>),
    CString {
        max_size: Option<u32>,
        varying: bool,
    },
}

pub enum ArraySize {
    Fixed(u32),
    Variable,
    VariableWithMax(u32),
}

pub struct TypeAnnotations {
    pub ctype: Option<String>,
    pub cusertype: Option<String>,
    pub cservertype: Option<String>,
    pub intran: Option<(String, String)>,  // (type, func)
    pub outtran: Option<(String, String)>,
    pub destructor: Option<String>,
}

pub struct Routine {
    pub name: String,
    pub kind: RoutineKind,
    pub args: Vec<Argument>,
    pub number: u32,
}

pub enum RoutineKind {
    Routine,
    SimpleRoutine,
}

pub struct Argument {
    pub name: String,
    pub direction: Direction,
    pub arg_type: TypeSpec,
    pub flags: IpcFlags,
}

pub enum Direction {
    In,
    Out,
    InOut,
    RequestPort,
    ReplyPort,
    SReplyPort,
    UReplyPort,
    WaitTime,
    MsgOption,
    MsgSeqno,
}

pub struct IpcFlags {
    pub is_long: Option<bool>,
    pub dealloc: Option<DeallocMode>,
    pub server_copy: bool,
    pub count_in_out: bool,
}

// src/types/mod.rs
pub struct TypeSystem {
    types: HashMap<String, IpcType>,
}

pub struct IpcType {
    pub name: String,
    pub size_bytes: u32,
    pub alignment: u32,
    pub ipc_type_in: IpcTypeName,
    pub ipc_type_out: IpcTypeName,
    pub element_size_bits: u32,
    pub element_count: ElementCount,
    pub inline: bool,
    pub port_type: Option<PortType>,
    // ...
}

// src/codegen/mod.rs
pub trait CodeGenerator {
    fn generate_user_header(&self, subsystem: &Subsystem) -> Result<String>;
    fn generate_user_impl(&self, subsystem: &Subsystem) -> Result<String>;
    fn generate_server_header(&self, subsystem: &Subsystem) -> Result<String>;
    fn generate_server_impl(&self, subsystem: &Subsystem) -> Result<String>;
}

pub struct CCodeGenerator {
    // Configuration
}

pub struct RustCodeGenerator {
    // Configuration for generating Rust stubs
}
```

### Suggested Dependencies
```toml
[dependencies]
nom = "8.0"              # Parser combinators
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"        # Error handling
quote = "1.0"            # Code generation (optional)
proc-macro2 = "1.0"      # Token manipulation (optional)
```

### Module Layout
```
tools/mig-rust/
├── src/
│   ├── main.rs              // CLI entry point
│   ├── lib.rs               // Library interface
│   ├── lexer/
│   │   ├── mod.rs
│   │   └── tokens.rs
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── grammar.rs
│   │   └── ast.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── type_system.rs
│   │   ├── ipc_type.rs
│   │   └── symbol_table.rs
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── semantic.rs
│   │   ├── validation.rs
│   │   └── size_calc.rs
│   ├── codegen/
│   │   ├── mod.rs
│   │   ├── user.rs          // User stub generation
│   │   ├── server.rs        // Server stub generation
│   │   ├── header.rs        // Header generation
│   │   └── rust.rs          // Rust code generation
│   └── error.rs
├── tests/
│   ├── integration_tests.rs
│   └── fixtures/            // .defs test files
└── Cargo.toml
```

## 8. Implementation Strategy

### Phase 1: Lexer (Week 1)
1. Implement token types
2. Use `nom` for tokenization
3. Handle keywords, identifiers, numbers, strings
4. Support preprocessor directives
5. Test with simple .defs files

### Phase 2: Parser (Week 2-3)
1. Define AST types
2. Implement parser using `nom` combinators
3. Build subsystem, type, and routine parsers
4. Handle imports and skip directives
5. Create symbol table for types
6. Test with real Mach .defs files

### Phase 3: Type System (Week 4)
1. Implement IpcType structure
2. Build type system with built-in types
3. Handle type resolution
4. Calculate sizes and alignment
5. Support type annotations

### Phase 4: Semantic Analysis (Week 5)
1. Validate routine definitions
2. Compute message sizes
3. Determine argument kinds
4. Calculate positions in messages
5. Generate implicit arguments

### Phase 5: Code Generation - C (Week 6-8)
1. Implement header generation
2. Implement user stub generation
3. Implement server stub generation
4. Implement dispatch table generation
5. Test by compiling generated code
6. Verify compatibility with legacy Mach

### Phase 6: Testing (Week 9)
1. Test with all Mach .defs files
2. Build original Mach with new MIG
3. Run IPC tests
4. Benchmark performance

### Phase 7: Rust Code Generation (Week 10-12)
1. Design Rust IPC API
2. Implement Rust stub generation
3. Generate type-safe Rust bindings
4. Support async Rust patterns
5. Integrate with Mach_R kernel

## 9. Testing Strategy

### Unit Tests
- Lexer: Test token recognition
- Parser: Test AST construction
- Type system: Test size calculations
- Codegen: Test output format

### Integration Tests
Use real .defs files:
- `mach/mach.defs` - Complex, production
- `mach/mach_host.defs` - Host interface
- `mach/memory_object.defs` - VM pager interface
- `device/device.defs` - Device interface

### Compatibility Tests
1. Generate C stubs with Rust MIG
2. Compile with original Mach
3. Run IPC tests
4. Verify binary compatibility

### Regression Tests
- Use GNU MIG test suite
- Compare generated code
- Ensure identical behavior

## 10. Key Implementation Insights

1. **Two-Pass Processing**: Parse first, analyze second. Don't try to compute sizes during parsing.

2. **Type System is Core**: Get the type system right first. Everything else depends on it.

3. **Implicit Arguments**: Count, poly, dealloc arguments are automatically generated. Track parent relationships carefully.

4. **Alignment is Critical**: Message layout requires precise alignment (4 or 8 byte boundaries).

5. **Direction Matters**: In/Out/InOut dramatically affects code generation. Use strong typing.

6. **Port Types are Special**: Different handling for userland vs kernel port types.

7. **Message Type Encoding**: Master `mach_msg_type_t` structure format.

8. **Quick Check Optimization**: Generate integer compare when possible for type checking.

9. **Error Messages**: Provide helpful errors with line numbers and context.

10. **Incremental Development**: Start simple (basic routines), add complexity (arrays, ports, translation).

## 11. Reference Locations

### Source Code
- **GNU MIG**: `reference/sources/gnu-mig/` (~7,806 lines)
- **OSF MIG 0.90**: `reference/extracted/osfmig-0.90/`
- **GNU OSF MIG**: `reference/extracted/gnu-osfmig/`

### Test Files
- **Production .defs**: `reference/extracted/osfmk/src/mach_kernel/mach/*.defs`
- **GNU MIG tests**: `reference/sources/gnu-mig/tests/good/*.defs`

### Key Files to Study
- **Lexer**: `gnu-mig/lexxer.l` (287 lines)
- **Parser**: `gnu-mig/parser.y` (706 lines)
- **Type System**: `gnu-mig/type.h`, `type.c` (300 lines)
- **Routine**: `gnu-mig/routine.h`, `routine.c` (388 lines)
- **User Codegen**: `gnu-mig/user.c` (1329 lines)
- **Server Codegen**: `gnu-mig/server.c` (1599 lines)
- **Header Codegen**: `gnu-mig/header.c`

## 12. Success Criteria

- [ ] Parse all Mach .defs files without error
- [ ] Generate C code that compiles
- [ ] Generated code is binary-compatible with GNU MIG output
- [ ] Can build original Mach with Rust MIG
- [ ] Passes GNU MIG test suite
- [ ] Performance comparable to GNU MIG
- [ ] Generate type-safe Rust bindings
- [ ] Support all MIG features
- [ ] Comprehensive error messages
- [ ] Full documentation

## Appendix: Common .defs Patterns

### Simple Routine
```c
routine vm_allocate(
        target_task : vm_task_t;
        address     : vm_address_t;
        size        : vm_size_t;
        anywhere    : boolean_t);
```

### Out Parameters
```c
routine vm_read(
        target_task : vm_task_t;
        address     : vm_address_t;
        size        : vm_size_t;
    out data        : pointer_t);
```

### Variable Arrays
```c
routine host_processors(
        host_priv   : host_priv_t;
    out processor_list : processor_array_t,
                         CountInOut, Dealloc);
```

### Port Rights
```c
routine task_get_special_port(
        task        : task_t;
        which_port  : int;
    out special_port : mach_port_t);
```

### Translation Functions
```c
type vm_map_t = mach_port_t
    intran: vm_map_t convert_port_to_map(mach_port_t)
    outtran: mach_port_t convert_map_to_port(vm_map_t)
    destructor: vm_map_deallocate(vm_map_t);
```

This analysis provides a complete foundation for implementing MIG in Rust while maintaining full compatibility with legacy Mach systems.
