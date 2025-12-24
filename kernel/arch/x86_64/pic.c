/* PIC (8259 Programmable Interrupt Controller) initialization */

#include <stdint.h>

/* I/O port operations */
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

static inline void io_wait(void) {
    outb(0x80, 0);  /* Write to unused port for small delay */
}

/* PIC ports */
#define PIC1_COMMAND    0x20
#define PIC1_DATA       0x21
#define PIC2_COMMAND    0xA0
#define PIC2_DATA       0xA1

/* ICW (Initialization Command Words) */
#define ICW1_INIT       0x10
#define ICW1_ICW4       0x01
#define ICW4_8086       0x01

/* Remap PICs: IRQ 0-7 -> INT 32-39, IRQ 8-15 -> INT 40-47 */
void pic_init(void) {
    /* Save masks */
    uint8_t mask1 = inb(PIC1_DATA);
    uint8_t mask2 = inb(PIC2_DATA);
    
    /* Start initialization sequence (cascade mode) */
    outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
    io_wait();
    outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
    io_wait();
    
    /* Set vector offsets */
    outb(PIC1_DATA, 32);      /* Master PIC: IRQ 0-7 -> INT 32-39 */
    io_wait();
    outb(PIC2_DATA, 40);      /* Slave PIC: IRQ 8-15 -> INT 40-47 */
    io_wait();
    
    /* Tell Master about Slave at IRQ2 */
    outb(PIC1_DATA, 4);       /* Slave on IRQ2 (bit 2) */
    io_wait();
    outb(PIC2_DATA, 2);       /* Slave cascade identity */
    io_wait();
    
    /* Set 8086 mode */
    outb(PIC1_DATA, ICW4_8086);
    io_wait();
    outb(PIC2_DATA, ICW4_8086);
    io_wait();
    
    /* Restore masks (or set to 0 to enable all) */
    outb(PIC1_DATA, mask1);
    outb(PIC2_DATA, mask2);
}

/* Enable specific IRQ */
void pic_enable_irq(uint8_t irq) {
    uint16_t port;
    uint8_t mask;
    
    if (irq < 8) {
        port = PIC1_DATA;
    } else {
        port = PIC2_DATA;
        irq -= 8;
    }
    
    mask = inb(port) & ~(1 << irq);
    outb(port, mask);
}

/* Disable specific IRQ */
void pic_disable_irq(uint8_t irq) {
    uint16_t port;
    uint8_t mask;
    
    if (irq < 8) {
        port = PIC1_DATA;
    } else {
        port = PIC2_DATA;
        irq -= 8;
    }
    
    mask = inb(port) | (1 << irq);
    outb(port, mask);
}

/* Disable all IRQs */
void pic_disable_all(void) {
    outb(PIC1_DATA, 0xFF);
    outb(PIC2_DATA, 0xFF);
}

/* Enable all IRQs */
void pic_enable_all(void) {
    outb(PIC1_DATA, 0x00);
    outb(PIC2_DATA, 0x00);
}
