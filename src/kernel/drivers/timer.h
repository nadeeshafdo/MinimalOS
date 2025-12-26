#ifndef TIMER_H
#define TIMER_H

#include "../include/types.h"

// Timer frequency (Hz)
#define TIMER_FREQUENCY 100  // 100 Hz = 10ms tick

/**
 * Initialize the Programmable Interval Timer (PIT)
 */
void timer_init(void);

/**
 * Get number of ticks since boot
 */
u64 timer_get_ticks(void);

/**
 * Register a timer callback function
 */
typedef void (*timer_callback_t)(void);
void timer_register_callback(timer_callback_t callback);

#endif // TIMER_H
