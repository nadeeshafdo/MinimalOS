/* Memory shell commands for x86_64 */
#include <kernel/commands.h>
#include <kernel/kheap.h>
#include <kernel/tty.h>

void cmd_peek(const char *args) {
  if (!args || !*args) {
    terminal_writestring("\nUsage: peek <address>\n");
    return;
  }

  uint64_t addr = parse_hex(args);
  uint64_t value = *(volatile uint64_t *)addr;

  terminal_writestring("\n[");
  print_hex64(addr);
  terminal_writestring("] = ");
  print_hex64(value);
  terminal_writestring("\n");
}

void cmd_poke(const char *args) {
  if (!args || !*args) {
    terminal_writestring("\nUsage: poke <address> <value>\n");
    return;
  }

  /* Find space between address and value */
  const char *p = args;
  while (*p && *p != ' ')
    p++;
  if (!*p) {
    terminal_writestring("\nUsage: poke <address> <value>\n");
    return;
  }
  p++;

  uint64_t addr = parse_hex(args);
  uint64_t value = parse_hex(p);

  *(volatile uint64_t *)addr = value;

  terminal_writestring("\nWrote ");
  print_hex64(value);
  terminal_writestring(" to ");
  print_hex64(addr);
  terminal_writestring("\n");
}

void cmd_hexdump(const char *args) {
  if (!args || !*args) {
    terminal_writestring("\nUsage: hexdump <address>\n");
    return;
  }

  uint64_t addr = parse_hex(args);
  uint8_t *ptr = (uint8_t *)addr;

  terminal_writestring("\n");

  for (int row = 0; row < 4; row++) {
    print_hex64(addr + row * 16);
    terminal_writestring(": ");

    for (int col = 0; col < 16; col++) {
      uint8_t byte = ptr[row * 16 + col];
      char hex[3];
      hex[0] = "0123456789ABCDEF"[byte >> 4];
      hex[1] = "0123456789ABCDEF"[byte & 0xF];
      hex[2] = '\0';
      terminal_writestring(hex);
      terminal_writestring(" ");
    }

    terminal_writestring(" ");
    for (int col = 0; col < 16; col++) {
      uint8_t byte = ptr[row * 16 + col];
      char c = (byte >= 32 && byte < 127) ? byte : '.';
      terminal_putchar(c);
    }

    terminal_writestring("\n");
  }
}

void cmd_alloc(const char *args) {
  if (!args || !*args) {
    terminal_writestring("\nUsage: alloc <size>\n");
    return;
  }

  uint64_t size = parse_dec(args);
  void *ptr = kmalloc(size);

  if (ptr) {
    terminal_writestring("\nAllocated ");
    print_dec64(size);
    terminal_writestring(" bytes at ");
    print_hex64((uint64_t)ptr);
    terminal_writestring("\n");
  } else {
    terminal_writestring("\nAllocation failed!\n");
  }
}
