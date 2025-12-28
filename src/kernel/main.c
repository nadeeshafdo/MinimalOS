/*
 * src/kernel/main.c
 * Minimal Kernel Entry
 */

#include <stdint.h>
#include <stdint.h>
#include <serial.h>
#include <vga.h>
#include <panic.h>

#if !defined(__x86_64__)
#error "This kernel requires an x86_64 compiler"
#endif

// Multiboot2 Magic Number we expect
#define MULTIBOOT2_MAGIC 0x36d76289

void kernel_main(uint64_t multiboot_addr, uint64_t magic) {
    // 1. Initialize Serial first for debugging
    serial_init();
    serial_print("\n[KERNEL] Serial initialized.\n");

    // 2. Initialize VGA
    vga_init();
    serial_print("[KERNEL] VGA initialized.\n");

    // 3. Verify Multiboot2 Magic
    if (magic != MULTIBOOT2_MAGIC) {
        serial_print("[KERNEL] CRITICAL: Invalid Multiboot2 magic!\n");
        panic("Invalid Multiboot2 Magic Number (EAX)");
    }
    serial_print("[KERNEL] Multiboot2 Magic verified.\n");

    // 4. Verify Stack Alignment (simple check)
    // We can't easily check alignment of the *frame* here in C easily without assembly.
    // But we can check if `&magic` is reasonable.
    // For now, assume boot.S set it up right.
    
    // 5. Announce Boot
    vga_set_color(0x1F, 0x00); // White on Blue
    vga_print("MinimalOS Kernel x86_64\n");
    vga_print("-----------------------\n");
    vga_set_color(0x0F, 0x00); // White on Black
    
    vga_print("Boot sequence complete.\n");
    serial_print("[KERNEL] Boot sequence complete. Halting.\n");

    /* Halt loop */
    while (1) {
        __asm__ volatile("hlt");
    }
}
