/* PS/2 Keyboard Driver */

#include <stdint.h>
#include "keyboard.h"
#include "idt.h"

/* Keyboard I/O ports */
#define KBD_DATA    0x60
#define KBD_STATUS  0x64

/* Keyboard buffer */
#define KBD_BUFFER_SIZE 256
static char kbd_buffer[KBD_BUFFER_SIZE];
static volatile int kbd_read = 0;
static volatile int kbd_write = 0;

/* Modifier keys state */
static int shift_pressed = 0;
static int ctrl_pressed = 0;
static int alt_pressed = 0;
static int caps_lock = 0;

/* I/O port operations */
static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

/* US QWERTY scancode to ASCII (lowercase) */
static const char scancode_to_ascii[] = {
    0, 27, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0, 'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    0, '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', 0,
    '*', 0, ' ', 0
};

/* Shifted versions */
static const char scancode_to_ascii_shift[] = {
    0, 27, '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', '\b',
    '\t', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', '\n',
    0, 'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~',
    0, '|', 'Z', 'X', 'C', 'V', 'B', 'N', 'M', '<', '>', '?', 0,
    '*', 0, ' ', 0
};

/* Add character to buffer */
static void kbd_buffer_put(char c) {
    int next = (kbd_write + 1) % KBD_BUFFER_SIZE;
    if (next != kbd_read) {
        kbd_buffer[kbd_write] = c;
        kbd_write = next;
    }
}

/* Keyboard IRQ handler */
static void keyboard_callback(uint64_t int_num, uint64_t error_code) {
    (void)int_num;
    (void)error_code;
    
    uint8_t scancode = inb(KBD_DATA);
    
    /* Key release (bit 7 set) */
    if (scancode & 0x80) {
        scancode &= 0x7F;  /* Remove release bit */
        
        /* Check for modifier release */
        if (scancode == 0x2A || scancode == 0x36) shift_pressed = 0;
        if (scancode == 0x1D) ctrl_pressed = 0;
        if (scancode == 0x38) alt_pressed = 0;
        return;
    }
    
    /* Key press */
    
    /* Check for modifier keys */
    if (scancode == 0x2A || scancode == 0x36) { shift_pressed = 1; return; }
    if (scancode == 0x1D) { ctrl_pressed = 1; return; }
    if (scancode == 0x38) { alt_pressed = 1; return; }
    if (scancode == 0x3A) { caps_lock = !caps_lock; return; }  /* Caps Lock toggle */
    
    /* Convert scancode to ASCII */
    if (scancode < sizeof(scancode_to_ascii)) {
        char c;
        
        int use_shift = shift_pressed;
        
        /* Caps lock affects letters only */
        if (caps_lock && scancode_to_ascii[scancode] >= 'a' && scancode_to_ascii[scancode] <= 'z') {
            use_shift = !use_shift;
        }
        
        if (use_shift) {
            c = scancode_to_ascii_shift[scancode];
        } else {
            c = scancode_to_ascii[scancode];
        }
        
        /* Handle Ctrl+key */
        if (ctrl_pressed && c >= 'a' && c <= 'z') {
            c = c - 'a' + 1;  /* Ctrl+A = 1, Ctrl+B = 2, etc. */
        }
        
        if (c != 0) {
            kbd_buffer_put(c);
        }
    }
}

void keyboard_init(void) {
    /* Register keyboard handler (IRQ1 = INT 33) */
    register_interrupt_handler(33, keyboard_callback);
}

int keyboard_available(void) {
    return kbd_read != kbd_write;
}

char keyboard_getchar(void) {
    while (kbd_read == kbd_write) {
        __asm__ volatile ("hlt");
    }
    char c = kbd_buffer[kbd_read];
    kbd_read = (kbd_read + 1) % KBD_BUFFER_SIZE;
    return c;
}

char keyboard_try_getchar(void) {
    if (kbd_read == kbd_write) return 0;
    char c = kbd_buffer[kbd_read];
    kbd_read = (kbd_read + 1) % KBD_BUFFER_SIZE;
    return c;
}
