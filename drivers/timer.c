/* PIT (Programmable Interval Timer) Driver */

#include <stdint.h>
#include "timer.h"
#include "idt.h"

/* PIT I/O ports */
#define PIT_CHANNEL0    0x40
#define PIT_COMMAND     0x43

/* PIT base frequency */
#define PIT_FREQUENCY   1193182

/* Timer state */
static volatile uint64_t timer_ticks = 0;
static uint32_t timer_freq = 0;

/* I/O port write */
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

/* Timer IRQ handler */
static void timer_callback(uint64_t int_num, uint64_t error_code) {
    (void)int_num;
    (void)error_code;
    timer_ticks++;
}

void timer_init(uint32_t frequency) {
    timer_freq = frequency;
    
    /* Calculate divisor */
    uint32_t divisor = PIT_FREQUENCY / frequency;
    
    /* Send command byte: Channel 0, lobyte/hibyte, rate generator */
    outb(PIT_COMMAND, 0x36);
    
    /* Send divisor */
    outb(PIT_CHANNEL0, divisor & 0xFF);         /* Low byte */
    outb(PIT_CHANNEL0, (divisor >> 8) & 0xFF);  /* High byte */
    
    /* Register timer handler (IRQ0 = INT 32) */
    register_interrupt_handler(32, timer_callback);
}

uint64_t timer_get_ticks(void) {
    return timer_ticks;
}

uint64_t timer_get_uptime(void) {
    if (timer_freq == 0) return 0;
    return timer_ticks / timer_freq;
}

void timer_sleep(uint32_t ms) {
    uint64_t target = timer_ticks + (ms * timer_freq / 1000);
    while (timer_ticks < target) {
        __asm__ volatile ("hlt");
    }
}
