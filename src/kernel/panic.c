#include <panic.h>
#include <serial.h>
#include <vga.h>

void panic(const char* message) {
    // Disable interrupts
    __asm__ volatile ("cli");

    // Output to Serial (Host)
    serial_print("\n!!! KERNEL PANIC !!!\n");
    serial_print("Reason: ");
    serial_print(message);
    serial_print("\nCenter halting.\n");

    // Output to VGA (User)
    vga_set_color(0x4F, 0x00); // White on Red
    vga_print("\n!!! KERNEL PANIC !!!\n");
    vga_print("Reason: ");
    vga_print(message);
    vga_print("\nSystem Halted.");

    // Halt loop
    while (1) {
        __asm__ volatile ("hlt");
    }
}
