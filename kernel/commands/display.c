/* Display commands: color, banner */
#include <stdint.h>
#include <kernel/commands.h>
#include <kernel/tty.h>

/* External helper */
extern uint32_t cmd_strlen(const char *s);

void cmd_color(const char *args) {
    char fg_str[5], bg_str[5];
    args = cmd_get_arg(args, fg_str, sizeof(fg_str));
    cmd_get_arg(args, bg_str, sizeof(bg_str));
    
    if (cmd_strlen(fg_str) == 0) {
        terminal_writestring("\nUsage: color <fg 0-15> [bg 0-15]\n");
        terminal_writestring("Colors: 0=Black 1=Blue 2=Green 3=Cyan\n");
        terminal_writestring("        4=Red 5=Magenta 6=Brown 7=LtGray\n");
        terminal_writestring("        8=DkGray 9=LtBlue 10=LtGreen 11=LtCyan\n");
        terminal_writestring("        12=LtRed 13=Pink 14=Yellow 15=White\n");
        return;
    }
    
    uint8_t fg = cmd_parse_dec(fg_str) & 0xF;
    uint8_t bg = cmd_strlen(bg_str) > 0 ? (cmd_parse_dec(bg_str) & 0xF) : VGA_COLOR_BLACK;
    
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
    terminal_writestring("              v0.1 Alpha - x86 32-bit\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}
