#ifndef _TIMER_H
#define _TIMER_H

#include <stdint.h>

/* Initialize PIT timer with given frequency (Hz) */
void timer_init(uint32_t frequency);

/* Called by timer IRQ handler to increment tick count */
void timer_tick(void);

/* Get current tick count */
uint64_t timer_get_ticks(void);

/* Get uptime in seconds */
uint64_t timer_get_uptime(void);

/* Sleep for approximately ms milliseconds */
void timer_sleep(uint32_t ms);

#endif /* _TIMER_H */
