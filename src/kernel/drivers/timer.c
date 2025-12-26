#include "timer.h"
#include "../arch/x86_64/idt.h"
#include "../lib/printk.h"

#define PIT_CHANNEL0 0x40
#define PIT_COMMAND  0x43
#define PIT_BASE_FREQ 1193182  // PIT base frequency in Hz

static u64 ticks = 0;
static timer_callback_t callback = NULL;

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static void timer_interrupt_handler(struct registers* regs) {
    (void)regs;  // Unused
    
    ticks++;
    
    if (callback != NULL) {
        callback();
    }
}

void timer_init(void) {
    printk("[TIMER] Initializing PIT at %u Hz...\n", TIMER_FREQUENCY);
    
    // Register timer interrupt handler (IRQ0 = interrupt 32)
    register_interrupt_handler(32, timer_interrupt_handler);
    
    // Calculate divisor
    u32 divisor = PIT_BASE_FREQ / TIMER_FREQUENCY;
    
    printk("[TIMER] PIT divisor: %u\n", divisor);
    
    // Send command byte: channel 0, lobyte/hibyte, rate generator
    outb(PIT_COMMAND, 0x36);
    
    // Send divisor
    outb(PIT_CHANNEL0, (u8)(divisor & 0xFF));
    outb(PIT_CHANNEL0, (u8)((divisor >> 8) & 0xFF));
    
    // Unmask IRQ0 in PIC
    u8 mask;
    __asm__ volatile("inb %1, %0" : "=a"(mask) : "Nd"((u16)0x21));
    mask &= ~0x01;  // Unmask IRQ0
    outb(0x21, mask);
    
    printk("[TIMER] Initialization complete! (tick every %u ms)\n", 1000 / TIMER_FREQUENCY);
}

u64 timer_get_ticks(void) {
    return ticks;
}

void timer_register_callback(timer_callback_t cb) {
    callback = cb;
}
