#include "timer.h"

#define PIT_FREQUENCY 1193182
#define TIMER_HZ 100

static volatile uint32_t tick_count = 0;

// Port I/O functions
static inline void outb(uint16_t port, uint8_t value) {
    asm volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

// Timer interrupt handler
void timer_handler(void) {
    tick_count++;
    
    // Send EOI to PIC
    outb(0x20, 0x20);
}

// Assembly wrapper for timer interrupt
extern void timer_interrupt_stub(void);
asm(
    ".global timer_interrupt_stub\n"
    "timer_interrupt_stub:\n"
    "   pusha\n"
    "   call timer_handler\n"
    "   popa\n"
    "   iret\n"
);

void timer_init(void) {
    // Calculate divisor for desired frequency
    uint32_t divisor = PIT_FREQUENCY / TIMER_HZ;
    
    // Send command byte
    outb(0x43, 0x36);
    
    // Send divisor
    outb(0x40, divisor & 0xFF);
    outb(0x40, (divisor >> 8) & 0xFF);
    
    // Timer IRQ is IRQ0, which maps to INT 0x20 after PIC remap
    // IDT entry will be set in main.c
}

uint32_t get_uptime_ticks(void) {
    return tick_count;
}

uint32_t get_uptime_seconds(void) {
    return tick_count / TIMER_HZ;
}

void get_uptime_string(char* buffer) {
    uint32_t seconds = get_uptime_seconds();
    uint32_t hours = seconds / 3600;
    uint32_t minutes = (seconds % 3600) / 60;
    uint32_t secs = seconds % 60;
    
    // Format as HH:MM:SS
    buffer[0] = '0' + (hours / 10);
    buffer[1] = '0' + (hours % 10);
    buffer[2] = ':';
    buffer[3] = '0' + (minutes / 10);
    buffer[4] = '0' + (minutes % 10);
    buffer[5] = ':';
    buffer[6] = '0' + (secs / 10);
    buffer[7] = '0' + (secs % 10);
    buffer[8] = '\0';
}
