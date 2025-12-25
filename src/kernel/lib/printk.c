#include "printk.h"
#include "string.h"
#include "../drivers/serial.h"
#include "../drivers/vga.h"
#include <stdarg.h>

static void print_char(char c) {
    serial_putc(c);
    vga_putc(c);
}

static void print_string(const char* str) {
    while (*str) {
        print_char(*str++);
    }
}

static void print_uint(u64 value, u32 base) {
    char buffer[32];
    const char* digits = "0123456789abcdef";
    i32 pos = 0;
    
    if (value == 0) {
        print_char('0');
        return;
    }
    
    while (value > 0) {
        buffer[pos++] = digits[value % base];
        value /= base;
    }
    
    while (pos > 0) {
        print_char(buffer[--pos]);
    }
}

static void print_int(i64 value, u32 base) {
    if (value < 0 && base == 10) {
        print_char('-');
        value = -value;
    }
    print_uint((u64)value, base);
}

void printk(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    
    while (*fmt) {
        if (*fmt == '%') {
            fmt++;
            switch (*fmt) {
                case 'd':
                case 'i': {
                    i32 val = va_arg(args, i32);
                    print_int(val, 10);
                    break;
                }
                case 'u': {
                    u32 val = va_arg(args, u32);
                    print_uint(val, 10);
                    break;
                }
                case 'x': {
                    u32 val = va_arg(args, u32);
                    print_uint(val, 16);
                    break;
                }
                case 'p': {
                    print_string("0x");
                    u64 val = (u64)va_arg(args, void*);
                    print_uint(val, 16);
                    break;
                }
                case 's': {
                    const char* str = va_arg(args, const char*);
                    if (str) {
                        print_string(str);
                    } else {
                        print_string("(null)");
                    }
                    break;
                }
                case 'c': {
                    char c = (char)va_arg(args, int);
                    print_char(c);
                    break;
                }
                case 'l': {
                    fmt++;
                    if (*fmt == 'x') {
                        u64 val = va_arg(args, u64);
                        print_uint(val, 16);
                    } else if (*fmt == 'd') {
                        i64 val = va_arg(args, i64);
                        print_int(val, 10);
                    }
                    break;
                }
                case '%': {
                    print_char('%');
                    break;
                }
                default:
                    print_char('%');
                    print_char(*fmt);
                    break;
            }
        } else {
            print_char(*fmt);
        }
        fmt++;
    }
    
    va_end(args);
}
