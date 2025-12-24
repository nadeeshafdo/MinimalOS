/* Serial Port Driver (COM1) for debug output */

#include <stdint.h>
#include "serial.h"

/* I/O port access */
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

/* Serial port registers (offsets from base) */
#define SERIAL_DATA         0   /* Data register (R/W) */
#define SERIAL_INT_ENABLE   1   /* Interrupt enable */
#define SERIAL_FIFO_CTRL    2   /* FIFO control */
#define SERIAL_LINE_CTRL    3   /* Line control */
#define SERIAL_MODEM_CTRL   4   /* Modem control */
#define SERIAL_LINE_STATUS  5   /* Line status */

/* Line status bits */
#define SERIAL_TRANSMIT_EMPTY 0x20

static uint16_t serial_port = COM1;

void serial_init(void) {
    /* Disable interrupts */
    outb(serial_port + SERIAL_INT_ENABLE, 0x00);
    
    /* Enable DLAB (set baud rate divisor) */
    outb(serial_port + SERIAL_LINE_CTRL, 0x80);
    
    /* Set divisor to 1 (115200 baud) */
    outb(serial_port + SERIAL_DATA, 0x01);      /* Low byte */
    outb(serial_port + SERIAL_INT_ENABLE, 0x00); /* High byte */
    
    /* 8 bits, no parity, one stop bit */
    outb(serial_port + SERIAL_LINE_CTRL, 0x03);
    
    /* Enable FIFO, clear them, with 14-byte threshold */
    outb(serial_port + SERIAL_FIFO_CTRL, 0xC7);
    
    /* Enable IRQs, RTS/DSR set */
    outb(serial_port + SERIAL_MODEM_CTRL, 0x0B);
    
    /* Send init message */
    serial_puts("\n[SERIAL] MinimalOS debug output initialized\n");
}

static int is_transmit_empty(void) {
    return inb(serial_port + SERIAL_LINE_STATUS) & SERIAL_TRANSMIT_EMPTY;
}

void serial_putchar(char c) {
    while (!is_transmit_empty());
    outb(serial_port + SERIAL_DATA, c);
}

void serial_puts(const char *s) {
    while (*s) {
        if (*s == '\n') serial_putchar('\r');
        serial_putchar(*s++);
    }
}

void serial_puthex(uint64_t n) {
    const char *hex = "0123456789ABCDEF";
    serial_puts("0x");
    int started = 0;
    for (int i = 60; i >= 0; i -= 4) {
        int d = (n >> i) & 0xF;
        if (d || started || i == 0) {
            serial_putchar(hex[d]);
            started = 1;
        }
    }
}

void serial_putdec(uint64_t n) {
    if (n == 0) {
        serial_putchar('0');
        return;
    }
    char buf[21];
    int i = 0;
    while (n) {
        buf[i++] = '0' + (n % 10);
        n /= 10;
    }
    while (i--) serial_putchar(buf[i]);
}

void serial_debug(const char *msg) {
    serial_puts("[DEBUG] ");
    serial_puts(msg);
    serial_puts("\n");
}
