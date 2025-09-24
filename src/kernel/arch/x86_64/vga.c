#include "vga.h"
#include "../../stdint.h"

#define VGA_WIDTH 80
#define VGA_HEIGHT 25
static uint16_t *vga_buf = (uint16_t *)0xB8000;
static int vga_row = 0, vga_col = 0;

void vga_init() {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        vga_buf[i] = 0x0700 | ' ';
    }
    vga_row = 0;
    vga_col = 0;
}

void vga_putchar(char ch) {
    if (ch == '\n') {
        vga_col = 0;
        vga_row++;
    } else {
        vga_buf[vga_row * VGA_WIDTH + vga_col] = 0x0700 | ch;
        vga_col++;
    }
    if (vga_col >= VGA_WIDTH) {
        vga_col = 0;
        vga_row++;
    }
    if (vga_row >= VGA_HEIGHT) {
        vga_row = 0;
    }
}

void vga_print(const char *str) {
    while (*str) {
        vga_putchar(*str);
        str++;
    }
}