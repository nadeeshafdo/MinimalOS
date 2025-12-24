#ifndef _PIC_H
#define _PIC_H

#include <stdint.h>

/* Initialize and remap PIC */
void pic_init(void);

/* Enable/disable specific IRQ */
void pic_enable_irq(uint8_t irq);
void pic_disable_irq(uint8_t irq);

/* Enable/disable all IRQs */
void pic_enable_all(void);
void pic_disable_all(void);

#endif /* _PIC_H */
