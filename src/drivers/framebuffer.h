/**
 * MinimalOS - Framebuffer Driver
 * Supports Multiboot2 Framebuffer (RGB 32-bit)
 */

#ifndef DRIVERS_FRAMEBUFFER_H
#define DRIVERS_FRAMEBUFFER_H

#include <minimalos/multiboot2.h>
#include <minimalos/types.h>

/* Initialize framebuffer from Multiboot2 tag */
void framebuffer_init(struct multiboot2_tag_framebuffer *tag);

/* Clear screen with color */
void framebuffer_clear(uint32_t color);

/* Draw pixel (RGB 0xRRGGBB) */
void framebuffer_draw_pixel(uint32_t x, uint32_t y, uint32_t color);

/* Draw character */
void framebuffer_draw_char(char c, uint32_t x, uint32_t y, uint32_t fg,
                           uint32_t bg);

/* Write string to console */
void framebuffer_console_write(const char *str);
void framebuffer_console_writelen(const char *str, size_t len);

/* Output string to framebuffer console (used by printk) */
void framebuffer_putchar(char c);

/* Check if initialized */
int framebuffer_is_ready(void);

#endif /* DRIVERS_FRAMEBUFFER_H */
