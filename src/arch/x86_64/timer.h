/**
 * MinimalOS - Timer Interface
 * APIC timer calibration and management
 */

#ifndef ARCH_X86_64_TIMER_H
#define ARCH_X86_64_TIMER_H

#include <minimalos/types.h>

/* Timer configuration */
#define TIMER_FREQUENCY_HZ 100 /* 100 Hz = 10ms per tick */
#define TIMER_MS_PER_TICK (1000 / TIMER_FREQUENCY_HZ)

/**
 * Initialize the timer subsystem
 * Calibrates APIC timer using PIT and starts periodic interrupts
 */
void timer_init(void);

/**
 * Get current tick count since boot
 */
uint64_t timer_get_ticks(void);

/**
 * Get milliseconds since boot
 */
uint64_t timer_get_ms(void);

/**
 * Busy-wait for specified milliseconds
 * Note: This is a blocking call
 */
void timer_sleep_ms(uint32_t ms);

/**
 * Called from timer interrupt handler
 */
void timer_tick_handler(void);

#endif /* ARCH_X86_64_TIMER_H */
