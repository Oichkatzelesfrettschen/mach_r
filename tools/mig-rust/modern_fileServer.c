/* Server stubs for modern_file subsystem */

#include "modern_fileServer.h"
#include <mach/message.h>
#include <mach/mig_errors.h>
#include <mach/ndr.h>

/* MIG error codes */
#ifndef MIG_NO_REPLY
#define MIG_NO_REPLY    (-305)
#define MIG_BAD_ID      (-303)
#define MIG_BAD_ARGUMENTS (-304)
#endif

/* Reply error structure */
typedef struct {
    mach_msg_header_t Head;
    NDR_record_t NDR;
    kern_return_t RetCode;
} mig_reply_error_t;

/* NDR record (if not provided by system) */
#ifndef NDR_RECORD
#define NDR_RECORD
const NDR_record_t NDR_record = { 0, 0, 0, 0, 0, 0, 0, 0 };
#endif

/* User-supplied implementation functions */
extern kern_return_t file_open_impl(
    mach_port_t server_port,
    const uint8_t *path,
    mach_msg_type_number_t pathCnt,
    uint32_t flags,
    file_handle_t *handle,
    error_code_t *error);
extern kern_return_t file_read_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);
extern kern_return_t file_write_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    const uint8_t *data,
    mach_msg_type_number_t dataCnt,
    uint32_t *count,
    error_code_t *error);
extern kern_return_t file_size_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_size_t *size,
    error_code_t *error);
extern kern_return_t file_close_impl(
    mach_port_t server_port,
    file_handle_t handle);
extern kern_return_t file_read_async_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint64_t *operation_id,
    error_code_t *error);
extern kern_return_t file_poll_async_impl(
    mach_port_t server_port,
    uint64_t operation_id,
    uint32_t *complete,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);

