/* Utility functions for shell commands */
#include <kernel/commands.h>
#include <kernel/tty.h>
#include <stdint.h>

/* Parse hexadecimal string to uint64 */
uint64_t parse_hex(const char *str) {
  uint64_t value = 0;

  /* Skip "0x" prefix if present */
  if (str[0] == '0' && (str[1] == 'x' || str[1] == 'X')) {
    str += 2;
  }

  while (*str) {
    char c = *str++;
    uint64_t digit;

    if (c >= '0' && c <= '9') {
      digit = c - '0';
    } else if (c >= 'a' && c <= 'f') {
      digit = c - 'a' + 10;
    } else if (c >= 'A' && c <= 'F') {
      digit = c - 'A' + 10;
    } else {
      break;
    }

    value = (value << 4) | digit;
  }

  return value;
}

/* Parse decimal string to uint64 */
uint64_t parse_dec(const char *str) {
  uint64_t value = 0;

  while (*str >= '0' && *str <= '9') {
    value = value * 10 + (*str - '0');
    str++;
  }

  return value;
}

/* Print 64-bit hex value */
void print_hex64(uint64_t value) {
  char hex[19] = "0x0000000000000000";
  const char *digits = "0123456789ABCDEF";

  for (int i = 17; i >= 2; i--) {
    hex[i] = digits[value & 0xF];
    value >>= 4;
  }

  terminal_writestring(hex);
}

/* Print 64-bit decimal value */
void print_dec64(uint64_t value) {
  char buf[21];
  int i = 20;
  buf[i] = '\0';

  if (value == 0) {
    terminal_writestring("0");
    return;
  }

  while (value > 0 && i > 0) {
    buf[--i] = '0' + (value % 10);
    value /= 10;
  }

  terminal_writestring(&buf[i]);
}
