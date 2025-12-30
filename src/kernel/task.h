/**
 * MinimalOS - Task Management
 * Task Control Block and scheduling primitives
 */

#ifndef KERNEL_TASK_H
#define KERNEL_TASK_H

#include <minimalos/types.h>

/* Task states */
typedef enum {
  TASK_RUNNING, /* Currently executing on CPU */
  TASK_READY,   /* Ready to run */
  TASK_BLOCKED, /* Waiting for event */
  TASK_ZOMBIE,  /* Terminated, waiting for cleanup */
} task_state_t;

/* Maximum number of tasks */
#define MAX_TASKS 64

/* Task stack size */
#define TASK_STACK_SIZE (4 * 4096) /* 16KB per task */

/* CPU context saved during context switch */
struct cpu_context {
  uint64_t r15;
  uint64_t r14;
  uint64_t r13;
  uint64_t r12;
  uint64_t rbx;
  uint64_t rbp;
  uint64_t rip;
} __packed;

/* Task Control Block */
struct task {
  uint64_t tid;       /* Task ID */
  task_state_t state; /* Current state */

  /* CPU context */
  struct cpu_context *context; /* Saved context pointer (RSP) */

  /* Stack */
  void *stack_base;  /* Base of allocated stack */
  size_t stack_size; /* Stack size */

  /* Scheduling */
  uint64_t time_slice;  /* Remaining time slice */
  uint64_t total_ticks; /* Total CPU ticks used */

  /* Name for debugging */
  char name[32];

  /* Linked list pointers */
  struct task *next;
  struct task *prev;
};

/* Current running task */
extern struct task *current_task;

/**
 * Initialize the scheduler
 */
void sched_init(void);

/**
 * Create a new kernel task
 * @param entry Entry point function
 * @param name Task name for debugging
 * @return Task pointer, or NULL on failure
 */
struct task *task_create(void (*entry)(void), const char *name);

/**
 * Exit current task
 * @param status Exit status
 */
void task_exit(int status);

/**
 * Yield CPU to another task
 */
void task_yield(void);

/**
 * Called from timer interrupt to handle preemption
 */
void sched_tick(void);

/**
 * Schedule next task (called with interrupts disabled)
 */
void schedule(void);

/**
 * Context switch (assembly function)
 * @param old_context Pointer to save current context
 * @param new_context Pointer to load new context
 */
extern void context_switch(struct cpu_context **old_context,
                           struct cpu_context *new_context);

#endif /* KERNEL_TASK_H */
