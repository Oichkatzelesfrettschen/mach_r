# MIG Server Stub Implementation Design

## Research Synthesis: MIG Server Architecture

Based on extensive research of XNU sources, Mach documentation, and real-world implementations.

### Core Concepts

**Server Stub Responsibilities:**
1. **Message Unpacking** - Extract parameters from IPC message
2. **Validation** - Verify message format and types
3. **Server Function Call** - Invoke user-supplied implementation
4. **Reply Packing** - Construct reply message with results
5. **Error Handling** - Return appropriate MIG error codes

**Demux Function Responsibilities:**
1. **Message Routing** - Map message ID to handler
2. **Reply Setup** - Initialize reply message header
3. **Stub Dispatch** - Call appropriate _X routine
4. **Error Response** - Handle unknown messages

---

## Server Stub Structure (_X Routines)

### Function Signature
```c
kern_return_t _Xroutine_name(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
```

### Implementation Pattern

```c
kern_return_t _Xadd(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    // 1. Type definitions
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t aType;
        int32_t a;
        mach_msg_type_t bType;
        int32_t b;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t resultType;
        int32_t result;
    } Reply;

    // 2. Cast messages
    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    // 3. Validate request
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    // 4. Extract parameters
    int32_t a = In0P->a;
    int32_t b = In0P->b;
    int32_t result;

    // 5. Call user-supplied server function
    OutP->RetCode = add_impl(
        In0P->Head.msgh_request_port,
        a,
        b,
        &result
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    // 6. Pack reply
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->resultType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->resultType.msgt_size = 32;
    OutP->resultType.msgt_number = 1;
    OutP->resultType.msgt_inline = TRUE;
    OutP->resultType.msgt_longform = FALSE;
    OutP->resultType.msgt_deallocate = FALSE;
    OutP->resultType.msgt_unused = 0;

    OutP->result = result;

    return KERN_SUCCESS;
}
```

---

## Demux Function Structure

### Function Signature
```c
mig_external boolean_t subsystem_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
```

### Implementation Pattern

```c
mig_external boolean_t simple_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    mach_msg_id_t msgid;
    kern_return_t check_result;

    // Initialize reply header
    OutHeadP->msgh_bits = MACH_MSGH_BITS(
        MACH_MSGH_BITS_REMOTE(InHeadP->msgh_bits),
        0);
    OutHeadP->msgh_remote_port = InHeadP->msgh_reply_port;
    OutHeadP->msgh_size = sizeof(mig_reply_error_t);
    OutHeadP->msgh_local_port = MACH_PORT_NULL;
    OutHeadP->msgh_id = InHeadP->msgh_id + 100;

    msgid = InHeadP->msgh_id;

    // Dispatch based on message ID
    if (msgid >= 1000 && msgid < 1000 + 10) {
        switch (msgid - 1000) {
            case 0:  // add
                check_result = _Xadd(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 1:  // log_message (simpleroutine)
                check_result = _Xlog_message(InHeadP, OutHeadP);
                if (check_result == MIG_NO_REPLY) {
                    return FALSE;  // No reply for simpleroutine
                }
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            default:
                break;
        }
    }

    // Unknown message - send error reply
    ((mig_reply_error_t *)OutHeadP)->NDR = NDR_record;
    ((mig_reply_error_t *)OutHeadP)->RetCode = MIG_BAD_ID;

    return FALSE;
}
```

---

## Key MIG Constants and Structures

### Error Codes (from mig_errors.h)
```c
#define MIG_NO_REPLY            (-305)  // Don't send reply
#define MIG_BAD_ID              (-303)  // Unknown routine ID
#define MIG_BAD_ARGUMENTS       (-304)  // Invalid arguments
#define MIG_TYPE_ERROR          (-307)  // Type check failed
#define MIG_REPLY_MISMATCH      (-308)  // Reply port mismatch
```

### Reply Error Structure
```c
typedef struct {
    mach_msg_header_t Head;
    NDR_record_t NDR;
    kern_return_t RetCode;
} mig_reply_error_t;
```

### NDR Record (Network Data Representation)
```c
typedef struct {
    unsigned char       mig_vers;
    unsigned char       if_vers;
    unsigned char       reserved1;
    unsigned char       mig_encoding;
    unsigned char       int_rep;
    unsigned char       char_rep;
    unsigned char       float_rep;
    unsigned char       reserved2;
} NDR_record_t;

// Standard NDR record
extern const NDR_record_t NDR_record;
```

---

## Implementation Strategy

### 1. Server Stub Generator
- Generate _X routines with message unpacking
- Validate message size
- Extract parameters from typed message fields
- Call user-supplied implementation function
- Pack reply message with results
- Return appropriate MIG error codes

### 2. Demux Generator
- Initialize reply header with proper bits
- Dispatch based on message ID range
- Handle simpleroutine vs routine differently
- Send error replies for unknown messages
- Support skip statements (gaps in numbering)

### 3. User Implementation Interface
- Generate prototype declarations for user to implement
- Example: `kern_return_t add_impl(mach_port_t, int32_t, int32_t, int32_t*)`
- User provides actual business logic
- Server stubs handle all IPC mechanics

---

## Testing Strategy

### Mock Mach Headers
Create minimal mock headers with:
- `mach_msg_header_t` structure
- `mach_msg_type_t` structure
- `kern_return_t` type
- `mach_port_t` type
- MIG error codes
- NDR record

### Compilation Test
1. Generate server stubs for simple.defs
2. Create mock Mach headers
3. Create stub implementations (return success)
4. Compile with gcc/clang
5. Verify no errors or warnings

### Integration Test
1. Generate both user and server stubs
2. Create simple test harness
3. Verify message flow (user → server → user)
4. Test error conditions
5. Validate simpleroutine behavior

---

## Advanced Features (Future)

### Type Validation
- Check msgt_name, msgt_size in received messages
- Validate msgt_number for arrays
- Handle msgt_longform descriptors

### Out-of-Line Data
- Handle OOL memory descriptors
- Deallocate transferred memory
- Validate descriptor counts

### Complex Types
- Array unpacking with count
- Struct field extraction
- C-string handling

### Port Rights
- Proper port disposition handling
- Move vs copy semantics
- Port deallocation

---

## Code Generation Checklist

Server Stub (_X routine):
- [ ] Function signature with InHeadP/OutHeadP
- [ ] Request/Reply structure typedefs
- [ ] Message pointer casting
- [ ] Message size validation
- [ ] Parameter extraction
- [ ] User function call
- [ ] Return code check
- [ ] Reply message packing
- [ ] Type descriptor initialization
- [ ] MIG_NO_REPLY for errors
- [ ] KERN_SUCCESS return

Demux Function:
- [ ] Function signature
- [ ] Reply header initialization
- [ ] Message ID extraction
- [ ] ID range check
- [ ] Switch/case dispatch
- [ ] Stub routine calls
- [ ] Return value handling
- [ ] MIG_NO_REPLY handling
- [ ] Error reply generation
- [ ] Boolean return (TRUE/FALSE)

---

*"The needs of the many (messages) outweigh the needs of the few (bugs)."* - Vulcan Server Proverb
