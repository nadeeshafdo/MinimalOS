/* Terminal driver with Framebuffer support for x86_64 */
#include <kernel/framebuffer.h>
#include <kernel/tty.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* ==================== FONT CONSTANTS ==================== */
#define FONT_WIDTH 8
#define FONT_HEIGHT 16

/* ==================== STATE ==================== */
static size_t terminal_row;
static size_t terminal_column;
static size_t terminal_width;
static size_t terminal_height;
static uint8_t terminal_color;
static int use_framebuffer = 1; /* Default to framebuffer in x86_64 */

/* Current colors for framebuffer mode */
static uint32_t fb_fg_color = 0xC0C0C0; /* Light gray */
static uint32_t fb_bg_color = 0x000000; /* Black */

/* VGA color to RGB mapping */
static const uint32_t vga_to_rgb[] = {
    0x000000, /* 0: Black */
    0x0000AA, /* 1: Blue */
    0x00AA00, /* 2: Green */
    0x00AAAA, /* 3: Cyan */
    0xAA0000, /* 4: Red */
    0xAA00AA, /* 5: Magenta */
    0xAA5500, /* 6: Brown */
    0xAAAAAA, /* 7: Light Gray */
    0x555555, /* 8: Dark Gray */
    0x5555FF, /* 9: Light Blue */
    0x55FF55, /* 10: Light Green */
    0x55FFFF, /* 11: Light Cyan */
    0xFF5555, /* 12: Light Red */
    0xFF55FF, /* 13: Light Magenta/Pink */
    0xFFFF55, /* 14: Yellow */
    0xFFFFFF, /* 15: White */
};

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

/* Simple strlen implementation */
static size_t strlen(const char *str) {
  size_t len = 0;
  while (str[len])
    len++;
  return len;
}

/* ==================== FRAMEBUFFER FUNCTIONS ==================== */

/* Draw software cursor (framebuffer) */
static void fb_draw_cursor(void) {
  int x = terminal_column * FONT_WIDTH;
  int y = terminal_row * FONT_HEIGHT + FONT_HEIGHT - 2;
  fb_fillrect(x, y, FONT_WIDTH, 2, fb_fg_color);
}

/* Erase software cursor (framebuffer) */
static void fb_erase_cursor(void) {
  int x = terminal_column * FONT_WIDTH;
  int y = terminal_row * FONT_HEIGHT + FONT_HEIGHT - 2;
  fb_fillrect(x, y, FONT_WIDTH, 2, fb_bg_color);
}

/* Scroll framebuffer mode */
static void fb_scroll_terminal(void) {
  fb_scroll(FONT_HEIGHT);
  terminal_row = terminal_height - 1;
}

/* Put character at position (framebuffer) */
static void fb_putentryat(char c, size_t x, size_t y) {
  fb_putchar(x * FONT_WIDTH, y * FONT_HEIGHT, c, fb_fg_color, fb_bg_color);
}

/* ==================== PUBLIC API ==================== */

void terminal_set_framebuffer(int enabled) {
  use_framebuffer = enabled;
  if (enabled && fb_available()) {
    framebuffer_info_t *fb = fb_get_info();
    terminal_width = fb->width / FONT_WIDTH;
    terminal_height = fb->height / FONT_HEIGHT;
  } else {
    use_framebuffer = 0;
    terminal_width = 80;
    terminal_height = 25;
  }
}

void terminal_initialize(void) {
  terminal_row = 0;
  terminal_column = 0;
  terminal_color = vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK);

  if (use_framebuffer && fb_available()) {
    framebuffer_info_t *fb = fb_get_info();
    terminal_width = fb->width / FONT_WIDTH;
    terminal_height = fb->height / FONT_HEIGHT;

    /* Clear framebuffer */
    fb_clear(fb_bg_color);
    fb_draw_cursor();
  } else {
    use_framebuffer = 0;
    terminal_width = 80;
    terminal_height = 25;
  }
}

void terminal_setcolor(uint8_t color) {
  terminal_color = color;

  if (use_framebuffer) {
    /* Convert VGA color to RGB */
    fb_fg_color = vga_to_rgb[color & 0x0F];
    fb_bg_color = vga_to_rgb[(color >> 4) & 0x0F];
  }
}

static void terminal_scroll(void) {
  if (use_framebuffer) {
    fb_scroll_terminal();
  }
}

static void update_cursor(void) {
  if (use_framebuffer) {
    fb_draw_cursor();
  }
}

void terminal_putentryat(char c, uint8_t color, size_t x, size_t y) {
  if (use_framebuffer) {
    /* Temporarily set colors */
    uint32_t old_fg = fb_fg_color;
    uint32_t old_bg = fb_bg_color;
    fb_fg_color = vga_to_rgb[color & 0x0F];
    fb_bg_color = vga_to_rgb[(color >> 4) & 0x0F];
    fb_putentryat(c, x, y);
    fb_fg_color = old_fg;
    fb_bg_color = old_bg;
  }
}

void terminal_putchar(char c) {
  terminal_acquire();

  if (use_framebuffer) {
    fb_erase_cursor();
  }

  /* Handle newline */
  if (c == '\n') {
    terminal_column = 0;
    if (++terminal_row >= terminal_height) {
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
      if (use_framebuffer) {
        fb_putentryat(' ', terminal_column, terminal_row);
      }
    }
    update_cursor();
    terminal_release();
    return;
  }

  /* Handle tab (4 spaces) */
  if (c == '\t') {
    terminal_column = (terminal_column + 4) & ~(4 - 1);
    if (terminal_column >= terminal_width) {
      terminal_column = 0;
      if (++terminal_row >= terminal_height) {
        terminal_scroll();
      }
    }
    update_cursor();
    terminal_release();
    return;
  }

  /* Print regular character */
  if (use_framebuffer) {
    fb_putentryat(c, terminal_column, terminal_row);
  }

  if (++terminal_column >= terminal_width) {
    terminal_column = 0;
    if (++terminal_row >= terminal_height) {
      terminal_scroll();
    }
  }

  update_cursor();
  terminal_release();
}

void terminal_write(const char *data, size_t size) {
  for (size_t i = 0; i < size; i++)
    terminal_putchar(data[i]);
}

void terminal_writestring(const char *data) {
  terminal_write(data, strlen(data));
}

void terminal_clear(void) {
  terminal_row = 0;
  terminal_column = 0;

  if (use_framebuffer) {
    fb_clear(fb_bg_color);
    fb_draw_cursor();
  }
}
