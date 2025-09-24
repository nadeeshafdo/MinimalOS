// Simple test kernel to verify boot process works
void kernel_main() {
    // Direct VGA memory access - simple and reliable
    volatile char *vga = (volatile char *)0xB8000;
    const char *msg = "KERNEL WORKS!";
    int i = 0;
    
    // Clear first line
    for (int j = 0; j < 80 * 2; j += 2) {
        vga[j] = ' ';      // Character
        vga[j + 1] = 0x07; // Attribute (light gray on black)
    }
    
    // Write message to VGA memory
    while (msg[i] && i < 80) {
        vga[i * 2] = msg[i];      // Character
        vga[i * 2 + 1] = 0x0A;    // Attribute (bright green on black)
        i++;
    }
    
    // Simple infinite loop with halt to save CPU
    while (1) {
        asm volatile("hlt");
    }
}
