/**
 * MinimalOS - Timer Implementation
 * APIC timer calibration using PIT
 * Supports both APIC Periodic Timer and TSC-Deadline Timer
 */

#include "timer.h"
#include "apic.h"
#include "cpu.h"
#include "idt.h"

extern void printk(const char *fmt, ...);

/* PIT (Programmable Interval Timer) ports and constants */
#define PIT_CHANNEL0 0x40
#define PIT_COMMAND 0x43

/* I/O port access */
static inline void outb(uint16_t port, uint8_t val) {
  __asm__ volatile("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
  uint8_t ret;
  __asm__ volatile("inb %1, %0" : "=a"(ret) : "Nd"(port));
  return ret;
}

/* Timer state */
static volatile uint64_t timer_ticks = 0;
static uint32_t apic_ticks_per_ms = 0;

/* TSC-Deadline state */
static bool using_tsc_deadline = false;
static uint64_t tsc_frequency = 0;
static uint64_t tsc_deadline_interval = 0;
static uint64_t next_tsc_deadline = 0;

/**
 * PIT one-shot delay for calibration
 * Waits for approximately 10ms
 */
static void pit_wait_10ms(void) {
  /* Set PIT channel 0 to one-shot mode */
  /* Divisor for 10ms: 1193182 / 100 = 11932 */
  uint16_t divisor = 11932;

  outb(PIT_COMMAND, 0x30); /* Channel 0, lobyte/hibyte, mode 0 */
  outb(PIT_CHANNEL0, divisor & 0xFF);
  outb(PIT_CHANNEL0, (divisor >> 8) & 0xFF);

  /* Wait for counter to reach 0 by polling */
  uint16_t count;
  do {
    outb(PIT_COMMAND, 0x00);
    count = inb(PIT_CHANNEL0);
    count |= (uint16_t)inb(PIT_CHANNEL0) << 8;
  } while (count > 0 && count <= divisor);
}

/**
 * Calibrate APIC timer using PIT
 * Returns APIC timer ticks per millisecond
 */
static uint32_t calibrate_apic_timer(void) {
  extern void apic_timer_one_shot(uint32_t count);
  extern uint32_t apic_timer_read_current(void);

  /* Set APIC timer to a large initial count */
  uint32_t initial_count = 0xFFFFFFFF;

  apic_timer_one_shot(initial_count);
  pit_wait_10ms();

  uint32_t current = apic_timer_read_current();
  uint32_t elapsed = initial_count - current;

  return elapsed / 10;
}

/**
 * Initialize timer subsystem
 */
void timer_init(void) {
  printk("  Initializing timer...\n");

  /* Check for TSC-Deadline support */
  if (cpu_info.tsc_deadline_supported) {
    printk("  TSC-Deadline support detected\n");
    printk("  Calibrating TSC...\n");

    uint64_t t1 = rdtsc();
    pit_wait_10ms();
    uint64_t t2 = rdtsc();

    /* Calculate TSC frequency (ticks per 10ms * 100) */
    tsc_frequency = (t2 - t1) * 100;
    tsc_deadline_interval = tsc_frequency / TIMER_FREQUENCY_HZ;

    printk("  TSC Frequency: %lu Hz\n", tsc_frequency);

    /* Initialize TSC Deadline mode */
    apic_timer_tsc_deadline_init();

    /* Arm first deadline */
    using_tsc_deadline = true;
    next_tsc_deadline = rdtsc() + tsc_deadline_interval;
    apic_timer_arm(next_tsc_deadline);

    printk("  TSC-Deadline timer configured: %u Hz\n", TIMER_FREQUENCY_HZ);
    return;
  }

  /* Fallback to Periodic APIC Timer */
  printk("  Using Legacy APIC Periodic Timer\n");
  printk("  Calibrating APIC timer...\n");

  apic_ticks_per_ms = calibrate_apic_timer();

  if (apic_ticks_per_ms == 0) {
    printk("  WARNING: APIC calibration failed, using default\n");
    apic_ticks_per_ms = 100000;
  }

  printk("  APIC timer: %u ticks/ms\n", apic_ticks_per_ms);

  uint32_t count = (apic_ticks_per_ms * 1000) / TIMER_FREQUENCY_HZ;
  extern void apic_timer_periodic(uint32_t count);
  apic_timer_periodic(count);

  printk("  Periodic timer configured: %u Hz\n", TIMER_FREQUENCY_HZ);
}

/**
 * Get current tick count
 */
uint64_t timer_get_ticks(void) { return timer_ticks; }

/**
 * Get milliseconds since boot
 */
uint64_t timer_get_ms(void) { return timer_ticks * TIMER_MS_PER_TICK; }

/**
 * Busy-wait sleep
 */
void timer_sleep_ms(uint32_t ms) {
  uint64_t target = timer_get_ms() + ms;
  while (timer_get_ms() < target) {
    __asm__ volatile("pause");
  }
}

/**
 * Timer tick handler - called from ISR
 */
void timer_tick_handler(void) {
  timer_ticks++;

  /* Re-arm TSC Deadline timer if active */
  if (using_tsc_deadline) {
    next_tsc_deadline += tsc_deadline_interval;
    apic_timer_arm(next_tsc_deadline);
  }

  /* Call scheduler */
  extern void sched_tick(void);
  sched_tick();
}
