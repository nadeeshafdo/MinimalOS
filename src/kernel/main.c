// External function declarations
extern void setup_gdt(void);
extern void setup_idt(void);
extern void setup_tss(void);
extern void vga_init(void);
extern void setup_keyboard(void);
extern void setup_syscalls(void);
extern void user_shell_main(void);

// VGA output functions
extern void vga_print(const char *str);
extern void vga_set_color(unsigned char color);

// Kernel main - initialize subsystems and start shell
void kernel_main() {
    // Initialize VGA first so we can display messages
    vga_init();
    
    vga_set_color(0x0A); // Light green
    vga_print("MinimalOS v2.0 - Booting...\n\n");
    
    vga_set_color(0x07); // Light grey
    vga_print("[*] Setting up GDT...\n");
    setup_gdt();
    
    vga_print("[*] Setting up IDT...\n");
    setup_idt();
    
    vga_print("[*] Setting up TSS...\n");
    setup_tss();
    
    vga_print("[*] Initializing keyboard driver...\n");
    setup_keyboard();
    
    vga_print("[*] Setting up system calls...\n");
    setup_syscalls();
    
    vga_set_color(0x0A); // Light green
    vga_print("\n[OK] All subsystems initialized successfully!\n\n");
    
    vga_set_color(0x0E); // Yellow
    vga_print("Starting shell...\n\n");
    
    // Transfer control to shell (never returns)
    user_shell_main();
    
    // Should never reach here
    vga_set_color(0x0C); // Light red
    vga_print("ERROR: Shell returned unexpectedly!\n");
    while (1) {
        asm volatile("hlt");
    }
}
