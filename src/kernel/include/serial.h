#ifndef _KERNEL_SERIAL_H
#define _KERNEL_SERIAL_H

#include <stdint.h>

void serial_init(void);
void serial_write(char c);
void serial_print(const char* str);

#endif
