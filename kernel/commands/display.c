/* Display commands: color, banner */
#include <kernel/commands.h>
#include <kernel/tty.h>
#include <stdint.h>

/* Simple strlen */
static size_t str_len(const char *s) {
  size_t len = 0;
  while (s[len])
    len++;
  return len;
}

/* Get next argument from string */
static const char *get_arg(const char *args, char *buf, size_t bufsize) {
  size_t i = 0;

  /* Skip leading spaces */
  while (*args == ' ')
    args++;

  /* Copy until space or end */
  while (*args && *args != ' ' && i < bufsize - 1) {
    buf[i++] = *args++;
  }
  buf[i] = '\0';

  /* Skip trailing spaces */
  while (*args == ' ')
    args++;

  return args;
}

void cmd_color(const char *args) {
  char fg_str[5] = {0}, bg_str[5] = {0};

  if (!args || !*args) {
    terminal_writestring("\nUsage: color <fg 0-15> [bg 0-15]\n");
    terminal_writestring("Colors: 0=Black 1=Blue 2=Green 3=Cyan\n");
    terminal_writestring("        4=Red 5=Magenta 6=Brown 7=LtGray\n");
    terminal_writestring("        8=DkGray 9=LtBlue 10=LtGreen 11=LtCyan\n");
    terminal_writestring("        12=LtRed 13=Pink 14=Yellow 15=White\n");
    return;
  }

  args = get_arg(args, fg_str, sizeof(fg_str));
  get_arg(args, bg_str, sizeof(bg_str));

  uint8_t fg = parse_dec(fg_str) & 0xF;
  uint8_t bg =
      str_len(bg_str) > 0 ? (parse_dec(bg_str) & 0xF) : VGA_COLOR_BLACK;

  terminal_setcolor(vga_entry_color(fg, bg));
  terminal_writestring("\nColor changed!\n");
}

void cmd_banner(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n");
  terminal_writestring("  __  __ _       _             _  ___  ____  \n");
  terminal_writestring(" |  \\/  (_)_ __ (_)_ __   __ _| |/ _ \\/ ___| \n");
  terminal_writestring(" | |\\/| | | '_ \\| | '_ \\ / _` | | | | \\___ \\ \n");
  terminal_writestring(" | |  | | | | | | | | | | (_| | | |_| |___) |\n");
  terminal_writestring(" |_|  |_|_|_| |_|_|_| |_|\\__,_|_|\\___/|____/ \n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
  terminal_writestring("              v0.2 - x86_64 64-bit\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}
