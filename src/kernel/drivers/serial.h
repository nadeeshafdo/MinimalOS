#ifndef SERIAL_H
#define SERIAL_H

#include "../include/types.h"

#define COM1 0x3F8

void serial_init(void);
void serial_putc(char c);
void serial_puts(const char* str);

#endif // SERIAL_H
