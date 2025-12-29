/**
 * MinimalOS - Kernel Printing (printk)
 * Outputs to both serial port and VGA console
 */

#include "../drivers/framebuffer.h"
#include <minimalos/types.h>

/* External output functions */
extern void serial_putchar(char c);
extern void vga_putchar(char c);

/* Static flag to track if VGA is initialized */
static bool vga_ready = false;

void vga_set_ready(void) { vga_ready = true; }

/**
 * Output a single character to all consoles
 */
static void putchar(char c) {
  serial_putchar(c);
  if (vga_ready) {
    vga_putchar(c);
  }

  if (framebuffer_is_ready()) {
    framebuffer_putchar(c);
  }
}

/**
 * Output a string
 */
static void puts(const char *s) {
  while (*s) {
    putchar(*s++);
  }
}

/**
 * Convert integer to string (decimal)
 */
static void print_int(int64_t value, bool is_signed) {
  char buf[21]; /* Enough for 64-bit integer */
  char *p = buf + sizeof(buf) - 1;
  bool negative = false;
  uint64_t uvalue;

  *p = '\0';

  if (is_signed && value < 0) {
    negative = true;
    uvalue = (uint64_t)(-value);
  } else {
    uvalue = (uint64_t)value;
  }

  if (uvalue == 0) {
    *--p = '0';
  } else {
    while (uvalue > 0) {
      *--p = '0' + (uvalue % 10);
      uvalue /= 10;
    }
  }

  if (negative) {
    *--p = '-';
  }

  puts(p);
}

/**
 * Convert integer to hex string
 */
static void print_hex(uint64_t value, int width) {
  static const char hex_chars[] = "0123456789abcdef";
  char buf[17];
  char *p = buf + sizeof(buf) - 1;
  int digits = 0;

  *p = '\0';

  if (value == 0) {
    *--p = '0';
    digits = 1;
  } else {
    while (value > 0) {
      *--p = hex_chars[value & 0xF];
      value >>= 4;
      digits++;
    }
  }

  /* Pad with zeros if width specified */
  while (digits < width) {
    *--p = '0';
    digits++;
  }

  puts(p);
}

/**
 * Simple printf-like function for kernel
 * Supports: %s, %d, %u, %x, %lx, %lu, %ld, %p, %c, %%
 */
void printk(const char *fmt, ...) {
  __builtin_va_list args;
  __builtin_va_start(args, fmt);

  while (*fmt) {
    if (*fmt != '%') {
      putchar(*fmt++);
      continue;
    }

    fmt++; /* Skip '%' */

    /* Check for 'l' modifier */
    bool is_long = false;
    if (*fmt == 'l') {
      is_long = true;
      fmt++;
    }

    switch (*fmt) {
    case 's': {
      const char *s = __builtin_va_arg(args, const char *);
      puts(s ? s : "(null)");
      break;
    }
    case 'd': {
      if (is_long) {
        int64_t val = __builtin_va_arg(args, int64_t);
        print_int(val, true);
      } else {
        int val = __builtin_va_arg(args, int);
        print_int(val, true);
      }
      break;
    }
    case 'u': {
      if (is_long) {
        uint64_t val = __builtin_va_arg(args, uint64_t);
        print_int(val, false);
      } else {
        unsigned int val = __builtin_va_arg(args, unsigned int);
        print_int(val, false);
      }
      break;
    }
    case 'x': {
      if (is_long) {
        uint64_t val = __builtin_va_arg(args, uint64_t);
        print_hex(val, 0);
      } else {
        unsigned int val = __builtin_va_arg(args, unsigned int);
        print_hex(val, 0);
      }
      break;
    }
    case 'p': {
      void *ptr = __builtin_va_arg(args, void *);
      puts("0x");
      print_hex((uint64_t)ptr, 16);
      break;
    }
    case 'c': {
      char c = (char)__builtin_va_arg(args, int);
      putchar(c);
      break;
    }
    case '%':
      putchar('%');
      break;
    default:
      putchar('%');
      putchar(*fmt);
      break;
    }
    fmt++;
  }

  __builtin_va_end(args);
}
