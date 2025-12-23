#ifndef _KERNEL_TIMER_H
#define _KERNEL_TIMER_H

#include <stdint.h>

/* Initialize PIT timer */
void timer_init(uint32_t frequency);

/* Get system uptime in ticks */
uint32_t timer_get_ticks(void);

#endif /* _KERNEL_TIMER_H */
