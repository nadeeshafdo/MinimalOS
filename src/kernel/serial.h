#ifndef SERIAL_H
#define SERIAL_H

#include "stdint.h"

// COM1 serial port base address
#define COM1 0x3F8

// Initialize serial port
void serial_init(void);

// Write a character to serial port
void serial_putchar(char c);

// Write a string to serial port
void serial_print(const char* str);

// Check if data is available to read
int serial_available(void);

// Read a character from serial port
char serial_getchar(void);

#endif
