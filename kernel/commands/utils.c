/* Command utility functions */
#include <stdint.h>
#include <kernel/commands.h>
#include <kernel/tty.h>

/* Print hex number */
void cmd_print_hex(uint32_t value) {
    char hex[11] = "0x00000000";
    const char* digits = "0123456789ABCDEF";
    for (int i = 9; i >= 2; i--) {
        hex[i] = digits[value & 0xF];
        value >>= 4;
    }
    terminal_writestring(hex);
}

/* Print hex byte */
void cmd_print_hex_byte(uint8_t value) {
    const char* digits = "0123456789ABCDEF";
    terminal_putchar(digits[(value >> 4) & 0xF]);
    terminal_putchar(digits[value & 0xF]);
}

/* Print decimal number */
void cmd_print_dec(uint32_t value) {
    char buf[12];
    int i = 10;
    buf[11] = '\0';
    if (value == 0) {
        terminal_writestring("0");
        return;
    }
    while (value > 0) {
        buf[i--] = '0' + (value % 10);
        value /= 10;
    }
    terminal_writestring(&buf[i + 1]);
}

/* Parse hex string */
uint32_t cmd_parse_hex(const char *s) {
    uint32_t result = 0;
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) s += 2;
    while (*s) {
        char c = *s++;
        result <<= 4;
        if (c >= '0' && c <= '9') result |= c - '0';
        else if (c >= 'a' && c <= 'f') result |= c - 'a' + 10;
        else if (c >= 'A' && c <= 'F') result |= c - 'A' + 10;
        else break;
    }
    return result;
}

/* Parse decimal string */
uint32_t cmd_parse_dec(const char *s) {
    uint32_t result = 0;
    while (*s >= '0' && *s <= '9') {
        result = result * 10 + (*s++ - '0');
    }
    return result;
}

/* Get next argument */
const char* cmd_get_arg(const char *s, char *buf, uint32_t max) {
    while (*s == ' ' || *s == '\t') s++;
    uint32_t i = 0;
    while (*s && *s != ' ' && *s != '\t' && i < max - 1) {
        buf[i++] = *s++;
    }
    buf[i] = '\0';
    return s;
}

/* String length helper */
uint32_t cmd_strlen(const char *s) {
    uint32_t len = 0;
    while (s[len]) len++;
    return len;
}
