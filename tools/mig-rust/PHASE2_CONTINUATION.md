# Phase 2 Continuation: Semantic Analysis and Complete Code Generation

## Intelligence Gathered: Real-World .defs Complexity

### Analyzed Real Mach .defs Files
From `reference/extracted/gnu-osfmach/mach/`:

- **std_types.defs** (169 lines) - Fundamental type system
- **exc.defs** (154 lines) - Exception handling (moderate complexity)
- **mach.defs** (954 lines) - Core Mach subsystem (high complexity)
- **mach_port.defs** (421 lines) - Port management
- **memory_object.defs** (393 lines) - VM pager interface

### Critical Features Discovered

#### 1. Type System Extensions
```c
// Message type constants
type char = MACH_MSG_TYPE_CHAR;
type mach_port_t = MACH_MSG_TYPE_COPY_SEND;

// ctype annotations (C type mapping)
type pointer_t = ^array[] of MACH_MSG_TYPE_BYTE
    ctype: vm_offset_t;

type mach_port_name_t = MACH_MSG_TYPE_PORT_NAME
    ctype: mach_port_t;

// Polymorphic types
type mach_port_poly_t = polymorphic
    ctype: mach_port_t;

// Indefinite arrays
type mach_port_array_t = array[] of mach_port_t;
```

#### 2. Preprocessor Integration
```c
subsystem
#if KERNEL_USER
    KernelUser
#endif
    mach 2000;

#ifdef KERNEL_USER
userprefix r_;
#endif

routine exception_raise(
#if KERNEL_USER
    exception_port : mach_port_move_send_t;
#else
    exception_port : mach_port_t;
#endif
    exception : exception_type_t;
    code : exception_data_t
);
```

#### 3. Parameter Modifiers
```c
routine exception_raise_state(
    exception_port : mach_port_t;
    exception : exception_type_t;
    code : exception_data_t, const;      // const modifier
  inout flavor : int;                     // inout as prefix
    old_state : thread_state_t, const;
  out new_state : thread_state_t
);
```

#### 4. Subsystem Modifiers
```c
subsystem KernelUser mach 2000;
subsystem KernelServer mach 2000;

userprefix r_;      // Prefix for user-side functions
serverprefix _X;    // Prefix for server-side functions (default)
```

---

## Phase 2.B: Semantic Analyzer Architecture

### Purpose
Transform parsed AST into semantically-validated, ready-to-codegen IR with:
- Type resolution and validation
- Message layout calculation
- Routine numbering
- Cross-file import resolution

### Components

#### 1. Type Resolver (`src/semantic/type_resolver.rs`)
```rust
pub struct TypeResolver {
    types: HashMap<String, ResolvedType>,
    primitives: HashMap<String, MachMsgType>,
}

pub struct ResolvedType {
    name: String,
    mach_type: MachMsgType,
    c_type: Option<String>,  // ctype annotation
    is_polymorphic: bool,
    size: TypeSize,
}

pub enum MachMsgType {
    TypeBool,
    TypeInteger16,
    TypeInteger32,
    TypeInteger64,
    TypeByte,
    TypeCopyMoveDisposition(PortDisposition),
    TypePolymorphic,
}

pub enum TypeSize {
    Fixed(usize),           // Fixed size in bytes
    Variable { max: usize },// Variable up to max
    Indefinite,             // Size determined at runtime
}
```

**Responsibilities:**
- Build type table from type declarations
- Resolve type names to concrete types
- Handle ctype mappings
- Validate type compatibility
- Calculate type sizes for message layout

#### 2. Message Layout Calculator (`src/semantic/message_layout.rs`)
```rust
pub struct MessageLayout {
    header_size: usize,
    body_size: BodySize,
    descriptors: Vec<DescriptorLayout>,
    inline_data: Vec<InlineData>,
    out_of_line_data: Vec<OOLData>,
}

pub enum BodySize {
    Fixed(usize),
    Variable { min: usize, max: usize },
}

pub struct DescriptorLayout {
    offset: usize,
    descriptor_type: DescriptorType,
    size: usize,
}

pub enum DescriptorType {
    Port,
    OOLMemory,
    OOLPorts,
}
```

