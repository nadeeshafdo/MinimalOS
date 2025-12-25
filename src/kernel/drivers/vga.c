#include "vga.h"

#define VGA_WIDTH  80
#define VGA_HEIGHT 25
#define VGA_MEMORY 0xB8000

static u16* vga_buffer = (u16*)VGA_MEMORY;
static u32 vga_row = 0;
static u32 vga_column = 0;
static u8 vga_color = 0;

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline u8 make_color(u8 fg, u8 bg) {
    return fg | (bg << 4);
}

static inline u16 make_vga_entry(char c, u8 color) {
    return (u16)c | ((u16)color << 8);
}

static void update_cursor(void) {
    u16 pos = vga_row * VGA_WIDTH + vga_column;
    outb(0x3D4, 0x0F);
    outb(0x3D5, (u8)(pos & 0xFF));
    outb(0x3D4, 0x0E);
    outb(0x3D5, (u8)((pos >> 8) & 0xFF));
}

static void vga_scroll(void) {
    // Move all lines up
    for (u32 y = 0; y < VGA_HEIGHT - 1; y++) {
        for (u32 x = 0; x < VGA_WIDTH; x++) {
            vga_buffer[y * VGA_WIDTH + x] = vga_buffer[(y + 1) * VGA_WIDTH + x];
        }
    }
    
    // Clear last line
    for (u32 x = 0; x < VGA_WIDTH; x++) {
        vga_buffer[(VGA_HEIGHT - 1) * VGA_WIDTH + x] = make_vga_entry(' ', vga_color);
    }
    
    vga_row = VGA_HEIGHT - 1;
}

void vga_init(void) {
    vga_color = make_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK);
    vga_clear();
}

void vga_clear(void) {
    for (u32 y = 0; y < VGA_HEIGHT; y++) {
        for (u32 x = 0; x < VGA_WIDTH; x++) {
            vga_buffer[y * VGA_WIDTH + x] = make_vga_entry(' ', vga_color);
        }
    }
    vga_row = 0;
    vga_column = 0;
    update_cursor();
}

void vga_set_color(u8 fg, u8 bg) {
    vga_color = make_color(fg, bg);
}

void vga_putc(char c) {
    if (c == '\n') {
        vga_column = 0;
        vga_row++;
    } else if (c == '\r') {
        vga_column = 0;
    } else if (c == '\t') {
        vga_column = (vga_column + 8) & ~7;
    } else if (c == '\b') {
        if (vga_column > 0) {
            vga_column--;
        }
    } else {
        vga_buffer[vga_row * VGA_WIDTH + vga_column] = make_vga_entry(c, vga_color);
        vga_column++;
    }
    
    if (vga_column >= VGA_WIDTH) {
        vga_column = 0;
        vga_row++;
    }
    
    if (vga_row >= VGA_HEIGHT) {
        vga_scroll();
    }
    
    update_cursor();
}

void vga_puts(const char* str) {
    while (*str) {
        vga_putc(*str++);
    }
}
