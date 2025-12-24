/* Round-robin scheduler */

#include <stdint.h>
#include "scheduler.h"
#include "process.h"

/* Scheduler state */
static uint64_t scheduler_ticks = 0;
static int scheduler_enabled = 0;

/* Time slice in ticks */
#define TIME_SLICE 10

void scheduler_init(void) {
    scheduler_enabled = 0;
    scheduler_ticks = 0;
}

void scheduler_start(void) {
    scheduler_enabled = 1;
}

void scheduler_tick(void) {
    scheduler_ticks++;
    
    /* Preemptive scheduling disabled for now - shell needs priority */
    /* if (scheduler_enabled && (scheduler_ticks % TIME_SLICE) == 0) {
        process_yield();
    } */
}

uint64_t scheduler_get_ticks(void) {
    return scheduler_ticks;
}
