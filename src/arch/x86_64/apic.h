/**
 * MinimalOS - APIC Header (Local APIC / x2APIC)
 */

#ifndef ARCH_X86_64_APIC_H
#define ARCH_X86_64_APIC_H

#include <minimalos/types.h>

/* LAPIC register offsets (for memory-mapped xAPIC mode) */
#define LAPIC_ID 0x020          /* Local APIC ID */
#define LAPIC_VER 0x030         /* Local APIC Version */
#define LAPIC_TPR 0x080         /* Task Priority Register */
#define LAPIC_APR 0x090         /* Arbitration Priority Register */
#define LAPIC_PPR 0x0A0         /* Processor Priority Register */
#define LAPIC_EOI 0x0B0         /* End of Interrupt */
#define LAPIC_RRD 0x0C0         /* Remote Read Register */
#define LAPIC_LDR 0x0D0         /* Logical Destination Register */
#define LAPIC_DFR 0x0E0         /* Destination Format Register */
#define LAPIC_SVR 0x0F0         /* Spurious Interrupt Vector Register */
#define LAPIC_ISR 0x100         /* In-Service Register (8 x 32-bit) */
#define LAPIC_TMR 0x180         /* Trigger Mode Register (8 x 32-bit) */
#define LAPIC_IRR 0x200         /* Interrupt Request Register (8 x 32-bit) */
#define LAPIC_ESR 0x280         /* Error Status Register */
#define LAPIC_ICR_LO 0x300      /* Interrupt Command Register Low */
#define LAPIC_ICR_HI 0x310      /* Interrupt Command Register High */
#define LAPIC_LVT_TIMER 0x320   /* LVT Timer Register */
#define LAPIC_LVT_THERMAL 0x330 /* LVT Thermal Sensor Register */
#define LAPIC_LVT_PERF 0x340    /* LVT Performance Counter Register */
#define LAPIC_LVT_LINT0 0x350   /* LVT LINT0 Register */
#define LAPIC_LVT_LINT1 0x360   /* LVT LINT1 Register */
#define LAPIC_LVT_ERROR 0x370   /* LVT Error Register */
#define LAPIC_TIMER_ICR 0x380   /* Timer Initial Count Register */
#define LAPIC_TIMER_CCR 0x390   /* Timer Current Count Register */
#define LAPIC_TIMER_DCR 0x3E0   /* Timer Divide Configuration Register */

/* x2APIC MSRs (base = 0x800) */
#define X2APIC_MSR_BASE 0x800
#define X2APIC_ID (X2APIC_MSR_BASE + 0x02)
#define X2APIC_VER (X2APIC_MSR_BASE + 0x03)
#define X2APIC_TPR (X2APIC_MSR_BASE + 0x08)
#define X2APIC_PPR (X2APIC_MSR_BASE + 0x0A)
#define X2APIC_EOI (X2APIC_MSR_BASE + 0x0B)
#define X2APIC_LDR (X2APIC_MSR_BASE + 0x0D)
#define X2APIC_SVR (X2APIC_MSR_BASE + 0x0F)
#define X2APIC_ISR0 (X2APIC_MSR_BASE + 0x10)
#define X2APIC_TMR0 (X2APIC_MSR_BASE + 0x18)
#define X2APIC_IRR0 (X2APIC_MSR_BASE + 0x20)
#define X2APIC_ESR (X2APIC_MSR_BASE + 0x28)
#define X2APIC_ICR (X2APIC_MSR_BASE + 0x30)
#define X2APIC_LVT_TIMER (X2APIC_MSR_BASE + 0x32)
#define X2APIC_LVT_THERMAL (X2APIC_MSR_BASE + 0x33)
#define X2APIC_LVT_PERF (X2APIC_MSR_BASE + 0x34)
#define X2APIC_LVT_LINT0 (X2APIC_MSR_BASE + 0x35)
#define X2APIC_LVT_LINT1 (X2APIC_MSR_BASE + 0x36)
#define X2APIC_LVT_ERROR (X2APIC_MSR_BASE + 0x37)
#define X2APIC_TIMER_ICR (X2APIC_MSR_BASE + 0x38)
#define X2APIC_TIMER_CCR (X2APIC_MSR_BASE + 0x39)
#define X2APIC_TIMER_DCR (X2APIC_MSR_BASE + 0x3E)
#define X2APIC_SELF_IPI (X2APIC_MSR_BASE + 0x3F)

/* SVR flags */
#define SVR_ENABLE (1 << 8)
#define SVR_FOCUS_DISABLED (1 << 9)

/* LVT flags */
#define LVT_MASKED (1 << 16)
#define LVT_TRIGGER_LEVEL (1 << 15)
#define LVT_TRIGGER_EDGE (0 << 15)
#define LVT_DELIVERY_FIXED (0 << 8)
#define LVT_DELIVERY_NMI (4 << 8)
#define LVT_DELIVERY_EXTINT (7 << 8)

/* Timer modes */
#define TIMER_ONESHOT (0 << 17)
#define TIMER_PERIODIC (1 << 17)
#define TIMER_TSC_DEADLINE (2 << 17)

/* Timer divider values */
#define TIMER_DIV_1 0xB
#define TIMER_DIV_2 0x0
#define TIMER_DIV_4 0x1
#define TIMER_DIV_8 0x2
#define TIMER_DIV_16 0x3
#define TIMER_DIV_32 0x8
#define TIMER_DIV_64 0x9
#define TIMER_DIV_128 0xA

/* ICR delivery modes */
#define ICR_FIXED (0 << 8)
#define ICR_LOWEST (1 << 8)
#define ICR_SMI (2 << 8)
#define ICR_NMI (4 << 8)
#define ICR_INIT (5 << 8)
#define ICR_STARTUP (6 << 8)

/* ICR destination modes */
#define ICR_PHYSICAL (0 << 11)
#define ICR_LOGICAL (1 << 11)

/* ICR level/trigger */
#define ICR_DEASSERT (0 << 14)
#define ICR_ASSERT (1 << 14)
#define ICR_EDGE (0 << 15)
#define ICR_LEVEL (1 << 15)

/* ICR destination shorthand */
#define ICR_NO_SHORTHAND (0 << 18)
#define ICR_SELF (1 << 18)
#define ICR_ALL_INCL (2 << 18)
#define ICR_ALL_EXCL (3 << 18)

/* Function prototypes */
void apic_init(void);
void apic_eoi(void);
uint32_t apic_get_id(void);
void apic_send_ipi(uint32_t apic_id, uint32_t vector);
void apic_timer_init(uint32_t frequency_hz);
bool apic_is_x2apic(void);

#endif /* ARCH_X86_64_APIC_H */
