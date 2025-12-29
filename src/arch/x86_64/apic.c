/**
 * MinimalOS - APIC Initialization (Local APIC / x2APIC)
 */

#include "apic.h"
#include "cpu.h"
#include "idt.h"

extern void printk(const char *fmt, ...);
extern struct cpu_info cpu_info;

/* APIC base address (xAPIC mode only) */
static volatile uint32_t *lapic_base = NULL;
static bool x2apic_mode = false;

/* Virtual address for LAPIC (memory-mapped) */
#define LAPIC_VIRT_BASE 0xFFFFFFFF80000000UL + 0xFEE00000UL

/**
 * Read LAPIC register
 */
static uint32_t lapic_read(uint32_t reg) {
  if (x2apic_mode) {
    return (uint32_t)rdmsr(X2APIC_MSR_BASE + (reg >> 4));
  } else {
    return lapic_base[reg / 4];
  }
}

/**
 * Write LAPIC register
 */
static void lapic_write(uint32_t reg, uint32_t value) {
  if (x2apic_mode) {
    wrmsr(X2APIC_MSR_BASE + (reg >> 4), value);
  } else {
    lapic_base[reg / 4] = value;
  }
}

/**
 * Check if running in x2APIC mode
 */
bool apic_is_x2apic(void) { return x2apic_mode; }

/**
 * Get local APIC ID
 */
uint32_t apic_get_id(void) {
  if (x2apic_mode) {
    return (uint32_t)rdmsr(X2APIC_ID);
  } else {
    return lapic_read(LAPIC_ID) >> 24;
  }
}

/**
 * Send End of Interrupt
 */
void apic_eoi(void) {
  if (x2apic_mode) {
    wrmsr(X2APIC_EOI, 0);
  } else {
    lapic_write(LAPIC_EOI, 0);
  }
}

/**
 * Send Inter-Processor Interrupt (IPI)
 */
void apic_send_ipi(uint32_t apic_id, uint32_t vector) {
  if (x2apic_mode) {
    /* x2APIC: single 64-bit write to ICR */
    uint64_t icr = ((uint64_t)apic_id << 32) | vector;
    wrmsr(X2APIC_ICR, icr);
  } else {
    /* xAPIC: write destination to ICR_HI, then command to ICR_LO */
    lapic_write(LAPIC_ICR_HI, apic_id << 24);
    lapic_write(LAPIC_ICR_LO, vector);

    /* Wait for delivery */
    while (lapic_read(LAPIC_ICR_LO) & (1 << 12)) {
      pause();
    }
  }
}

/**
 * Initialize APIC timer
 */
void apic_timer_init(uint32_t frequency_hz) {
  /* We'll calibrate using PIT later; for now just set up basic timer */

  /* Set divider to 16 */
  lapic_write(LAPIC_TIMER_DCR, TIMER_DIV_16);

  /* Set up LVT timer entry: periodic mode, vector 32 (IRQ_TIMER) */
  lapic_write(LAPIC_LVT_TIMER, IRQ_TIMER | TIMER_PERIODIC);

  /* Set initial count (rough estimate - should be calibrated) */
  /* Assume ~1GHz bus, div 16 = ~62.5 MHz timer */
  /* For 100 Hz = 625000 counts */
  uint32_t initial_count = 625000; /* ~100 Hz on most systems */

  if (frequency_hz != 0 && frequency_hz != 100) {
    initial_count = (625000 * 100) / frequency_hz;
  }

  lapic_write(LAPIC_TIMER_ICR, initial_count);

  printk("  APIC timer: vector %u, divider 16, count %u\n", IRQ_TIMER,
         initial_count);
}

/**
 * Initialize Local APIC
 */
void apic_init(void) {
  uint64_t apic_base_msr;

  /* Read APIC base MSR */
  apic_base_msr = rdmsr(MSR_IA32_APIC_BASE);

  /* Check if APIC is enabled */
  if (!(apic_base_msr & (1 << 11))) {
    printk("  WARNING: APIC not enabled in MSR\n");
    apic_base_msr |= (1 << 11); /* Enable APIC */
    wrmsr(MSR_IA32_APIC_BASE, apic_base_msr);
  }

  /* Check for x2APIC support and enable if available */
  if (cpu_info.x2apic_supported) {
    /* Enable x2APIC mode */
    apic_base_msr |= (1 << 10); /* x2APIC enable bit */
    wrmsr(MSR_IA32_APIC_BASE, apic_base_msr);
    x2apic_mode = true;
    printk("  x2APIC mode enabled\n");
  } else {
    /* Use xAPIC (memory-mapped) */
    lapic_base = (volatile uint32_t *)LAPIC_VIRT_BASE;
    x2apic_mode = false;
    printk("  xAPIC mode (base: 0x%lx)\n", (uint64_t)lapic_base);
  }

  /* Get APIC ID */
  uint32_t id = apic_get_id();
  printk("  Local APIC ID: %u\n", id);

  /* Set spurious interrupt vector and enable APIC */
  uint32_t svr = lapic_read(LAPIC_SVR);
  svr |= SVR_ENABLE;   /* Enable APIC */
  svr |= IRQ_SPURIOUS; /* Spurious vector = 255 */
  lapic_write(LAPIC_SVR, svr);

  /* Mask all LVT entries initially */
  lapic_write(LAPIC_LVT_TIMER, LVT_MASKED);
  lapic_write(LAPIC_LVT_LINT0, LVT_MASKED);
  lapic_write(LAPIC_LVT_LINT1, LVT_MASKED);
  lapic_write(LAPIC_LVT_ERROR, LVT_MASKED);

  /* Clear error status by writing twice */
  lapic_write(LAPIC_ESR, 0);
  lapic_write(LAPIC_ESR, 0);

  /* Send EOI to clear any pending interrupts */
  apic_eoi();

  /* Set task priority to 0 (accept all interrupts) */
  lapic_write(LAPIC_TPR, 0);

  printk("  Local APIC enabled\n");
}
