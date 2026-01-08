/* Timer header for x86_64 */
#ifndef KERNEL_TIMER_H
#define KERNEL_TIMER_H

#include <stdint.h>

void timer_init(uint32_t frequency);
uint32_t timer_get_ticks(void);

#endif
