#ifndef _MODERN_FILE_server_
#define _MODERN_FILE_server_

#ifdef __cplusplus
extern "C" {
#endif

/* Server header for modern_file subsystem */

#include <mach/kern_return.h>
#include <mach/port.h>
#include <mach/message.h>
#include <mach/std_types.h>
#include <mach/boolean.h>

/* Fallback for mach_msg_type_number_t if not in system headers */
#ifndef mach_msg_type_number_t
typedef uint32_t mach_msg_type_number_t;
#endif

/* Server implementation functions (provided by user) */

/* Routine file_open implementation */
extern kern_return_t file_open_impl(
    mach_port_t server_port,
    const uint8_t *path,
    mach_msg_type_number_t pathCnt,
    uint32_t flags,
    file_handle_t *handle,
    error_code_t *error);

/* Routine file_read implementation */
extern kern_return_t file_read_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);

/* Routine file_write implementation */
extern kern_return_t file_write_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    const uint8_t *data,
    mach_msg_type_number_t dataCnt,
    uint32_t *count,
    error_code_t *error);

/* Routine file_size implementation */
extern kern_return_t file_size_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_size_t *size,
    error_code_t *error);

/* Routine file_close implementation */
extern kern_return_t file_close_impl(
    mach_port_t server_port,
    file_handle_t handle);

/* Routine file_read_async implementation */
extern kern_return_t file_read_async_impl(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint64_t *operation_id,
    error_code_t *error);

/* Routine file_poll_async implementation */
extern kern_return_t file_poll_async_impl(
    mach_port_t server_port,
    uint64_t operation_id,
    uint32_t *complete,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);

/* Demux function */
extern boolean_t modern_file_server(
    mach_msg_header_t *InHeadP,
    mach_msg_header_t *OutHeadP);

#ifdef __cplusplus
}
#endif

#endif /* _MODERN_FILE_server_ */
