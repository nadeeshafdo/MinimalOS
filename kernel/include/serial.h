#ifndef _SERIAL_H
#define _SERIAL_H

#include <stdint.h>

/* COM port addresses */
#define COM1 0x3F8
#define COM2 0x2F8

/* Initialize serial port (default COM1, 115200 baud) */
void serial_init(void);

/* Write a character to serial */
void serial_putchar(char c);

/* Write a string to serial */
void serial_puts(const char *s);

/* Write a hex number to serial */
void serial_puthex(uint64_t n);

/* Write a decimal number to serial */
void serial_putdec(uint64_t n);

/* Debug print (prefixed with [DEBUG]) */
void serial_debug(const char *msg);

#endif /* _SERIAL_H */
