/* Memory tool commands: peek, poke, hexdump, alloc */
#include <stdint.h>
#include <kernel/commands.h>
#include <kernel/tty.h>
#include <kernel/kheap.h>

/* External helper */
extern uint32_t cmd_strlen(const char *s);
extern void cmd_print_hex_byte(uint8_t value);

void cmd_peek(const char *args) {
    char addr_str[20];
    cmd_get_arg(args, addr_str, sizeof(addr_str));
    
    if (cmd_strlen(addr_str) == 0) {
        terminal_writestring("\nUsage: peek <address>\n");
        return;
    }
    
    uint32_t addr = cmd_parse_hex(addr_str);
    uint32_t value = *(volatile uint32_t*)addr;
    
    terminal_writestring("\n[");
    cmd_print_hex(addr);
    terminal_writestring("] = ");
    cmd_print_hex(value);
    terminal_writestring("\n");
}

void cmd_poke(const char *args) {
    char addr_str[20], val_str[20];
    args = cmd_get_arg(args, addr_str, sizeof(addr_str));
    cmd_get_arg(args, val_str, sizeof(val_str));
    
    if (cmd_strlen(addr_str) == 0 || cmd_strlen(val_str) == 0) {
        terminal_writestring("\nUsage: poke <address> <value>\n");
        return;
    }
    
    uint32_t addr = cmd_parse_hex(addr_str);
    uint32_t value = cmd_parse_hex(val_str);
    
    *(volatile uint32_t*)addr = value;
    terminal_writestring("\nWrote ");
    cmd_print_hex(value);
    terminal_writestring(" to ");
    cmd_print_hex(addr);
    terminal_writestring("\n");
}

void cmd_hexdump(const char *args) {
    char addr_str[20];
    cmd_get_arg(args, addr_str, sizeof(addr_str));
    
    if (cmd_strlen(addr_str) == 0) {
        terminal_writestring("\nUsage: hexdump <address>\n");
        return;
    }
    
    uint32_t addr = cmd_parse_hex(addr_str);
    terminal_writestring("\n");
    
    for (int row = 0; row < 4; row++) {
        cmd_print_hex(addr + row * 16);
        terminal_writestring(": ");
        
        for (int col = 0; col < 16; col++) {
            cmd_print_hex_byte(*(uint8_t*)(addr + row * 16 + col));
            terminal_putchar(' ');
            if (col == 7) terminal_putchar(' ');
        }
        
        terminal_writestring(" |");
        for (int col = 0; col < 16; col++) {
            char c = *(char*)(addr + row * 16 + col);
            terminal_putchar((c >= 32 && c < 127) ? c : '.');
        }
        terminal_writestring("|\n");
    }
}

void cmd_alloc(const char *args) {
    char size_str[20];
    cmd_get_arg(args, size_str, sizeof(size_str));
    
    if (cmd_strlen(size_str) == 0) {
        terminal_writestring("\nUsage: alloc <size>\n");
        return;
    }
    
    uint32_t size = cmd_parse_dec(size_str);
    void *ptr = kmalloc(size);
    
    if (ptr) {
        terminal_writestring("\nAllocated ");
        cmd_print_dec(size);
        terminal_writestring(" bytes at ");
        cmd_print_hex((uint32_t)ptr);
        terminal_writestring("\n");
    } else {
        terminal_writestring("\nAllocation failed!\n");
    }
}
