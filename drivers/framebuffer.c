/* Framebuffer driver for Limine bootloader */
#include <kernel/framebuffer.h>
#include <limine.h>
#include <stddef.h>
#include <stdint.h>

static framebuffer_info_t fb_info;
static int fb_initialized = 0;

/* Memory copy */
static void *fb_memcpy(void *dest, const void *src, size_t n) {
  uint8_t *d = dest;
  const uint8_t *s = src;
  while (n--)
    *d++ = *s++;
  return dest;
}

/* Memory set */
static void *fb_memset(void *s, int c, size_t n) {
  uint8_t *p = s;
  while (n--)
    *p++ = (uint8_t)c;
  return s;
}

/* Initialize framebuffer from Limine structure */
int fb_init_limine(struct limine_framebuffer *fb) {
  if (fb == NULL) {
    return 0;
  }

  /* Limine provides virtual address directly */
  fb_info.address = (uint64_t)fb->address;
  fb_info.pitch = fb->pitch;
  fb_info.width = fb->width;
  fb_info.height = fb->height;
  fb_info.bpp = fb->bpp;
  fb_info.type = fb->memory_model;

  /* Color info */
  fb_info.red_pos = fb->red_mask_shift;
  fb_info.red_size = fb->red_mask_size;
  fb_info.green_pos = fb->green_mask_shift;
  fb_info.green_size = fb->green_mask_size;
  fb_info.blue_pos = fb->blue_mask_shift;
  fb_info.blue_size = fb->blue_mask_size;

  fb_initialized = 1;
  return 1;
}

int fb_available(void) { return fb_initialized; }

framebuffer_info_t *fb_get_info(void) { return &fb_info; }

void fb_putpixel(int x, int y, uint32_t color) {
  if (!fb_initialized)
    return;
  if (x < 0 || x >= (int)fb_info.width)
    return;
  if (y < 0 || y >= (int)fb_info.height)
    return;

  uint32_t *pixel =
      (uint32_t *)(fb_info.address + y * fb_info.pitch + x * (fb_info.bpp / 8));
  *pixel = color;
}

void fb_clear(uint32_t color) {
  if (!fb_initialized)
    return;

  for (uint32_t y = 0; y < fb_info.height; y++) {
    uint32_t *row = (uint32_t *)(fb_info.address + y * fb_info.pitch);
    for (uint32_t x = 0; x < fb_info.width; x++) {
      row[x] = color;
    }
  }
}

void fb_fillrect(int x, int y, int w, int h, uint32_t color) {
  if (!fb_initialized)
    return;

  for (int j = y; j < y + h; j++) {
    if (j < 0 || j >= (int)fb_info.height)
      continue;
    for (int i = x; i < x + w; i++) {
      if (i < 0 || i >= (int)fb_info.width)
        continue;
      uint32_t *pixel =
          (uint32_t *)(fb_info.address + j * fb_info.pitch + i * 4);
      *pixel = color;
    }
  }
}

void fb_drawrect(int x, int y, int w, int h, uint32_t color) {
  if (!fb_initialized)
    return;

  /* Top and bottom */
  for (int i = x; i < x + w; i++) {
    fb_putpixel(i, y, color);
    fb_putpixel(i, y + h - 1, color);
  }
  /* Left and right */
  for (int j = y; j < y + h; j++) {
    fb_putpixel(x, j, color);
    fb_putpixel(x + w - 1, j, color);
  }
}

void fb_drawline(int x1, int y1, int x2, int y2, uint32_t color) {
  if (!fb_initialized)
    return;

  int dx = x2 > x1 ? x2 - x1 : x1 - x2;
  int dy = y2 > y1 ? y2 - y1 : y1 - y2;
  int sx = x1 < x2 ? 1 : -1;
  int sy = y1 < y2 ? 1 : -1;
  int err = dx - dy;

  while (1) {
    fb_putpixel(x1, y1, color);
    if (x1 == x2 && y1 == y2)
      break;
    int e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      x1 += sx;
    }
    if (e2 < dx) {
      err += dx;
      y1 += sy;
    }
  }
}

void fb_scroll(int pixels) {
  if (!fb_initialized)
    return;

  /* Move screen content up */
  uint64_t bytes_to_move = (fb_info.height - pixels) * fb_info.pitch;
  fb_memcpy((void *)fb_info.address,
            (void *)(fb_info.address + pixels * fb_info.pitch), bytes_to_move);

  /* Clear bottom area */
  fb_memset((void *)(fb_info.address + bytes_to_move), 0,
            pixels * fb_info.pitch);
}
