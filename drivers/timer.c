#include <stdint.h>
#include <kernel/timer.h>
#include <kernel/irq.h>
#include <kernel/scheduler.h>

/* I/O port operations */
static inline void outb(uint16_t port, uint8_t value) {
    __asm__ volatile ("outb %0, %1" : : "a"(value), "Nd"(port));
}

/* PIT constants */
#define PIT_CHANNEL0 0x40
#define PIT_COMMAND  0x43
#define PIT_FREQUENCY 1193180

static uint32_t timer_ticks = 0;

/* Timer interrupt handler */
static void timer_handler(struct registers* regs) {
    (void)regs;  /* Unused */
    timer_ticks++;
    scheduler_tick();
}

uint32_t timer_get_ticks(void) {
    return timer_ticks;
}

void timer_init(uint32_t frequency) {
    /* Register timer interrupt handler */
    irq_register_handler(0, timer_handler);
    
    /* Calculate divisor */
    uint32_t divisor = PIT_FREQUENCY / frequency;
    
    /* Set command byte: channel 0, lobyte/hibyte, rate generator */
    outb(PIT_COMMAND, 0x36);
    
    /* Set frequency */
    outb(PIT_CHANNEL0, (uint8_t)(divisor & 0xFF));
    outb(PIT_CHANNEL0, (uint8_t)((divisor >> 8) & 0xFF));
}
