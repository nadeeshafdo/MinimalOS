#ifndef _KERNEL_FRAMEBUFFER_H
#define _KERNEL_FRAMEBUFFER_H

#include <stdint.h>

/* Framebuffer info structure */
typedef struct {
    uint32_t address;       /* Physical address of framebuffer */
    uint32_t pitch;         /* Bytes per scanline */
    uint32_t width;         /* Width in pixels */
    uint32_t height;        /* Height in pixels */
    uint8_t  bpp;           /* Bits per pixel */
    uint8_t  type;          /* Framebuffer type */
    uint8_t  red_pos;       /* Red field position */
    uint8_t  red_size;      /* Red field size */
    uint8_t  green_pos;     /* Green field position */
    uint8_t  green_size;    /* Green field size */
    uint8_t  blue_pos;      /* Blue field position */
    uint8_t  blue_size;     /* Blue field size */
} framebuffer_info_t;

/* Colors */
#define FB_BLACK    0x000000
#define FB_WHITE    0xFFFFFF
#define FB_RED      0xFF0000
#define FB_GREEN    0x00FF00
#define FB_BLUE     0x0000FF
#define FB_CYAN     0x00FFFF
#define FB_MAGENTA  0xFF00FF
#define FB_YELLOW   0xFFFF00
#define FB_GRAY     0x808080
#define FB_LTGRAY   0xC0C0C0
#define FB_DKGRAY   0x404040

/* Initialize framebuffer from multiboot info */
int fb_init(void *multiboot_info);

/* Check if framebuffer is available */
int fb_available(void);

/* Get framebuffer info */
framebuffer_info_t *fb_get_info(void);

/* Drawing primitives */
void fb_putpixel(int x, int y, uint32_t color);
void fb_clear(uint32_t color);
void fb_fillrect(int x, int y, int w, int h, uint32_t color);
void fb_drawrect(int x, int y, int w, int h, uint32_t color);
void fb_drawline(int x1, int y1, int x2, int y2, uint32_t color);

/* Text rendering */
void fb_putchar(int x, int y, char c, uint32_t fg, uint32_t bg);
void fb_puts(int x, int y, const char *s, uint32_t fg, uint32_t bg);

/* Scroll framebuffer up by n pixels */
void fb_scroll(int pixels);

#endif /* _KERNEL_FRAMEBUFFER_H */