**Responsibilities:**
- Calculate message header size
- Determine inline vs out-of-line data placement
- Align data according to Mach IPC rules
- Generate port descriptor layouts
- Compute total message size bounds

#### 3. Routine Analyzer (`src/semantic/routine_analyzer.rs`)
```rust
pub struct RoutineAnalyzer {
    subsystem_base: u32,
    routine_number: u32,
}

pub struct AnalyzedRoutine {
    name: String,
    number: u32,  // subsystem_base + ordinal
    kind: RoutineKind,
    request_layout: MessageLayout,
    reply_layout: Option<MessageLayout>,  // None for simpleroutine
    user_prototype: FunctionSignature,
    server_prototype: FunctionSignature,
}

pub struct FunctionSignature {
    return_type: String,
    parameters: Vec<Parameter>,
}

pub struct Parameter {
    name: String,
    c_type: String,
    direction: Direction,
    pass_by: PassBy,
}

pub enum PassBy {
    Value,
    Pointer,
    ConstPointer,
}
```

**Responsibilities:**
- Assign routine numbers (base + sequential)
- Analyze request/reply message structures
- Generate function signatures for C
- Determine parameter passing conventions
- Validate routine argument compatibility

#### 4. Semantic Validator (`src/semantic/validator.rs`)
```rust
pub struct SemanticValidator {
    type_resolver: TypeResolver,
    errors: Vec<SemanticError>,
    warnings: Vec<SemanticWarning>,
}

pub enum SemanticError {
    UndefinedType { name: String, location: Location },
    IncompatibleTypes { expected: String, found: String, location: Location },
    InvalidPortDisposition { routine: String, arg: String },
    MessageTooLarge { routine: String, size: usize, max: usize },
    DuplicateRoutineNumber { routine1: String, routine2: String, number: u32 },
}
```

**Responsibilities:**
- Validate type usage
- Check message size constraints
- Verify routine number uniqueness
- Ensure port rights are correctly specified
- Report errors with locations

---

## Phase 2.C: Complete C Code Generation

### User Stub Generation (`src/codegen/c_user_stubs.rs`)

**Purpose:** Generate client-side IPC wrappers

**Structure:**
```c
kern_return_t routine_name(
    mach_port_t server_port,
    type1 arg1,
    type2 *out_arg2
) {
    // 1. Declare request/reply messages
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t arg1Type;
        type1 arg1;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t out_arg2Type;
        type2 out_arg2;
    } Reply;

    // 2. Declare union for alignment
    union {
        Request In;
        Reply Out;
    } Mess;

    // 3. Fill request message
    Mess.In.Head.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE);
    Mess.In.Head.msgh_size = sizeof(Request);
    Mess.In.Head.msgh_remote_port = server_port;
    Mess.In.Head.msgh_local_port = mig_get_reply_port();
    Mess.In.Head.msgh_id = routine_number;

    Mess.In.arg1Type = mach_msg_type_t{...};
    Mess.In.arg1 = arg1;

    // 4. Send request and receive reply
    mach_msg_return_t msg_result = mach_msg(
        &Mess.In.Head,
        MACH_SEND_MSG | MACH_RCV_MSG,
        sizeof(Request),
        sizeof(Reply),
        Mess.In.Head.msgh_local_port,
        MACH_MSG_TIMEOUT_NONE,
        MACH_PORT_NULL
    );

    if (msg_result != MACH_MSG_SUCCESS) {
        return msg_result;
    }

    // 5. Extract reply data
    *out_arg2 = Mess.Out.out_arg2;

    return Mess.Out.RetCode;
}
```

**Generator Tasks:**
1. Generate message struct definitions
2. Initialize message headers
3. Pack in parameters
4. Generate mach_msg() call
5. Unpack out parameters
6. Handle errors

### Server Stub Generation (`src/codegen/c_server_stubs.rs`)

**Purpose:** Generate server-side message handlers

