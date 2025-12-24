#ifndef _SCHEDULER_H
#define _SCHEDULER_H

#include <stdint.h>

/* Initialize scheduler */
void scheduler_init(void);

/* Start scheduler (called after all initialization) */
void scheduler_start(void);

/* Called by timer interrupt */
void scheduler_tick(void);

/* Get scheduler tick count */
uint64_t scheduler_get_ticks(void);

#endif /* _SCHEDULER_H */
