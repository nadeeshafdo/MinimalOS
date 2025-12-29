/**
 * MinimalOS - Framebuffer Driver Implementation
 * Software font rendering for UEFI/GOP framebuffers
 */

#include "framebuffer.h"
#include "font.h"
#include <minimalos/multiboot2.h>
#include <mm/vmm.h>

/* Fixed virtual address for framebuffer mapping */
#define FB_VIRT_BASE 0xFFFFFFFFFC000000

/* Driver state */
static uint32_t *fb_buffer = NULL;
static uint32_t fb_pitch = 0;
static uint32_t fb_width = 0;
static uint32_t fb_height = 0;
static uint32_t fb_bpp = 0;

/* Console state */
static uint32_t cursor_x = 0;
static uint32_t cursor_y = 0;
static uint32_t fg_color = 0xFFFFFF; /* White */
static uint32_t bg_color = 0x000000; /* Black */

/* Check if framebuffer is initialized */
int framebuffer_is_ready(void) { return fb_buffer != NULL; }

/* Initialize framebuffer */
void framebuffer_init(struct multiboot2_tag_framebuffer *tag) {
  if (!tag)
    return;

  fb_width = tag->framebuffer_width;
  fb_height = tag->framebuffer_height;
  fb_pitch = tag->framebuffer_pitch;
  fb_bpp = tag->framebuffer_bpp;

  /* We only support 32-bit color for now */
  if (fb_bpp != 32)
    return;

  /* Calculate required size (pitch * height) */
  size_t size = (size_t)fb_pitch * fb_height;

  /* Align size to page boundary */
  size = (size + 0xFFF) & ~0xFFF;

  /* Map physical framebuffer to virtual address */
  /* Use VMM_KERNEL_MMIO flags (uncached/write-through usually, or just kernel
   * RW) */
  /* VMM_KERNEL_MMIO is defined in vmm.h */
  if (vmm_map_region(FB_VIRT_BASE, tag->framebuffer_addr, size,
                     VMM_KERNEL_MMIO) == 0) {
    fb_buffer = (uint32_t *)FB_VIRT_BASE;
    framebuffer_clear(bg_color);
  }
}

/* Draw a single pixel */
void framebuffer_draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
  if (!fb_buffer || x >= fb_width || y >= fb_height)
    return;

  /* Calculate offset in bytes: y * pitch + x * 4 (32-bit) */
  uint32_t offset = (y * fb_pitch) + (x * 4);

  /* Write pixel */
  *(volatile uint32_t *)((uint8_t *)fb_buffer + offset) = color;
}

/* Draw a character using bitmap font */
void framebuffer_draw_char(char c, uint32_t x, uint32_t y, uint32_t fg,
                           uint32_t bg) {
  if (c < 32 || c > 126)
    c = '?';

  /* Get font bitmap for character */
  const uint8_t *glyph = &font_8x16[(c - 32) * 16];

  for (int row = 0; row < 16; row++) {
    uint8_t bits = glyph[row];
    for (int col = 0; col < 8; col++) {
      /* Bitmap is MSB-first (pixel 0 is bit 7) */
      if (bits & (1 << (7 - col))) {
        framebuffer_draw_pixel(x + col, y + row, fg);
      } else {
        framebuffer_draw_pixel(x + col, y + row, bg);
      }
    }
  }
}

/* Clear screen */
void framebuffer_clear(uint32_t color) {
  if (!fb_buffer)
    return;

  /* Naive clear (pixel by pixel) - fast enough for boot */
  for (uint32_t y = 0; y < fb_height; y++) {
    for (uint32_t x = 0; x < fb_width; x++) {
      framebuffer_draw_pixel(x, y, color);
    }
  }

  cursor_x = 0;
  cursor_y = 0;
}

/* Scroll screen up (simple implementation: clear and reset for now) */
/* Real scrolling requires reading back from FB (slow) or keeping a backbuffer
 */
static void framebuffer_scroll(void) {
  /* For MinimalOS v0.1: Just clear screen on overflow */
  framebuffer_clear(bg_color);
  cursor_x = 0;
  cursor_y = 0;
}

/* Put character on console */
void framebuffer_putchar(char c) {
  if (!fb_buffer)
    return;

  if (c == '\n') {
    cursor_x = 0;
    cursor_y += 16;
  } else if (c == '\r') {
    cursor_x = 0;
  } else if (c == '\b') {
    if (cursor_x >= 8)
      cursor_x -= 8;
  } else {
    framebuffer_draw_char(c, cursor_x, cursor_y, fg_color, bg_color);
    cursor_x += 8;
    if (cursor_x >= fb_width) {
      cursor_x = 0;
      cursor_y += 16;
    }
  }

  if (cursor_y + 16 > fb_height) {
    framebuffer_scroll();
  }
}

/* Write len characters */
void framebuffer_console_writelen(const char *str, size_t len) {
  for (size_t i = 0; i < len; i++) {
    framebuffer_putchar(str[i]);
  }
}

/* Write string */
void framebuffer_console_write(const char *str) {
  while (*str) {
    framebuffer_putchar(*str++);
  }
}
