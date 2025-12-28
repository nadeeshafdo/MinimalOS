#include <vga.h>
#include <io.h>

#define VGA_MEMORY ((volatile uint16_t*)0xFFFFFFFF800B8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

static uint8_t vga_color = 0x0F; // White on Black
static uint16_t cursor_x = 0;
static uint16_t cursor_y = 0;

void vga_set_color(uint8_t fg, uint8_t bg) {
    vga_color = (bg << 4) | fg;
}

static void vga_put_entry_at(char c, uint8_t color, uint16_t x, uint16_t y) {
    const uint16_t index = y * VGA_WIDTH + x;
    VGA_MEMORY[index] = (uint16_t)c | ((uint16_t)color << 8);
}

void vga_init(void) {
    cursor_x = 0;
    cursor_y = 0;
    for (int y = 0; y < VGA_HEIGHT; y++) {
        for (int x = 0; x < VGA_WIDTH; x++) {
            vga_put_entry_at(' ', vga_color, x, y);
        }
    }
}

static void vga_scroll(void) {
    for (int y = 0; y < VGA_HEIGHT - 1; y++) {
        for (int x = 0; x < VGA_WIDTH; x++) {
            const uint16_t index = y * VGA_WIDTH + x;
            const uint16_t next_index = (y + 1) * VGA_WIDTH + x;
            VGA_MEMORY[index] = VGA_MEMORY[next_index];
        }
    }
    for (int x = 0; x < VGA_WIDTH; x++) {
        vga_put_entry_at(' ', vga_color, x, VGA_HEIGHT - 1);
    }
}

void vga_write_char(char c) {
    if (c == '\n') {
        cursor_x = 0;
        cursor_y++;
    } else {
        vga_put_entry_at(c, vga_color, cursor_x, cursor_y);
        cursor_x++;
        if (cursor_x >= VGA_WIDTH) {
            cursor_x = 0;
            cursor_y++;
        }
    }

    if (cursor_y >= VGA_HEIGHT) {
        vga_scroll();
        cursor_y = VGA_HEIGHT - 1;
    }
}

void vga_print(const char* str) {
    while (*str) {
        vga_write_char(*str++);
    }
}
