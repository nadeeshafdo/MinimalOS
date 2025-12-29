/**
 * MinimalOS - VGA Text Mode Driver
 * Basic 80x25 text mode console output
 */

#include <minimalos/types.h>

/* VGA text mode constants */
#define VGA_BUFFER 0xFFFFFFFF800B8000 /* Virtual address */
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

/* VGA colors */
#define VGA_BLACK 0x0
#define VGA_BLUE 0x1
#define VGA_GREEN 0x2
#define VGA_CYAN 0x3
#define VGA_RED 0x4
#define VGA_MAGENTA 0x5
#define VGA_BROWN 0x6
#define VGA_LIGHT_GRAY 0x7
#define VGA_DARK_GRAY 0x8
#define VGA_LIGHT_BLUE 0x9
#define VGA_LIGHT_GREEN 0xA
#define VGA_LIGHT_CYAN 0xB
#define VGA_LIGHT_RED 0xC
#define VGA_LIGHT_MAGENTA 0xD
#define VGA_YELLOW 0xE
#define VGA_WHITE 0xF

/* Create VGA color attribute */
#define VGA_COLOR(fg, bg) ((bg << 4) | fg)
#define VGA_ENTRY(c, color) ((uint16_t)(c) | ((uint16_t)(color) << 8))

/* Current state */
static uint16_t *vga_buffer = (uint16_t *)VGA_BUFFER;
static uint8_t vga_row = 0;
static uint8_t vga_col = 0;
static uint8_t vga_color = VGA_COLOR(VGA_LIGHT_GRAY, VGA_BLACK);

/* External function to signal VGA is ready */
extern void vga_set_ready(void);

/* Port I/O */
static inline void outb(uint16_t port, uint8_t value) {
  __asm__ volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

/**
 * Update hardware cursor position
 */
static void vga_update_cursor(void) {
  uint16_t pos = vga_row * VGA_WIDTH + vga_col;

  outb(0x3D4, 0x0F);
  outb(0x3D5, (uint8_t)(pos & 0xFF));
  outb(0x3D4, 0x0E);
  outb(0x3D5, (uint8_t)((pos >> 8) & 0xFF));
}

/**
 * Scroll the screen up by one line
 */
static void vga_scroll(void) {
  /* Move all lines up by one */
  for (int i = 0; i < (VGA_HEIGHT - 1) * VGA_WIDTH; i++) {
    vga_buffer[i] = vga_buffer[i + VGA_WIDTH];
  }

  /* Clear the last line */
  for (int i = 0; i < VGA_WIDTH; i++) {
    vga_buffer[(VGA_HEIGHT - 1) * VGA_WIDTH + i] = VGA_ENTRY(' ', vga_color);
  }

  vga_row = VGA_HEIGHT - 1;
}

/**
 * Clear the screen
 */
void vga_clear(void) {
  for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
    vga_buffer[i] = VGA_ENTRY(' ', vga_color);
  }
  vga_row = 0;
  vga_col = 0;
  vga_update_cursor();
}

/**
 * Initialize VGA text mode
 */
void vga_init(void) {
  vga_color = VGA_COLOR(VGA_LIGHT_GRAY, VGA_BLACK);
  vga_clear();
  vga_set_ready();
}

/**
 * Set the current text color
 */
void vga_set_color(uint8_t fg, uint8_t bg) { vga_color = VGA_COLOR(fg, bg); }

/**
 * Put a character at current position
 */
void vga_putchar(char c) {
  if (c == '\n') {
    vga_col = 0;
    vga_row++;
  } else if (c == '\r') {
    vga_col = 0;
  } else if (c == '\t') {
    vga_col = (vga_col + 8) & ~7;
    if (vga_col >= VGA_WIDTH) {
      vga_col = 0;
      vga_row++;
    }
  } else if (c == '\b') {
    if (vga_col > 0) {
      vga_col--;
      vga_buffer[vga_row * VGA_WIDTH + vga_col] = VGA_ENTRY(' ', vga_color);
    }
  } else {
    vga_buffer[vga_row * VGA_WIDTH + vga_col] = VGA_ENTRY(c, vga_color);
    vga_col++;
    if (vga_col >= VGA_WIDTH) {
      vga_col = 0;
      vga_row++;
    }
  }

  if (vga_row >= VGA_HEIGHT) {
    vga_scroll();
  }

  vga_update_cursor();
}

/**
 * Print a string
 */
void vga_puts(const char *str) {
  while (*str) {
    vga_putchar(*str++);
  }
}

/**
 * Put a character at specific position
 */
void vga_putchar_at(char c, uint8_t row, uint8_t col, uint8_t color) {
  if (row < VGA_HEIGHT && col < VGA_WIDTH) {
    vga_buffer[row * VGA_WIDTH + col] = VGA_ENTRY(c, color);
  }
}
