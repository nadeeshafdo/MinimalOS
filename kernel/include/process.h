#ifndef _PROCESS_H
#define _PROCESS_H

#include <stdint.h>

/* Process states */
typedef enum {
    PROCESS_READY,
    PROCESS_RUNNING,
    PROCESS_BLOCKED,
    PROCESS_TERMINATED
} process_state_t;

/* CPU context (saved on stack during context switch) */
typedef struct {
    uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
    uint64_t rdi, rsi, rbp, rbx, rdx, rcx, rax;
    uint64_t rip, cs, rflags, rsp, ss;
} __attribute__((packed)) cpu_context_t;

/* Process Control Block */
typedef struct process {
    uint64_t pid;              /* Process ID */
    process_state_t state;     /* Current state */
    
    uint64_t rsp;              /* Saved stack pointer */
    uint64_t rip;              /* Entry point (for new processes) */
    
    uint64_t *stack;           /* Kernel stack base */
    uint64_t stack_size;       /* Stack size */
    
    const char *name;          /* Process name */
    
    struct process *next;      /* Next process in list */
} process_t;

/* Initialize process management */
void process_init(void);

/* Create a new kernel task */
process_t *process_create(const char *name, void (*entry)(void));

/* Get current process */
process_t *process_current(void);

/* Get process by PID */
process_t *process_get(uint64_t pid);

/* Yield CPU to scheduler */
void process_yield(void);

/* Terminate current process */
void process_exit(void);

/* Get process count */
uint64_t process_count(void);

#endif /* _PROCESS_H */
