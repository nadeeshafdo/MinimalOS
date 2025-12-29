/**
 * MinimalOS - Task Management and Scheduler
 * Simple round-robin scheduler with preemption
 */

#include "task.h"
#include "../mm/heap.h"
#include <arch/x86_64/cpu.h>

extern void printk(const char *fmt, ...);

/* Task list */
static struct task *task_list = NULL;
static struct task *idle_task = NULL;
struct task *current_task = NULL;
static uint64_t next_tid = 0;

/* Time slice in ticks */
#define DEFAULT_TIME_SLICE 10 /* 100ms at 100Hz */

/**
 * Idle task - runs when no other task is ready
 */
static void idle_entry(void) {
  for (;;) {
    __asm__ volatile("hlt");
  }
}

/**
 * Task wrapper - ensures task_exit is called if entry returns
 * (Will be used when we add proper entry wrapper support)
 */
__attribute__((unused)) static void task_wrapper(void (*entry)(void)) {
  entry();
  task_exit(0);
}

/**
 * Add task to the task list
 */
static void task_list_add(struct task *task) {
  if (!task_list) {
    task_list = task;
    task->next = task;
    task->prev = task;
  } else {
    task->next = task_list;
    task->prev = task_list->prev;
    task_list->prev->next = task;
    task_list->prev = task;
  }
}

/**
 * Remove task from the task list
 * (Will be used for task cleanup)
 */
__attribute__((unused)) static void task_list_remove(struct task *task) {
  if (task->next == task) {
    /* Only task in list */
    task_list = NULL;
  } else {
    task->prev->next = task->next;
    task->next->prev = task->prev;
    if (task_list == task) {
      task_list = task->next;
    }
  }
  task->next = NULL;
  task->prev = NULL;
}

/**
 * Create a new task
 */
struct task *task_create(void (*entry)(void), const char *name) {
  /* Allocate TCB */
  struct task *task = kzalloc(sizeof(struct task));
  if (!task) {
    printk("SCHED: Failed to allocate TCB\n");
    return NULL;
  }

  /* Allocate stack */
  task->stack_base = kmalloc(TASK_STACK_SIZE);
  if (!task->stack_base) {
    kfree(task);
    printk("SCHED: Failed to allocate stack\n");
    return NULL;
  }
  task->stack_size = TASK_STACK_SIZE;

  /* Setup initial context on top of stack */
  uint64_t *stack_top =
      (uint64_t *)((uint8_t *)task->stack_base + TASK_STACK_SIZE);

  /* Push initial context (will be popped by context_switch) */
  /* Stack layout (from high to low addresses):
   *   [return address - entry point]
   *   rbp
   *   rbx
   *   r12
   *   r13
   *   r14
   *   r15 <- RSP points here after setup
   */

  *(--stack_top) = (uint64_t)entry; /* RIP - return address */
  *(--stack_top) = 0;               /* RBP */
  *(--stack_top) = 0;               /* RBX */
  *(--stack_top) = 0;               /* R12 */
  *(--stack_top) = 0;               /* R13 */
  *(--stack_top) = 0;               /* R14 */
  *(--stack_top) = 0;               /* R15 */

  task->context = (struct cpu_context *)stack_top;

  /* Initialize task fields */
  task->tid = next_tid++;
  task->state = TASK_READY;
  task->time_slice = DEFAULT_TIME_SLICE;
  task->time_slice = DEFAULT_TIME_SLICE;
  task->total_ticks = 0;
  task->fs_base = 0; /* Default user TLS base */

  /* Copy name */
  const char *src = name;
  char *dst = task->name;
  int i = 0;
  while (*src && i < 31) {
    *dst++ = *src++;
    i++;
  }
  *dst = '\0';

  /* Add to task list */
  task_list_add(task);

  printk("SCHED: Created task %lu '%s'\n", task->tid, task->name);

  return task;
}

/**
 * Exit current task
 */
void task_exit(int status) {
  (void)status; /* Unused for now */

  if (!current_task || current_task == idle_task) {
    printk("SCHED: Cannot exit idle task!\n");
    for (;;)
      __asm__ volatile("hlt");
  }

  printk("SCHED: Task %lu '%s' exiting\n", current_task->tid,
         current_task->name);

  current_task->state = TASK_ZOMBIE;

  /* Switch to another task */
  schedule();

  /* Should never reach here */
  for (;;)
    __asm__ volatile("hlt");
}

/**
 * Yield CPU voluntarily
 */
void task_yield(void) {
  current_task->time_slice = 0;
  schedule();
}

/**
 * Find next ready task (round-robin)
 */
static struct task *find_next_ready(void) {
  if (!task_list)
    return idle_task;

  struct task *start = current_task ? current_task->next : task_list;
  struct task *task = start;

  /* Look for a ready task that isn't idle (prefer real work) */
  do {
    if (task != idle_task && task->state == TASK_READY) {
      return task;
    }
    task = task->next;
  } while (task != start);

  /* If current task is still runnable, continue with it */
  if (current_task && current_task != idle_task &&
      (current_task->state == TASK_RUNNING ||
       current_task->state == TASK_READY)) {
    return current_task;
  }

  /* Fall back to idle */
  return idle_task;
}

/**
 * Schedule next task
 */
void schedule(void) {
  struct task *next = find_next_ready();

  if (next == current_task) {
    /* Continue running current task */
    current_task->time_slice = DEFAULT_TIME_SLICE;
    return;
  }

  /* Switch tasks */
  struct task *prev = current_task;

  if (prev && prev->state == TASK_RUNNING) {
    prev->state = TASK_READY;
  }

  current_task = next;
  current_task->state = TASK_RUNNING;
  current_task->time_slice = DEFAULT_TIME_SLICE;

  /* Update per-cpu data for syscalls */
  extern struct per_cpu_data bsp_cpu_data;
  bsp_cpu_data.current_task = next;
  bsp_cpu_data.kernel_stack =
      (uint64_t)((uint8_t *)next->stack_base + next->stack_size);

  /* Perform context switch */
  if (prev) {
    if (prev->fs_base != next->fs_base) {
      wrmsr(MSR_IA32_FS_BASE, next->fs_base);
    }
    context_switch(&prev->context, next->context);
  } else {
    /* First task - just load context */
    wrmsr(MSR_IA32_FS_BASE, next->fs_base);
    context_switch(&idle_task->context, next->context);
  }
}

/**
 * Timer tick handler for preemption
 * Note: Preemptive scheduling from interrupt context requires
 * saving/restoring the full interrupt frame, which is more complex.
 * For now, just track tick counts.
 */
void sched_tick(void) {
  if (!current_task)
    return;

  current_task->total_ticks++;

  if (current_task->time_slice > 0) {
    current_task->time_slice--;
  }

  /* TODO: Preemptive scheduling requires saving interrupt context */
  /* For now, rely on cooperative scheduling via task_yield() */
}

/**
 * Initialize scheduler
 */
void sched_init(void) {
  printk("  Creating idle task...\n");

  /* Create idle task */
  idle_task = task_create(idle_entry, "idle");
  if (!idle_task) {
    printk("  ERROR: Failed to create idle task!\n");
    return;
  }

  /* Idle task starts as current */
  current_task = idle_task;
  current_task->state = TASK_RUNNING;

  printk("  Scheduler ready\n");
}
