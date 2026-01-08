/* Framebuffer driver for VESA graphics mode */
#include <stdint.h>
#include <stddef.h>
#include <kernel/framebuffer.h>

/* Multiboot info structure (partial) */
struct multiboot_info {
    uint32_t flags;
    uint32_t mem_lower;
    uint32_t mem_upper;
    uint32_t boot_device;
    uint32_t cmdline;
    uint32_t mods_count;
    uint32_t mods_addr;
    uint32_t syms[4];
    uint32_t mmap_length;
    uint32_t mmap_addr;
    uint32_t drives_length;
    uint32_t drives_addr;
    uint32_t config_table;
    uint32_t boot_loader_name;
    uint32_t apm_table;
    uint32_t vbe_control_info;
    uint32_t vbe_mode_info;
    uint16_t vbe_mode;
    uint16_t vbe_interface_seg;
    uint16_t vbe_interface_off;
    uint16_t vbe_interface_len;
    uint64_t framebuffer_addr;
    uint32_t framebuffer_pitch;
    uint32_t framebuffer_width;
    uint32_t framebuffer_height;
    uint8_t  framebuffer_bpp;
    uint8_t  framebuffer_type;
    uint8_t  color_info[6];
} __attribute__((packed));

static framebuffer_info_t fb_info;
static int fb_initialized = 0;

/* Memory copy */
static void *memcpy(void *dest, const void *src, size_t n) {
    uint8_t *d = dest;
    const uint8_t *s = src;
    while (n--) *d++ = *s++;
    return dest;
}

/* Memory set */
static void *memset(void *s, int c, size_t n) {
    uint8_t *p = s;
    while (n--) *p++ = (uint8_t)c;
    return s;
}

int fb_init(void *multiboot_info) {
    struct multiboot_info *mb = (struct multiboot_info *)multiboot_info;
    
    /* Check if framebuffer info is available (bit 12) */
    if (!(mb->flags & (1 << 12))) {
        return 0;  /* No framebuffer */
    }
    
    /* Only support linear framebuffer (type 1) */
    if (mb->framebuffer_type != 1) {
        return 0;
    }
    
    fb_info.address = (uint32_t)mb->framebuffer_addr;
    fb_info.pitch = mb->framebuffer_pitch;
    fb_info.width = mb->framebuffer_width;
    fb_info.height = mb->framebuffer_height;
    fb_info.bpp = mb->framebuffer_bpp;
    fb_info.type = mb->framebuffer_type;
    
    /* Parse color info for RGB mode */
    fb_info.red_pos = mb->color_info[0];
    fb_info.red_size = mb->color_info[1];
    fb_info.green_pos = mb->color_info[2];
    fb_info.green_size = mb->color_info[3];
    fb_info.blue_pos = mb->color_info[4];
    fb_info.blue_size = mb->color_info[5];
    
    fb_initialized = 1;
    return 1;
}

int fb_available(void) {
    return fb_initialized;
}

framebuffer_info_t *fb_get_info(void) {
    return &fb_info;
}

void fb_putpixel(int x, int y, uint32_t color) {
    if (!fb_initialized) return;
    if (x < 0 || x >= (int)fb_info.width) return;
    if (y < 0 || y >= (int)fb_info.height) return;
    
    uint32_t *pixel = (uint32_t *)(fb_info.address + y * fb_info.pitch + x * (fb_info.bpp / 8));
    *pixel = color;
}

void fb_clear(uint32_t color) {
    if (!fb_initialized) return;
    
    for (uint32_t y = 0; y < fb_info.height; y++) {
        uint32_t *row = (uint32_t *)(fb_info.address + y * fb_info.pitch);
        for (uint32_t x = 0; x < fb_info.width; x++) {
            row[x] = color;
        }
    }
}

void fb_fillrect(int x, int y, int w, int h, uint32_t color) {
    if (!fb_initialized) return;
    
    for (int j = y; j < y + h; j++) {
        if (j < 0 || j >= (int)fb_info.height) continue;
        for (int i = x; i < x + w; i++) {
            if (i < 0 || i >= (int)fb_info.width) continue;
            uint32_t *pixel = (uint32_t *)(fb_info.address + j * fb_info.pitch + i * 4);
            *pixel = color;
        }
    }
}

void fb_drawrect(int x, int y, int w, int h, uint32_t color) {
    if (!fb_initialized) return;
    
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
    if (!fb_initialized) return;
    
    int dx = x2 > x1 ? x2 - x1 : x1 - x2;
    int dy = y2 > y1 ? y2 - y1 : y1 - y2;
    int sx = x1 < x2 ? 1 : -1;
    int sy = y1 < y2 ? 1 : -1;
    int err = dx - dy;
    
    while (1) {
        fb_putpixel(x1, y1, color);
        if (x1 == x2 && y1 == y2) break;
        int e2 = 2 * err;
        if (e2 > -dy) { err -= dy; x1 += sx; }
        if (e2 < dx) { err += dx; y1 += sy; }
    }
}

void fb_scroll(int pixels) {
    if (!fb_initialized) return;
    
    /* Move screen content up */
    uint32_t bytes_to_move = (fb_info.height - pixels) * fb_info.pitch;
    memcpy((void *)fb_info.address, 
           (void *)(fb_info.address + pixels * fb_info.pitch), 
           bytes_to_move);
    
    /* Clear bottom area */
    memset((void *)(fb_info.address + bytes_to_move), 
           0, 
           pixels * fb_info.pitch);
}
