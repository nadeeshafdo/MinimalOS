#include "keyboard.h"
#include "vga.h"
#include "../../stdint.h"

// Port I/O functions
static inline void outb(uint16_t port, uint8_t val) {
    asm volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    asm volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

#define KB_BUF_SIZE 256
static char kb_buffer[KB_BUF_SIZE];
static int kb_head = 0, kb_tail = 0;

// Simplified scancode map - just basic keys
static const char scancode_map[128] = {
    0,   27, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0,   'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    0,   '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', 0,
    '*', 0, ' '
};

void keyboard_isr() {
    uint8_t sc = inb(0x60);
    if (sc < 128) {  // Key press (not release)
        char ch = scancode_map[sc];
        if (ch) {
            kb_buffer[kb_head] = ch;
            kb_head = (kb_head + 1) % KB_BUF_SIZE;
        }
    }
    outb(0x20, 0x20);  // Send EOI to PIC
}

void setup_keyboard() {
    // IRQ1 is already set up in IDT
}

char kb_read() {
    while (kb_head == kb_tail) {
        // Wait for input
        asm volatile("hlt");
    }
    char ch = kb_buffer[kb_tail];
    kb_tail = (kb_tail + 1) % KB_BUF_SIZE;
    return ch;
}