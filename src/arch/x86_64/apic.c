/**
 * MinimalOS - APIC Initialization (Local APIC / x2APIC)
 */

#include "apic.h"
#include "cpu.h"
#include "idt.h"

extern void printk(const char *fmt, ...);
extern struct cpu_info cpu_info;

/* APIC state */
static volatile uint32_t *lapic_base = NULL;
static bool x2apic_mode = false;
static bool apic_initialized = false;

/* Virtual address for LAPIC (memory-mapped) - requires proper page table
 * mapping */
#define LAPIC_VIRT_BASE (0xFFFFFFFF80000000UL + 0xFEE00000UL)

/**
 * Read LAPIC register - only valid when APIC is initialized
 */
static uint32_t lapic_read(uint32_t reg) {
  if (x2apic_mode) {
    return (uint32_t)rdmsr(X2APIC_MSR_BASE + (reg >> 4));
  } else if (lapic_base != NULL) {
    return lapic_base[reg / 4];
  }
  return 0;
}

/**
 * Write LAPIC register - only valid when APIC is initialized
 */
static void lapic_write(uint32_t reg, uint32_t value) {
  if (x2apic_mode) {
    wrmsr(X2APIC_MSR_BASE + (reg >> 4), value);
  } else if (lapic_base != NULL) {
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
  if (!apic_initialized)
    return 0;
  if (x2apic_mode) {
    return (uint32_t)rdmsr(X2APIC_ID);
  } else {
    return lapic_read(LAPIC_ID) >> 24;
  }
}

/**
 * Send End of Interrupt - safe to call even if APIC not initialized
 */
void apic_eoi(void) {
  if (!apic_initialized)
    return;
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
  if (!apic_initialized)
    return;
  if (x2apic_mode) {
    /* x2APIC: single 64-bit write to ICR */
    uint64_t icr = ((uint64_t)apic_id << 32) | vector;
    wrmsr(X2APIC_ICR, icr);
  } else if (lapic_base != NULL) {
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
  if (!apic_initialized)
    return;

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
    apic_initialized = true;
    printk("  x2APIC mode enabled\n");
  } else {
    /* xAPIC mode requires memory mapping at physical 0xFEE00000 */
    /* We don't have that mapped in our initial page tables */
    printk("  xAPIC mode - skipping (LAPIC not mapped)\n");
    printk("  NOTE: Full APIC support requires page table mapping\n");
    x2apic_mode = false;
    apic_initialized = false;
    return;
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
