#ifndef PROCESS_H
#define PROCESS_H

#include "../include/types.h"
#include "../mm/vmm.h"

#define MAX_PROCESSES 256
#define KERNEL_STACK_SIZE 16384   // 16KB
#define USER_STACK_SIZE   1048576  // 1MB

// Process states
typedef enum {
    PROCESS_STATE_CREATED,   // Just created, not yet ready
    PROCESS_STATE_READY,     // Ready to run
    PROCESS_STATE_RUNNING,   // Currently executing
    PROCESS_STATE_BLOCKED,   // Waiting for I/O or event
    PROCESS_STATE_ZOMBIE,    // Terminated, waiting for parent
    PROCESS_STATE_DEAD       // Fully terminated
} process_state_t;

// CPU context saved during context switch
typedef struct {
    u64 r15, r14, r13, r12, r11, r10, r9, r8;
    u64 rbp, rdi, rsi, rdx, rcx, rbx, rax;
    u64 rip;      // Instruction pointer
    u64 cs;       // Code segment
    u64 rflags;   // CPU flags
    u64 rsp;      // Stack pointer
    u64 ss;       // Stack segment
} __attribute__((packed)) cpu_context_t;

// Process Control Block
typedef struct process {
    u32 pid;                          // Process ID
    char name[32];                    // Process name
    process_state_t state;            // Current state
    
    cpu_context_t* context;           // Saved CPU context
    page_directory_t* page_directory; // Virtual memory space
    
    uintptr kernel_stack;             // Kernel mode stack (16KB)
    uintptr user_stack;               // User mode stack top (for ELF processes)
    
    struct process* parent;           // Parent process
    
    int exit_code;                    // Exit status
    u32 priority;                     // Priority (for future use)
    u64 time_slice;                   // Remaining time slice
    struct process* next;             // Next in ready queue
} process_t;

/**
 * Initialize process management
 */
void process_init(void);

/**
 * Create a new process
 */
process_t* process_create(const char* name);

/**
 * Setup a process to run as kernel thread
 */
void process_setup_kernel_thread(process_t* proc, void (*entry_point)(void));

/**
 * Destroy a process
 */
void process_destroy(process_t* proc);

/**
 * Get current running process
 */
process_t* process_get_current(void);

/**
 * Set current running process
 */
void process_set_current(process_t* proc);

/**
 * Set process state
 */
void process_set_state(process_t* proc, process_state_t state);

/**
 * Get process state
 */
process_state_t process_get_state(process_t* proc);

/**
 * Exit current process
 */
void process_exit(int code);

/**
 * Get process by PID
 */
process_t* process_get_by_pid(u32 pid);

#endif // PROCESS_H
