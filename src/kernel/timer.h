#ifndef TIMER_H
#define TIMER_H

#include "stdint.h"

// Initialize PIT timer
void timer_init(void);

// Get uptime in ticks (1 tick = 10ms at 100Hz)
uint32_t get_uptime_ticks(void);

// Get uptime in seconds
uint32_t get_uptime_seconds(void);

// Get formatted uptime string (HH:MM:SS)
void get_uptime_string(char* buffer);

#endif
