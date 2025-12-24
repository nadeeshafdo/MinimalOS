#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <kernel/tty.h>

/* VGA text mode buffer */
#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VGA_MEMORY 0xB8000

/* VGA CRTC ports for cursor control */
#define VGA_CRTC_INDEX 0x3D4
#define VGA_CRTC_DATA  0x3D5

static uint16_t* const VGA_BUFFER = (uint16_t*) VGA_MEMORY;
static size_t terminal_row;
static size_t terminal_column;
static uint8_t terminal_color;

/* Simple spinlock for terminal thread safety */
static volatile int terminal_lock = 0;

static inline void terminal_acquire(void) {
    while (__sync_lock_test_and_set(&terminal_lock, 1)) {
        /* Spin */
    }
}

static inline void terminal_release(void) {
    __sync_lock_release(&terminal_lock);
}

/* I/O port operations */
static inline void outb(uint16_t port, uint8_t value) {
    __asm__ volatile ("outb %0, %1" : : "a"(value), "Nd"(port));
}

/* Helper to create VGA entry (character + color) */
static inline uint16_t vga_entry(unsigned char uc, uint8_t color) {
    return (uint16_t) uc | (uint16_t) color << 8;
}

/* Simple strlen implementation */
static size_t strlen(const char* str) {
    size_t len = 0;
    while (str[len])
        len++;
    return len;
}

/* Update hardware cursor position */
static void update_cursor(void) {
    uint16_t pos = terminal_row * VGA_WIDTH + terminal_column;
    
    outb(VGA_CRTC_INDEX, 0x0F);  /* Cursor location low register */
    outb(VGA_CRTC_DATA, (uint8_t)(pos & 0xFF));
    outb(VGA_CRTC_INDEX, 0x0E);  /* Cursor location high register */
    outb(VGA_CRTC_DATA, (uint8_t)((pos >> 8) & 0xFF));
}

/* Enable the hardware cursor */
static void enable_cursor(uint8_t cursor_start, uint8_t cursor_end) {
    outb(VGA_CRTC_INDEX, 0x0A);  /* Cursor start register */
    outb(VGA_CRTC_DATA, cursor_start);
    outb(VGA_CRTC_INDEX, 0x0B);  /* Cursor end register */
    outb(VGA_CRTC_DATA, cursor_end);
}

void terminal_initialize(void) {
    terminal_row = 0;
    terminal_column = 0;
    terminal_color = vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK);
    
    /* Clear screen */
    for (size_t y = 0; y < VGA_HEIGHT; y++) {
        for (size_t x = 0; x < VGA_WIDTH; x++) {
            const size_t index = y * VGA_WIDTH + x;
            VGA_BUFFER[index] = vga_entry(' ', terminal_color);
        }
    }
    
    /* Enable and position cursor (underline style: scanlines 13-14) */
    enable_cursor(13, 14);
    update_cursor();
}

void terminal_setcolor(uint8_t color) {
    terminal_color = color;
}

/* Scroll screen up by one line */
static void terminal_scroll(void) {
    /* Move all lines up */
    for (size_t y = 0; y < VGA_HEIGHT - 1; y++) {
        for (size_t x = 0; x < VGA_WIDTH; x++) {
            const size_t dst_index = y * VGA_WIDTH + x;
            const size_t src_index = (y + 1) * VGA_WIDTH + x;
            VGA_BUFFER[dst_index] = VGA_BUFFER[src_index];
        }
    }
    
    /* Clear bottom line */
    for (size_t x = 0; x < VGA_WIDTH; x++) {
        const size_t index = (VGA_HEIGHT - 1) * VGA_WIDTH + x;
        VGA_BUFFER[index] = vga_entry(' ', terminal_color);
    }
    
    /* Keep cursor on last line */
    terminal_row = VGA_HEIGHT - 1;
}

void terminal_putentryat(char c, uint8_t color, size_t x, size_t y) {
    const size_t index = y * VGA_WIDTH + x;
    VGA_BUFFER[index] = vga_entry(c, color);
}

void terminal_putchar(char c) {
    terminal_acquire();
    
    /* Handle newline */
    if (c == '\n') {
        terminal_column = 0;
        if (++terminal_row == VGA_HEIGHT) {
            terminal_scroll();
        }
        update_cursor();
        terminal_release();
        return;
    }
    
    /* Handle carriage return */
    if (c == '\r') {
        terminal_column = 0;
        update_cursor();
        terminal_release();
        return;
    }
    
    /* Handle backspace */
    if (c == '\b') {
        if (terminal_column > 0) {
            terminal_column--;
            terminal_putentryat(' ', terminal_color, terminal_column, terminal_row);
            update_cursor();
        }
        terminal_release();
        return;
    }
    
    /* Handle tab (4 spaces) */
    if (c == '\t') {
        terminal_column = (terminal_column + 4) & ~(4 - 1);
        if (terminal_column >= VGA_WIDTH) {
            terminal_column = 0;
            if (++terminal_row == VGA_HEIGHT) {
                terminal_scroll();
            }
        }
        update_cursor();
        terminal_release();
        return;
    }
    
    /* Print regular character */
    terminal_putentryat(c, terminal_color, terminal_column, terminal_row);
    
    if (++terminal_column == VGA_WIDTH) {
        terminal_column = 0;
        if (++terminal_row == VGA_HEIGHT) {
            terminal_scroll();
        }
    }
    
    update_cursor();
    terminal_release();
}

void terminal_write(const char* data, size_t size) {
    for (size_t i = 0; i < size; i++)
        terminal_putchar(data[i]);
}

void terminal_writestring(const char* data) {
    terminal_write(data, strlen(data));
}

void terminal_clear(void) {
    terminal_row = 0;
    terminal_column = 0;
    
    for (size_t y = 0; y < VGA_HEIGHT; y++) {
        for (size_t x = 0; x < VGA_WIDTH; x++) {
            const size_t index = y * VGA_WIDTH + x;
            VGA_BUFFER[index] = vga_entry(' ', terminal_color);
        }
    }
    
    update_cursor();
}
