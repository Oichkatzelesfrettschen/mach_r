#ifndef _MODERN_FILE_user_
#define _MODERN_FILE_user_

#ifdef __cplusplus
extern "C" {
#endif

/* User header for modern_file subsystem */

#include <mach/kern_return.h>
#include <mach/port.h>
#include <mach/message.h>
#include <mach/std_types.h>

/* Fallback for mach_msg_type_number_t if not in system headers */
#ifndef mach_msg_type_number_t
typedef uint32_t mach_msg_type_number_t;
#endif

/* User-side function prototypes */

/* Routine file_open */
extern kern_return_t file_open(
    mach_port_t server_port,
    const uint8_t *path,
    mach_msg_type_number_t pathCnt,
    uint32_t flags,
    file_handle_t *handle,
    error_code_t *error);

/* Routine file_read */
extern kern_return_t file_read(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);

/* Routine file_write */
extern kern_return_t file_write(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    const uint8_t *data,
    mach_msg_type_number_t dataCnt,
    uint32_t *count,
    error_code_t *error);

/* Routine file_size */
extern kern_return_t file_size(
    mach_port_t server_port,
    file_handle_t handle,
    file_size_t *size,
    error_code_t *error);

/* Routine file_close */
extern kern_return_t file_close(
    mach_port_t server_port,
    file_handle_t handle);

/* Routine file_read_async */
extern kern_return_t file_read_async(
    mach_port_t server_port,
    file_handle_t handle,
    file_offset_t offset,
    uint32_t max_bytes,
    uint64_t *operation_id,
    error_code_t *error);

/* Routine file_poll_async */
extern kern_return_t file_poll_async(
    mach_port_t server_port,
    uint64_t operation_id,
    uint32_t *complete,
    uint8_t *data,
    mach_msg_type_number_t *dataCnt,
    uint32_t *count,
    error_code_t *error);

#ifdef __cplusplus
}
#endif

#endif /* _MODERN_FILE_user_ */
