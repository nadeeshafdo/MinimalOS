#include <io.h>
#include <serial.h>

#define PORT_COM1 0x3F8

void serial_init(void) {
    outb(PORT_COM1 + 1, 0x00);    // Disable all interrupts
    outb(PORT_COM1 + 3, 0x80);    // Enable DLAB (set baud rate divisor)
    outb(PORT_COM1 + 0, 0x03);    // Set divisor to 3 (lo byte) 38400 baud
    outb(PORT_COM1 + 1, 0x00);    //                  (hi byte)
    outb(PORT_COM1 + 3, 0x03);    // 8 bit, no parity, 1 stop bit
    outb(PORT_COM1 + 2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
    outb(PORT_COM1 + 4, 0x0B);    // IRQs enabled, RTS/DSR set
}

int is_transmit_empty(void) {
    return inb(PORT_COM1 + 5) & 0x20;
}

void serial_write(char c) {
    while (is_transmit_empty() == 0);
    outb(PORT_COM1, c);
}

void serial_print(const char* str) {
    while (*str) {
        serial_write(*str++);
    }
}
