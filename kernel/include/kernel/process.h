#ifndef _KERNEL_PROCESS_H
#define _KERNEL_PROCESS_H

#include <stdint.h>
#include <kernel/paging.h>

/* Maximum number of processes */
#define MAX_PROCESSES 256

/* Process states */
typedef enum {
    PROCESS_STATE_UNUSED = 0,
    PROCESS_STATE_READY,
    PROCESS_STATE_RUNNING,
    PROCESS_STATE_BLOCKED,
    PROCESS_STATE_ZOMBIE
} process_state_t;

/* CPU context for context switching */
typedef struct {
    uint32_t edi;
    uint32_t esi;
    uint32_t ebp;
    uint32_t esp;
    uint32_t ebx;
    uint32_t edx;
    uint32_t ecx;
    uint32_t eax;
    uint32_t eip;
    uint32_t cs;
    uint32_t eflags;
} cpu_context_t;

/* Process Control Block (PCB) */
typedef struct process {
    uint32_t pid;               /* Process ID */
    process_state_t state;      /* Current state */
    char name[32];              /* Process name */
    
    /* Memory */
    page_directory_t *page_dir; /* Page directory */
    uint32_t kernel_stack;      /* Kernel stack pointer */
    uint32_t user_stack;        /* User stack pointer */
    
    /* CPU state */
    cpu_context_t context;      /* Saved CPU state */
    
    /* Scheduling */
    uint32_t priority;          /* Process priority */
    uint32_t time_slice;        /* Time slice remaining */
    
    /* Linked list for scheduler */
    struct process *next;       /* Next process in queue */
} process_t;

/* Initialize process subsystem */
void process_init(void);

/* Create a new kernel process */
process_t *process_create(const char *name, void (*entry)(void));

/* Exit current process */
void process_exit(int status);

/* Get current running process */
process_t *process_current(void);

/* Get process by PID */
process_t *process_get(uint32_t pid);

/* Yield CPU to next process */
void process_yield(void);

/* Switch to user mode (never returns) */
void enter_user_mode(void *entry_point, uint32_t user_stack_top);

#endif /* _KERNEL_PROCESS_H */