**Structure:**
```c
// Individual routine handler
kern_return_t _Xroutine_name(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP
) {
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t arg1Type;
        type1 arg1;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t out_arg2Type;
        type2 out_arg2;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    // Validate request
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    // Extract parameters
    type1 arg1 = In0P->arg1;
    type2 out_arg2;

    // Call user-supplied server function
    OutP->RetCode = server_routine_name(
        In0P->Head.msgh_request_port,
        arg1,
        &out_arg2
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    // Pack reply
    OutP->Head.msgh_size = sizeof(Reply);
    OutP->out_arg2Type = mach_msg_type_t{...};
    OutP->out_arg2 = out_arg2;

    return KERN_SUCCESS;
}

// Demux function
boolean_t subsystem_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP
) {
    mach_msg_id_t msgid = InHeadP->msgh_id;

    // Set reply port
    OutHeadP->msgh_local_port = MACH_PORT_NULL;
    OutHeadP->msgh_remote_port = InHeadP->msgh_reply_port;
    OutHeadP->msgh_bits = MACH_MSGH_BITS(
        MACH_MSGH_BITS_REMOTE(InHeadP->msgh_bits), 0
    );

    // Dispatch to handler
    switch (msgid - subsystem_base) {
        case 0: return _Xroutine1(InHeadP, OutHeadP);
        case 1: return _Xroutine2(InHeadP, OutHeadP);
        ...
        default:
            OutHeadP->msgh_bits = 0;
            OutHeadP->msgh_remote_port = MACH_PORT_NULL;
            return FALSE;
    }
}
```

**Generator Tasks:**
1. Generate message unpacking code
2. Validate message format
3. Call user-supplied server function
4. Pack reply message
5. Generate demux dispatcher
6. Handle simpleroutines (no reply)

---

## Implementation Roadmap

### Week 1-2: Semantic Analyzer Foundation
- [ ] Day 1-2: Create type resolver with primitive types
- [ ] Day 3-4: Implement message layout calculator
- [ ] Day 5-6: Build routine analyzer
- [ ] Day 7-8: Add semantic validator
- [ ] Day 9-10: Test with std_types.defs and exc.defs

### Week 3-4: Complete C Code Generator
- [ ] Day 11-13: Implement user stub message packing
- [ ] Day 14-16: Implement user stub mach_msg() calls
- [ ] Day 17-19: Implement server stub message unpacking
- [ ] Day 20-22: Implement server demux generation
- [ ] Day 23-24: Test with simple.defs, verify compilation

### Week 5: Real-World Validation
- [ ] Day 25-26: Test with mach.defs (core subsystem)
- [ ] Day 27: Test with mach_port.defs
- [ ] Day 28: Fix issues, refine generator
- [ ] Day 29: Compile generated code with Mach headers
- [ ] Day 30: Document and commit Phase 2 complete

---

## Success Criteria

### Functional Requirements
- ✅ Parse real Mach .defs files without errors
- [ ] Resolve all type declarations correctly
- [ ] Calculate message layouts accurately
- [ ] Generate compilable C user stubs
- [ ] Generate compilable C server stubs
- [ ] Generated code links with Mach libraries

### Quality Requirements
- [ ] All real .defs files from gnu-osfmach parse successfully
- [ ] Generated code matches original MIG output structure
- [ ] No memory leaks in generated code
- [ ] Proper error handling in stubs
- [ ] Generated code passes basic integration tests

### Performance Requirements
- [ ] Parse mach.defs (954 lines) in < 100ms
- [ ] Generate code for full subsystem in < 500ms
- [ ] Memory usage < 100MB for largest .defs files

---

## Technical Debt to Address

### Current Limitations
1. No preprocessor support (#if, #ifdef) - Critical for real files
2. No ctype annotation parsing
3. No polymorphic type support
4. Missing `const` parameter modifier
5. Limited array type support (need indefinite arrays)

### Planned Improvements
1. Integrate C preprocessor (via cpp or custom)
2. Extend AST for ctype annotations
3. Add polymorphic type handling
4. Support all parameter modifiers
5. Complete array type implementation

---

## Next Immediate Steps

1. **Copy representative .defs to tests/** for regression testing
2. **Implement type resolver** with builtin types
3. **Implement message layout calculator** with simple fixed-size messages
4. **Generate basic user stub** for simple.defs
5. **Verify compilation** with mock Mach headers
6. **Iterate and refine** based on real-world complexity

---

*"Logic is the beginning of wisdom, not the end."* - Spock
*"Do or do not. There is no try."* - Yoda
*"The needs of the many outweigh the needs of the few."* - Vulcan Proverb

Phase 2.B commences with clarity of purpose and precision of execution.
