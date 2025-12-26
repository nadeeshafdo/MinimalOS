#ifndef IPC_H
#define IPC_H

#include "../include/types.h"

// IPC Constants
#define MAX_MSG_DATA 1024       // Maximum payload size in bytes
#define MAX_MAILBOX_SIZE 32     // Maximum messages per process

// Message types
#define IPC_TYPE_NONE 0
#define IPC_TYPE_PING 1
#define IPC_TYPE_TEXT 2

// IPC Message Structure
typedef struct ipc_message {
    u32 sender_pid;
    u32 receiver_pid; // Optional, useful for validation
    u32 type;
    u32 length;
    u8 data[MAX_MSG_DATA];
} ipc_message_t;

// Kernel IPC Internal API
int ipc_send_message(u32 dest_pid, const ipc_message_t* msg);
int ipc_receive_message(u32* from_pid, ipc_message_t* buffer);

#endif // IPC_H
