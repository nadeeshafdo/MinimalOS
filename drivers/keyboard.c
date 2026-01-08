#include <stdint.h>
#include <kernel/keyboard.h>
#include <kernel/irq.h>
#include <kernel/tty.h>
#include <kernel/shell.h>

/* I/O port operations */
static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

/* Keyboard data and status ports */
#define KEYBOARD_DATA 0x60
#define KEYBOARD_STATUS 0x64

/* Keyboard buffer */
#define BUFFER_SIZE 256
static char keyboard_buffer[BUFFER_SIZE];
static uint32_t buffer_start = 0;
static uint32_t buffer_end = 0;

/* US QWERTY scancode to ASCII table */
static const char scancode_to_ascii[] = {
    0, 0, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0, 'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    0, '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', 0,
    '*', 0, ' '
};

/* Shifted characters */
static const char scancode_to_ascii_shift[] = {
    0, 0, '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', '\b',
    '\t', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', '\n',
    0, 'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~',
    0, '|', 'Z', 'X', 'C', 'V', 'B', 'N', 'M', '<', '>', '?', 0,
    '*', 0, ' '
};

static uint8_t shift_pressed = 0;

/* Add character to buffer */
static void buffer_add(char c) {
    uint32_t next = (buffer_end + 1) % BUFFER_SIZE;
    if (next != buffer_start) {
        keyboard_buffer[buffer_end] = c;
        buffer_end = next;
    }
}

/* Keyboard interrupt handler */
static void keyboard_handler(struct registers* regs) {
    (void)regs;  /* Unused */
    
    uint8_t scancode = inb(KEYBOARD_DATA);
    
   /* Handle key release (high bit set) */
    if (scancode & 0x80) {
        scancode &= 0x7F;
        /* Check for shift release */
        if (scancode == 0x2A || scancode == 0x36) {
            shift_pressed = 0;
        }
    } else {
        /* Check for shift press */
        if (scancode == 0x2A || scancode == 0x36) {
            shift_pressed = 1;
        } else if (scancode < sizeof(scancode_to_ascii)) {
            char c;
            if (shift_pressed) {
                c = scancode_to_ascii_shift[scancode];
            } else {
                c = scancode_to_ascii[scancode];
            }
            
            if (c) {
                buffer_add(c);
                /* Send to shell instead of direct terminal output */
                shell_input(c);
            }
        }
    }
}

char keyboard_getchar(void) {
    if (buffer_start == buffer_end) {
        return 0;
    }
    
    char c = keyboard_buffer[buffer_start];
    buffer_start = (buffer_start + 1) % BUFFER_SIZE;
    return c;
}

void keyboard_init(void) {
    /* Register keyboard interrupt handler (IRQ1) */
    irq_register_handler(1, keyboard_handler);
}
