#include "serial.h"

// Port offsets
#define SERIAL_DATA(base)          (base)
#define SERIAL_FIFO_CMD(base)      (base + 2)
#define SERIAL_LINE_CMD(base)      (base + 3)
#define SERIAL_MODEM_CMD(base)     (base + 4)
#define SERIAL_LINE_STATUS(base)   (base + 5)

// Port I/O functions
static inline void outb(uint16_t port, uint8_t value) {
    asm volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t value;
    asm volatile("inb %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

void serial_init(void) {
    // Disable interrupts
    outb(COM1 + 1, 0x00);
    
    // Enable DLAB (set baud rate divisor)
    outb(SERIAL_LINE_CMD(COM1), 0x80);
    
    // Set divisor to 3 (38400 baud)
    outb(SERIAL_DATA(COM1), 0x03);
    outb(COM1 + 1, 0x00);
    
    // 8 bits, no parity, one stop bit
    outb(SERIAL_LINE_CMD(COM1), 0x03);
    
    // Enable FIFO, clear them, with 14-byte threshold
    outb(SERIAL_FIFO_CMD(COM1), 0xC7);
    
    // IRQs enabled, RTS/DSR set
    outb(SERIAL_MODEM_CMD(COM1), 0x0B);
    
    // Set in loopback mode, test the serial chip
    outb(SERIAL_MODEM_CMD(COM1), 0x1E);
    
    // Test serial chip (send byte 0xAE and check if serial returns same byte)
    outb(SERIAL_DATA(COM1), 0xAE);
    
    // Check if serial is faulty (i.e: not same byte as sent)
    if (inb(SERIAL_DATA(COM1)) != 0xAE) {
        return; // Serial is faulty
    }
    
    // If serial is not faulty set it in normal operation mode
    outb(SERIAL_MODEM_CMD(COM1), 0x0F);
}

static int is_transmit_empty(void) {
    return inb(SERIAL_LINE_STATUS(COM1)) & 0x20;
}

void serial_putchar(char c) {
    while (is_transmit_empty() == 0);
    outb(SERIAL_DATA(COM1), c);
}

void serial_print(const char* str) {
    while (*str) {
        serial_putchar(*str++);
    }
}

int serial_available(void) {
    return inb(SERIAL_LINE_STATUS(COM1)) & 1;
}

char serial_getchar(void) {
    while (serial_available() == 0);
    return inb(SERIAL_DATA(COM1));
}
