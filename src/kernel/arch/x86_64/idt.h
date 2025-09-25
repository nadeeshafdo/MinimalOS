#ifndef IDT_H
#define IDT_H

#include "../../stdint.h"

void setup_idt(void);
void set_idt_entry(int num, uint64_t base, uint16_t sel, uint8_t flags);

#endif