#ifndef SCHEDULER_H
#define SCHEDULER_H

#include "process.h"

/**
 * Initialize the scheduler
 */
void scheduler_init(void);

/**
 * Add a process to the ready queue
 */
void scheduler_add_process(process_t* proc);

/**
 * Remove a process from the ready queue
 */
void scheduler_remove_process(process_t* proc);

/**
 * Schedule next process (called on timer interrupt)
 * This performs context switching
 */
void schedule(void);

/**
 * Yield CPU voluntarily
 */
void yield(void);

/**
 * Enable/disable scheduler
 */
void scheduler_enable(void);
void scheduler_disable(void);

#endif // SCHEDULER_H
