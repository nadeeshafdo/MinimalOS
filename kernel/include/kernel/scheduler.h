/* Scheduler header for x86_64 */
#ifndef KERNEL_SCHEDULER_H
#define KERNEL_SCHEDULER_H

#include <kernel/process.h>

/* Default time slice (in timer ticks) */
#define DEFAULT_TIME_SLICE 10

void scheduler_init(void);
void scheduler_add(process_t *proc);
void scheduler_remove(process_t *proc);
process_t *scheduler_next(void);
void scheduler_tick(void);
void scheduler_start(void);

#endif
