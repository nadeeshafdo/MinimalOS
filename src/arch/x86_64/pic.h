/**
 * MinimalOS - 8259 PIC (Programmable Interrupt Controller)
 * Legacy interrupt controller for systems without x2APIC
 */

#ifndef ARCH_X86_64_PIC_H
#define ARCH_X86_64_PIC_H

#include <minimalos/types.h>

/* PIC I/O ports */
#define PIC1_COMMAND 0x20
#define PIC1_DATA 0x21
#define PIC2_COMMAND 0xA0
#define PIC2_DATA 0xA1

/* PIC commands */
#define PIC_EOI 0x20 /* End of Interrupt */

/* ICW1 (Initialization Command Word 1) */
#define ICW1_ICW4 0x01      /* ICW4 needed */
#define ICW1_SINGLE 0x02    /* Single (vs cascade) mode */
#define ICW1_INTERVAL4 0x04 /* Call address interval 4 */
#define ICW1_LEVEL 0x08     /* Level triggered mode */
#define ICW1_INIT 0x10      /* Initialization */

/* ICW4 (Initialization Command Word 4) */
#define ICW4_8086 0x01       /* 8086/88 mode */
#define ICW4_AUTO 0x02       /* Auto EOI */
#define ICW4_BUF_SLAVE 0x08  /* Buffered slave */
#define ICW4_BUF_MASTER 0x0C /* Buffered master */
#define ICW4_SFNM 0x10       /* Special fully nested */

/* IRQ vector offsets (remapped from 0-15 to 32-47) */
#define PIC1_OFFSET 32
#define PIC2_OFFSET 40

/* IRQ numbers */
#define IRQ_TIMER 0
#define IRQ_KEYBOARD 1
#define IRQ_CASCADE 2 /* Cascade to PIC2 */
#define IRQ_COM2 3
#define IRQ_COM1 4
#define IRQ_LPT2 5
#define IRQ_FLOPPY 6
#define IRQ_LPT1 7
#define IRQ_RTC 8
#define IRQ_FREE1 9
#define IRQ_FREE2 10
#define IRQ_FREE3 11
#define IRQ_MOUSE 12
#define IRQ_FPU 13
#define IRQ_ATA_PRIMARY 14
#define IRQ_ATA_SECOND 15

/**
 * Initialize the 8259 PIC
 * Remaps IRQs 0-15 to vectors 32-47
 */
void pic_init(void);

/**
 * Send End of Interrupt to PIC
 * @param irq IRQ number (0-15)
 */
void pic_eoi(uint8_t irq);

/**
 * Mask (disable) an IRQ
 * @param irq IRQ number (0-15)
 */
void pic_mask_irq(uint8_t irq);

/**
 * Unmask (enable) an IRQ
 * @param irq IRQ number (0-15)
 */
void pic_unmask_irq(uint8_t irq);

/**
 * Disable the PIC (for use with APIC)
 * Masks all IRQs
 */
void pic_disable(void);

/**
 * Get the combined ISR (In-Service Register)
 * @return 16-bit value with current IRQs being serviced
 */
uint16_t pic_get_isr(void);

/**
 * Get the combined IRR (Interrupt Request Register)
 * @return 16-bit value with pending IRQs
 */
uint16_t pic_get_irr(void);

#endif /* ARCH_X86_64_PIC_H */
