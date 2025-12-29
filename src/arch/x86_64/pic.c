/**
 * MinimalOS - 8259 PIC Implementation
 */

#include "pic.h"

extern void printk(const char *fmt, ...);

/* I/O port access */
static inline void outb(uint16_t port, uint8_t val) {
  __asm__ volatile("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
  uint8_t ret;
  __asm__ volatile("inb %1, %0" : "=a"(ret) : "Nd"(port));
  return ret;
}

static inline void io_wait(void) {
  /* Wait by writing to an unused port */
  outb(0x80, 0);
}

/**
 * Initialize the 8259 PIC
 */
void pic_init(void) {
  /* Start initialization sequence (cascade mode) */
  outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
  io_wait();
  outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
  io_wait();

  /* ICW2: Set vector offsets */
  outb(PIC1_DATA, PIC1_OFFSET); /* Master: IRQ 0-7 -> vectors 32-39 */
  io_wait();
  outb(PIC2_DATA, PIC2_OFFSET); /* Slave: IRQ 8-15 -> vectors 40-47 */
  io_wait();

  /* ICW3: Tell Master about Slave on IRQ2 */
  outb(PIC1_DATA, 0x04); /* Slave on IRQ2 */
  io_wait();
  outb(PIC2_DATA, 0x02); /* Slave cascade identity */
  io_wait();

  /* ICW4: 8086 mode */
  outb(PIC1_DATA, ICW4_8086);
  io_wait();
  outb(PIC2_DATA, ICW4_8086);
  io_wait();

  /* Mask all interrupts initially except cascade */
  outb(PIC1_DATA, 0xFB); /* Mask all except IRQ2 (cascade) */
  outb(PIC2_DATA, 0xFF); /* Mask all on slave */

  printk(
      "  PIC remapped: IRQ 0-7 -> vectors %u-%u, IRQ 8-15 -> vectors %u-%u\n",
      PIC1_OFFSET, PIC1_OFFSET + 7, PIC2_OFFSET, PIC2_OFFSET + 7);
}

/**
 * Send End of Interrupt
 */
void pic_eoi(uint8_t irq) {
  if (irq >= 8) {
    /* IRQ came from slave PIC */
    outb(PIC2_COMMAND, PIC_EOI);
  }
  /* Always send EOI to master */
  outb(PIC1_COMMAND, PIC_EOI);
}

/**
 * Mask (disable) an IRQ
 */
void pic_mask_irq(uint8_t irq) {
  uint16_t port;
  uint8_t value;

  if (irq < 8) {
    port = PIC1_DATA;
  } else {
    port = PIC2_DATA;
    irq -= 8;
  }

  value = inb(port) | (1 << irq);
  outb(port, value);
}

/**
 * Unmask (enable) an IRQ
 */
void pic_unmask_irq(uint8_t irq) {
  uint16_t port;
  uint8_t value;

  if (irq < 8) {
    port = PIC1_DATA;
  } else {
    port = PIC2_DATA;
    irq -= 8;
  }

  value = inb(port) & ~(1 << irq);
  outb(port, value);
}

/**
 * Disable the PIC (mask all IRQs)
 */
void pic_disable(void) {
  outb(PIC1_DATA, 0xFF);
  outb(PIC2_DATA, 0xFF);
}

/**
 * Read ISR (In-Service Register)
 */
uint16_t pic_get_isr(void) {
  outb(PIC1_COMMAND, 0x0B);
  outb(PIC2_COMMAND, 0x0B);
  return (inb(PIC2_COMMAND) << 8) | inb(PIC1_COMMAND);
}

/**
 * Read IRR (Interrupt Request Register)
 */
uint16_t pic_get_irr(void) {
  outb(PIC1_COMMAND, 0x0A);
  outb(PIC2_COMMAND, 0x0A);
  return (inb(PIC2_COMMAND) << 8) | inb(PIC1_COMMAND);
}
