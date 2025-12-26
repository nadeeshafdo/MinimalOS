#include "ipc.h"
#include "../process/process.h"
#include "../process/scheduler.h"
#include "../lib/string.h"
#include "../lib/printk.h"

// Send a message to a process (Kernel Side)
int ipc_send_message(u32 dest_pid, const ipc_message_t* msg) {
    process_t* dest = process_get_by_pid(dest_pid);
    if (!dest) {
        return -1; // Process not found
    }
    
    // Check if mailbox is full
    if (dest->mailbox_count >= MAX_MAILBOX_SIZE) {
        return -2; // Mailbox full
    }
    
    // Copy message to tail
    memcpy(&dest->mailbox[dest->mailbox_tail], msg, sizeof(ipc_message_t));
    
    // Use sender's PID from current process just to be safe/secure, 
    // though the syscall handler should force this.
    process_t* current = process_get_current();
    if (current) {
        dest->mailbox[dest->mailbox_tail].sender_pid = current->pid;
    }
    
    // Update Ring Buffer
    dest->mailbox_tail = (dest->mailbox_tail + 1) % MAX_MAILBOX_SIZE;
    dest->mailbox_count++;
    
    // Wake up process if blocked on receive
    if (dest->state == PROCESS_STATE_BLOCKED && dest->blocked_on_receive) {
        dest->state = PROCESS_STATE_READY;
        dest->blocked_on_receive = 0;
        scheduler_add_process(dest); // Re-add to scheduler ready queue
    }
    
    return 0; // Success
}

// Receive a message (Kernel Side)
// Returns 0 on success, -1 if empty (and non-blocking requested - TODO), 
// Blocks if empty.
int ipc_receive_message(u32* from_pid, ipc_message_t* buffer) {
    process_t* current = process_get_current();
    if (!current) return -1;
    
    while (current->mailbox_count == 0) {
        // Block the process
        printk("[IPC] PID %u blocking for message...\n", current->pid);
        current->state = PROCESS_STATE_BLOCKED;
        current->blocked_on_receive = 1;
        
        // Yield CPU
        yield();
        printk("[IPC] PID %u woke up!\n", current->pid);
    }
    
    // Copy message from head
    ipc_message_t* msg = &current->mailbox[current->mailbox_head];
    memcpy(buffer, msg, sizeof(ipc_message_t));
    
    if (from_pid) {
        *from_pid = msg->sender_pid;
    }
    
    // Update Ring Buffer
    current->mailbox_head = (current->mailbox_head + 1) % MAX_MAILBOX_SIZE;
    current->mailbox_count--;
    
    return 0;
}
