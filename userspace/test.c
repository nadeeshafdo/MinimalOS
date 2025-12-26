// Minimal user-space test program with syscalls

void _start(void) {
    // Test write syscall (Num 1)
    const char* msg = "Hello from User Space via Syscall!\n";
    
    // syscall(1, 0, msg, 0)
    // RAX=1, RDI=0, RSI=msg
    __asm__ volatile(
        "mov $1, %%rax\n"
        "mov $0, %%rdi\n"
        "mov %0, %%rsi\n"
        "syscall\n"
        : 
        : "r"(msg) 
        : "rax", "rdi", "rsi", "rcx", "r11"
    );
    
    // Test exit syscall (Num 60)
    // syscall(60, 0, 0, 0)
    // RAX=60, RDI=0 (exit code)
    __asm__ volatile(
        "mov $60, %%rax\n"
        "mov $0, %%rdi\n"
        "syscall\n"
        : 
        : 
        : "rax", "rdi", "rcx", "r11"
    );
    
    while(1);
}
