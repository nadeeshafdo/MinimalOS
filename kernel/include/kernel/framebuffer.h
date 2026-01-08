/* Framebuffer header for x86_64 with Limine */
#ifndef KERNEL_FRAMEBUFFER_H
#define KERNEL_FRAMEBUFFER_H

#include <stdint.h>

/* Forward declaration */
struct limine_framebuffer;

/* Framebuffer info structure */
typedef struct {
  uint64_t address;
  uint64_t pitch;
  uint32_t width;
  uint32_t height;
  uint16_t bpp;
  uint8_t type;
  uint8_t red_pos;
  uint8_t red_size;
  uint8_t green_pos;
  uint8_t green_size;
  uint8_t blue_pos;
  uint8_t blue_size;
} framebuffer_info_t;

/* Initialize from Limine framebuffer */
int fb_init_limine(struct limine_framebuffer *fb);

/* Check if framebuffer is available */
int fb_available(void);

/* Get framebuffer info */
framebuffer_info_t *fb_get_info(void);

/* Drawing functions */
void fb_putpixel(int x, int y, uint32_t color);
void fb_clear(uint32_t color);
void fb_fillrect(int x, int y, int w, int h, uint32_t color);
void fb_drawrect(int x, int y, int w, int h, uint32_t color);
void fb_drawline(int x1, int y1, int x2, int y2, uint32_t color);
void fb_scroll(int pixels);

/* Font rendering */
void fb_putchar(int x, int y, char c, uint32_t fg, uint32_t bg);

#endif
