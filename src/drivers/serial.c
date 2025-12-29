/**
 * MinimalOS - Serial Port Driver (COM1)
 * Used for early debug output
 */

#include <minimalos/types.h>

/* COM1 port addresses */
#define COM1_PORT 0x3F8

#define SERIAL_DATA 0    /* Data register (R/W) */
#define SERIAL_IER 1     /* Interrupt Enable Register */
#define SERIAL_FIFO 2    /* FIFO Control Register */
#define SERIAL_LCR 3     /* Line Control Register */
#define SERIAL_MCR 4     /* Modem Control Register */
#define SERIAL_LSR 5     /* Line Status Register */
#define SERIAL_MSR 6     /* Modem Status Register */
#define SERIAL_SCRATCH 7 /* Scratch Register */

/* Line Status Register bits */
#define LSR_DATA_READY 0x01
#define LSR_OVERRUN_ERR 0x02
#define LSR_PARITY_ERR 0x04
#define LSR_FRAMING_ERR 0x08
#define LSR_BREAK_INT 0x10
#define LSR_TX_EMPTY 0x20
#define LSR_TX_IDLE 0x40
#define LSR_RX_FIFO_ERR 0x80

/* Port I/O functions */
static inline void outb(uint16_t port, uint8_t value) {
  __asm__ volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
  uint8_t value;
  __asm__ volatile("inb %1, %0" : "=a"(value) : "Nd"(port));
  return value;
}

/**
 * Initialize serial port (COM1) for 115200 baud, 8N1
 */
void serial_init(void) {
  /* Disable interrupts */
  outb(COM1_PORT + SERIAL_IER, 0x00);

  /* Enable DLAB (Divisor Latch Access Bit) to set baud rate */
  outb(COM1_PORT + SERIAL_LCR, 0x80);

  /* Set divisor to 1 (115200 baud) */
  /*   Divisor = 115200 / baud_rate */
  /*   For 115200 baud: divisor = 1 */
  outb(COM1_PORT + SERIAL_DATA, 0x01); /* Low byte */
  outb(COM1_PORT + SERIAL_IER, 0x00);  /* High byte */

  /* 8 bits, no parity, 1 stop bit (8N1) */
  outb(COM1_PORT + SERIAL_LCR, 0x03);

  /* Enable FIFO, clear them, 14-byte threshold */
  outb(COM1_PORT + SERIAL_FIFO, 0xC7);

  /* IRQs disabled, RTS/DSR set */
  outb(COM1_PORT + SERIAL_MCR, 0x0B);

  /* Set in loopback mode to test the serial chip */
  outb(COM1_PORT + SERIAL_MCR, 0x1E);

  /* Test serial chip (send byte 0xAE and check if same byte comes back) */
  outb(COM1_PORT + SERIAL_DATA, 0xAE);

  /* Check if we received the test byte back */
  if (inb(COM1_PORT + SERIAL_DATA) != 0xAE) {
    /* Serial port not working, but continue anyway */
    return;
  }

  /* Serial chip works, set normal operation mode */
  /* (not loopback, IRQs disabled, OUT1/OUT2 enabled) */
  outb(COM1_PORT + SERIAL_MCR, 0x0F);
}

/**
 * Check if transmit buffer is empty
 */
static bool serial_tx_empty(void) {
  return (inb(COM1_PORT + SERIAL_LSR) & LSR_TX_EMPTY) != 0;
}

/**
 * Send a character over serial
 */
void serial_putchar(char c) {
  /* Wait for transmit buffer to be empty */
  while (!serial_tx_empty()) {
    __asm__ volatile("pause");
  }

  outb(COM1_PORT + SERIAL_DATA, c);
}

/**
 * Send a string over serial
 */
void serial_puts(const char *str) {
  while (*str) {
    if (*str == '\n') {
      serial_putchar('\r');
    }
    serial_putchar(*str++);
  }
}

/**
 * Check if data is available to read
 */
bool serial_data_ready(void) {
  return (inb(COM1_PORT + SERIAL_LSR) & LSR_DATA_READY) != 0;
}

/**
 * Read a character from serial (blocking)
 */
char serial_getchar(void) {
  while (!serial_data_ready()) {
    __asm__ volatile("pause");
  }
  return inb(COM1_PORT + SERIAL_DATA);
}
