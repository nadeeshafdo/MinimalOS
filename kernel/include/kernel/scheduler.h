#ifndef _KERNEL_SCHEDULER_H
#define _KERNEL_SCHEDULER_H

#include <kernel/process.h>

/* Default time slice (in timer ticks) */
#define DEFAULT_TIME_SLICE 10

/* Initialize scheduler */
void scheduler_init(void);

/* Add process to ready queue */
void scheduler_add(process_t *proc);

/* Remove process from ready queue */
void scheduler_remove(process_t *proc);

/* Get next process to run */
process_t *scheduler_next(void);

/* Scheduler tick (called from timer interrupt) */
void scheduler_tick(void);

/* Start scheduling (never returns) */
void scheduler_start(void);

#endif /* _KERNEL_SCHEDULER_H */
