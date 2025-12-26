// Minimal user-space test program
// Compiles to ELF64 to test our loader

// No libc - pure assembly/syscalls only
void _start(void) {
    // Infinite loop - proves we're in user space
    // When we add syscalls, this will do: syscall(SYS_EXIT, 42)
    while(1) {
        // Busy wait - visible as CPU usage
        __asm__ volatile("nop");
    }
}
