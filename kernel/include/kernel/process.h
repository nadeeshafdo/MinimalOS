/* Process management header for x86_64 */
#ifndef KERNEL_PROCESS_H
#define KERNEL_PROCESS_H

#include <kernel/paging.h>
#include <stdint.h>

/* Process states */
typedef enum {
  PROCESS_STATE_NEW,
  PROCESS_STATE_READY,
  PROCESS_STATE_RUNNING,
  PROCESS_STATE_BLOCKED,
  PROCESS_STATE_ZOMBIE
} process_state_t;

/* CPU context for x86_64 */
typedef struct {
  uint64_t r15, r14, r13, r12; /* Callee-saved registers */
  uint64_t rbx, rbp;
  uint64_t rsp;    /* Stack pointer */
  uint64_t rip;    /* Instruction pointer */
  uint64_t rflags; /* Flags register */
} cpu_context_t;

/* Process Control Block */
typedef struct process {
  uint32_t pid;          /* Process ID */
  process_state_t state; /* Current state */
  char name[32];         /* Process name */

  cpu_context_t context;      /* CPU context */
  page_directory_t *page_dir; /* Page directory */
  uint64_t kernel_stack;      /* Kernel stack pointer */

  uint32_t priority;   /* Scheduling priority */
  uint32_t time_slice; /* Time slice remaining */

  struct process *next; /* Next process in queue */
} process_t;

/* Maximum processes */
#define MAX_PROCESSES 256

/* Default time slice (in timer ticks) */
#define DEFAULT_TIME_SLICE 10

/* Functions */
void process_init(void);
process_t *process_create(const char *name, void (*entry)(void));
void process_exit(int status);
process_t *process_current(void);
process_t *process_get(uint32_t pid);
void process_yield(void);
void process_switch(process_t *next);

#endif