kern_return_t _Xfile_open(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t pathType;
        mach_msg_type_number_t pathCnt;
        uint8_t* path;
        mach_msg_type_t flagsType;
        uint32_t flags;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t handleType;
        file_handle_t handle;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->pathType.msgt_name != MACH_MSG_TYPE_BYTE) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->pathType.msgt_size != 8) {
        return MIG_BAD_ARGUMENTS;
    }
    mach_msg_type_number_t pathCnt = In0P->pathType.msgt_number;
    if (pathCnt > 4096) {
        return MIG_BAD_ARGUMENTS; /* Array count exceeds maximum */
    }
    if (!In0P->pathType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->flagsType.msgt_name != MACH_MSG_TYPE_INTEGER_32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->flagsType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->flagsType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->flagsType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    file_handle_t handle;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_open_impl(
        In0P->server_port,
        In0P->path,
        pathCnt,
        In0P->flags,
        &handle,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->handleType.msgt_name = MACH_MSG_TYPE_INTEGER_64;
    OutP->handleType.msgt_size = 64;
    OutP->handleType.msgt_number = 1;
    OutP->handleType.msgt_inline = TRUE;
    OutP->handleType.msgt_longform = FALSE;
    OutP->handleType.msgt_deallocate = FALSE;
    OutP->handleType.msgt_unused = 0;

    OutP->handle = handle;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

kern_return_t _Xfile_read(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t handleType;
        file_handle_t handle;
        mach_msg_type_t offsetType;
        file_offset_t offset;
        mach_msg_type_t max_bytesType;
        uint32_t max_bytes;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t dataType;
        mach_msg_type_number_t dataCnt;
        uint8_t* data;
        mach_msg_type_t countType;
        uint32_t count;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->handleType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->handleType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->offsetType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->offsetType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->max_bytesType.msgt_name != MACH_MSG_TYPE_INTEGER_32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->max_bytesType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->max_bytesType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->max_bytesType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    uint8_t* data;
    mach_msg_type_number_t dataCnt;
    uint32_t count;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_read_impl(
        In0P->server_port,
        In0P->handle,
        In0P->offset,
        In0P->max_bytes,
        &data,
        &dataCnt,
        &count,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->dataType.msgt_name = MACH_MSG_TYPE_BYTE;
    OutP->dataType.msgt_size = 8;
    OutP->dataType.msgt_number = dataCnt; /* Array count */
    OutP->dataType.msgt_inline = TRUE;
    OutP->dataType.msgt_longform = FALSE;
    OutP->dataType.msgt_deallocate = FALSE;
    OutP->dataType.msgt_unused = 0;

    OutP->data = data; /* TODO: handle array packing */

    OutP->countType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->countType.msgt_size = 32;
    OutP->countType.msgt_number = 1;
    OutP->countType.msgt_inline = TRUE;
    OutP->countType.msgt_longform = FALSE;
    OutP->countType.msgt_deallocate = FALSE;
    OutP->countType.msgt_unused = 0;

    OutP->count = count;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

kern_return_t _Xfile_write(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t handleType;
        file_handle_t handle;
        mach_msg_type_t offsetType;
        file_offset_t offset;
        mach_msg_type_t dataType;
        mach_msg_type_number_t dataCnt;
        uint8_t* data;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t countType;
        uint32_t count;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->handleType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->handleType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->offsetType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->offsetType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->dataType.msgt_name != MACH_MSG_TYPE_BYTE) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->dataType.msgt_size != 8) {
        return MIG_BAD_ARGUMENTS;
    }
    mach_msg_type_number_t dataCnt = In0P->dataType.msgt_number;
    if (dataCnt > 1048576) {
        return MIG_BAD_ARGUMENTS; /* Array count exceeds maximum */
    }
    if (!In0P->dataType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    uint32_t count;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_write_impl(
        In0P->server_port,
        In0P->handle,
        In0P->offset,
        In0P->data,
        dataCnt,
        &count,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->countType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->countType.msgt_size = 32;
    OutP->countType.msgt_number = 1;
    OutP->countType.msgt_inline = TRUE;
    OutP->countType.msgt_longform = FALSE;
    OutP->countType.msgt_deallocate = FALSE;
    OutP->countType.msgt_unused = 0;

    OutP->count = count;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

kern_return_t _Xfile_size(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t handleType;
        file_handle_t handle;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t sizeType;
        file_size_t size;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->handleType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->handleType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    file_size_t size;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_size_impl(
        In0P->server_port,
        In0P->handle,
        &size,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->sizeType.msgt_name = MACH_MSG_TYPE_INTEGER_64;
    OutP->sizeType.msgt_size = 64;
    OutP->sizeType.msgt_number = 1;
    OutP->sizeType.msgt_inline = TRUE;
    OutP->sizeType.msgt_longform = FALSE;
    OutP->sizeType.msgt_deallocate = FALSE;
    OutP->sizeType.msgt_unused = 0;

    OutP->size = size;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

kern_return_t _Xfile_close(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t handleType;
        file_handle_t handle;
    } Request;

    Request *In0P = (Request *) InHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->handleType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->handleType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }


    /* Call user implementation */
file_close_impl(
        In0P->server_port,
        In0P->handle
    );

    return MIG_NO_REPLY;  /* Simpleroutine: no reply */

    return KERN_SUCCESS;
}

kern_return_t _Xfile_read_async(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t handleType;
        file_handle_t handle;
        mach_msg_type_t offsetType;
        file_offset_t offset;
        mach_msg_type_t max_bytesType;
        uint32_t max_bytes;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t operation_idType;
        uint64_t operation_id;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->handleType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->handleType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->handleType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->offsetType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->offsetType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->offsetType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->max_bytesType.msgt_name != MACH_MSG_TYPE_INTEGER_32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->max_bytesType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->max_bytesType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->max_bytesType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    uint64_t operation_id;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_read_async_impl(
        In0P->server_port,
        In0P->handle,
        In0P->offset,
        In0P->max_bytes,
        &operation_id,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->operation_idType.msgt_name = MACH_MSG_TYPE_INTEGER_64;
    OutP->operation_idType.msgt_size = 64;
    OutP->operation_idType.msgt_number = 1;
    OutP->operation_idType.msgt_inline = TRUE;
    OutP->operation_idType.msgt_longform = FALSE;
    OutP->operation_idType.msgt_deallocate = FALSE;
    OutP->operation_idType.msgt_unused = 0;

    OutP->operation_id = operation_id;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

kern_return_t _Xfile_poll_async(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t server_portType;
        mach_port_t server_port;
        mach_msg_type_t operation_idType;
        uint64_t operation_id;
    } Request;

    typedef struct {
        mach_msg_header_t Head;
        mach_msg_type_t RetCodeType;
        kern_return_t RetCode;
        mach_msg_type_t completeType;
        uint32_t complete;
        mach_msg_type_t dataType;
        mach_msg_type_number_t dataCnt;
        uint8_t* data;
        mach_msg_type_t countType;
        uint32_t count;
        mach_msg_type_t errorType;
        error_code_t error;
    } Reply;

    Request *In0P = (Request *) InHeadP;
    Reply *OutP = (Reply *) OutHeadP;

    /* Validate request */
    if (In0P->Head.msgh_size != sizeof(Request)) {
        return MIG_BAD_ARGUMENTS;
    }

    /* Validate and extract parameters */
    if (In0P->server_portType.msgt_name != MACH_MSG_TYPE_COPY_SEND) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_size != 32) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->server_portType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->server_portType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    if (In0P->operation_idType.msgt_name != MACH_MSG_TYPE_INTEGER_64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->operation_idType.msgt_size != 64) {
        return MIG_BAD_ARGUMENTS;
    }
    if (In0P->operation_idType.msgt_number != 1) {
        return MIG_BAD_ARGUMENTS;
    }
    if (!In0P->operation_idType.msgt_inline) {
        return MIG_BAD_ARGUMENTS; /* Out-of-line not yet supported */
    }

    uint32_t complete;
    uint8_t* data;
    mach_msg_type_number_t dataCnt;
    uint32_t count;
    error_code_t error;

    /* Call user implementation */
    OutP->RetCode = file_poll_async_impl(
        In0P->server_port,
        In0P->operation_id,
        &complete,
        &data,
        &dataCnt,
        &count,
        &error
    );

    if (OutP->RetCode != KERN_SUCCESS) {
        return MIG_NO_REPLY;
    }

    /* Pack reply */
    OutP->Head.msgh_size = sizeof(Reply);

    OutP->completeType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->completeType.msgt_size = 32;
    OutP->completeType.msgt_number = 1;
    OutP->completeType.msgt_inline = TRUE;
    OutP->completeType.msgt_longform = FALSE;
    OutP->completeType.msgt_deallocate = FALSE;
    OutP->completeType.msgt_unused = 0;

    OutP->complete = complete;

    OutP->dataType.msgt_name = MACH_MSG_TYPE_BYTE;
    OutP->dataType.msgt_size = 8;
    OutP->dataType.msgt_number = dataCnt; /* Array count */
    OutP->dataType.msgt_inline = TRUE;
    OutP->dataType.msgt_longform = FALSE;
    OutP->dataType.msgt_deallocate = FALSE;
    OutP->dataType.msgt_unused = 0;

    OutP->data = data; /* TODO: handle array packing */

    OutP->countType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->countType.msgt_size = 32;
    OutP->countType.msgt_number = 1;
    OutP->countType.msgt_inline = TRUE;
    OutP->countType.msgt_longform = FALSE;
    OutP->countType.msgt_deallocate = FALSE;
    OutP->countType.msgt_unused = 0;

    OutP->count = count;

    OutP->errorType.msgt_name = MACH_MSG_TYPE_INTEGER_32;
    OutP->errorType.msgt_size = 32;
    OutP->errorType.msgt_number = 1;
    OutP->errorType.msgt_inline = TRUE;
    OutP->errorType.msgt_longform = FALSE;
    OutP->errorType.msgt_deallocate = FALSE;
    OutP->errorType.msgt_unused = 0;

    OutP->error = error;


    return KERN_SUCCESS;
}

/* Demux function for modern_file subsystem */
#ifdef __cplusplus
extern "C" {
#endif

boolean_t modern_file_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP)
{
    mach_msg_id_t msgid;
    kern_return_t check_result;

    /* Initialize reply header */
    OutHeadP->msgh_bits = MACH_MSGH_BITS(
        MACH_MSGH_BITS_REMOTE(InHeadP->msgh_bits),
        0);
    OutHeadP->msgh_remote_port = InHeadP->msgh_reply_port;
    OutHeadP->msgh_size = sizeof(mig_reply_error_t);
    OutHeadP->msgh_local_port = MACH_PORT_NULL;
    OutHeadP->msgh_id = InHeadP->msgh_id + 100;

    msgid = InHeadP->msgh_id;

    /* Dispatch to appropriate handler */
    if (msgid >= 5000 && msgid < 5000 + 7) {
        switch (msgid - 5000) {
            case 0:  /* file_open */
                check_result = _Xfile_open(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 1:  /* file_read */
                check_result = _Xfile_read(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 2:  /* file_write */
                check_result = _Xfile_write(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 3:  /* file_size */
                check_result = _Xfile_size(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 4:  /* file_close */
                check_result = _Xfile_close(InHeadP, OutHeadP);
                if (check_result == MIG_NO_REPLY) {
                    return FALSE;  /* No reply for simpleroutine */
                }
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 5:  /* file_read_async */
                check_result = _Xfile_read_async(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            case 6:  /* file_poll_async */
                check_result = _Xfile_poll_async(InHeadP, OutHeadP);
                if (check_result == KERN_SUCCESS) {
                    return TRUE;
                }
                break;

            default:
                break;
        }
    }

    /* Unknown message ID - send error reply */
    ((mig_reply_error_t *)OutHeadP)->NDR = NDR_record;
    ((mig_reply_error_t *)OutHeadP)->RetCode = MIG_BAD_ID;

    return FALSE;
}

#ifdef __cplusplus
}
#endif
