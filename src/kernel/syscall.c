#include "syscall.h"
#include "arch/x86_64/vga.h"
#include "arch/x86_64/keyboard.h"
#include "stdint.h"

#define SYS_READ 0
#define SYS_WRITE 1

// MSR functions
static inline uint64_t rdmsr(uint32_t msr) {
    uint32_t low, high;
    asm volatile ("rdmsr" : "=a"(low), "=d"(high) : "c"(msr));
    return ((uint64_t)high << 32) | low;
}

static inline void wrmsr(uint32_t msr, uint64_t value) {
    uint32_t low = value & 0xFFFFFFFF;
    uint32_t high = value >> 32;
    asm volatile ("wrmsr" : : "a"(low), "d"(high), "c"(msr));
}

void syscall_handler() {
    uint64_t sysno;
    asm volatile ("mov %%rax, %0" : "=r"(sysno));
    
    if (sysno == SYS_WRITE) {
        char *str;
        asm volatile ("mov %%rdi, %0" : "=r"(str));
        vga_print(str);
    } else if (sysno == SYS_READ) {
        char ch = kb_read();
        asm volatile ("mov %0, %%rax" :: "r"((uint64_t)ch));
    }
}

// Syscall entry point (assembly stub)
extern void syscall_entry(void);
asm(
    ".global syscall_entry\n"
    "syscall_entry:\n"
    "    swapgs\n"                    // Switch to kernel GS
    "    push %rax\n"                 // Save registers
    "    push %rbx\n"
    "    push %rcx\n"
    "    push %rdx\n"
    "    push %rsi\n"
    "    push %rdi\n"
    "    push %r8\n"
    "    push %r9\n"
    "    push %r10\n"
    "    push %r11\n"
    "    call syscall_handler\n"      // Call C handler
    "    pop %r11\n"                  // Restore registers
    "    pop %r10\n"
    "    pop %r9\n"
    "    pop %r8\n"
    "    pop %rdi\n"
    "    pop %rsi\n"
    "    pop %rdx\n"
    "    pop %rcx\n"
    "    pop %rbx\n"
    "    pop %rax\n"
    "    swapgs\n"                    // Switch back to user GS
    "    sysretq\n"                   // Return to user mode
);

void setup_syscalls() {
    // Set up syscall MSRs
    wrmsr(0xC0000082, (uint64_t)syscall_entry);  // LSTAR - syscall entry point
    wrmsr(0xC0000080, rdmsr(0xC0000080) | 1);    // EFER.SCE - enable syscall
    wrmsr(0xC0000081, 0x0010000800000000ULL);    // STAR - segment selectors
    wrmsr(0xC0000084, 0x200);                    // FMASK - clear IF on syscall
}