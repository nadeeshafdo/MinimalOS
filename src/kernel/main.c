// Ultra-simple kernel for debugging - no function calls
void kernel_main() {
    // Direct VGA memory access - write a test pattern
    volatile char *vga = (volatile char *)0xB8000;
    
    // Write a distinctive pattern that should be visible
    vga[0] = 'K';   vga[1] = 0x4F;  // 'K' in white on red
    vga[2] = 'E';   vga[3] = 0x4F;  // 'E' in white on red  
    vga[4] = 'R';   vga[5] = 0x4F;  // 'R' in white on red
    vga[6] = 'N';   vga[7] = 0x4F;  // 'N' in white on red
    vga[8] = 'E';   vga[9] = 0x4F;  // 'E' in white on red
    vga[10] = 'L';  vga[11] = 0x4F; // 'L' in white on red
    
    // Fill more of the screen for visibility
    for (int i = 12; i < 80 * 2; i += 2) {
        vga[i] = '!';       // Character
        vga[i + 1] = 0x2F;  // Green on white - very visible
    }
    
    // Infinite loop - no function calls, just halt
    while (1) {
        __asm__ volatile ("hlt");
    }
}
