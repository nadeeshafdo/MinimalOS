/*
 * src/kernel/main.c
 * Minimal Kernel Entry
 */

#if !defined(__x86_64__)
#error "This kernel requires an x86_64 compiler"
#endif

typedef unsigned long long uint64_t;
typedef unsigned short uint16_t;
typedef unsigned char uint8_t;

/* VGA Text Buffer (Higher Half) */
#define VGA_MEMORY ((volatile uint16_t*)0xFFFFFFFF800B8000)

void kernel_main(uint64_t multiboot_addr) {
    /* Prevent compiler warning about unused var */
    (void)multiboot_addr;

    /* Write "OK" to top-left corner */
    VGA_MEMORY[0] = (0x2F << 8) | 'O'; // Green on White
    VGA_MEMORY[1] = (0x2F << 8) | 'K';

    /* Halt loop */
    while (1) {
        __asm__ volatile("hlt");
    }
}
