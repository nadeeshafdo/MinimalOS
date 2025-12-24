/* User Mode Support */

#include <stdint.h>
#include "user.h"
#include "syscall.h"
#include "kheap.h"
#include "serial.h"

/* User stack size */
#define USER_STACK_SIZE (64 * 1024)  /* 64KB */

/* External assembly function */
extern void user_mode_enter(uint64_t entry, uint64_t user_stack);

/* Simple demo user program that runs in Ring 3 */
/* This uses syscalls to communicate with kernel */
void user_demo_program(void) {
    /* In Ring 3, we can only use syscalls */
    /* SYS_WRITE = 1, fd=1, buf, len */
    const char *msg = "Hello from Ring 3 userspace!\n";
    
    /* Count string length */
    int len = 0;
    while (msg[len]) len++;
    
    /* Make syscall to write */
    __asm__ volatile (
        "mov $1, %%rax\n"       /* SYS_WRITE */
        "mov $1, %%rdi\n"       /* fd = stdout */
        "mov %0, %%rsi\n"       /* buffer */
        "mov %1, %%rdx\n"       /* length */
        "syscall\n"
        :
        : "r"(msg), "r"((uint64_t)len)
        : "rax", "rdi", "rsi", "rdx", "rcx", "r11", "memory"
    );
    
    /* Get our PID */
    uint64_t pid;
    __asm__ volatile (
        "mov $3, %%rax\n"       /* SYS_GETPID */
        "syscall\n"
        : "=a"(pid)
        :
        : "rcx", "r11", "memory"
    );
    
    /* Exit with status 0 */
    __asm__ volatile (
        "mov $2, %%rax\n"       /* SYS_EXIT */
        "mov $0, %%rdi\n"       /* status = 0 */
        "syscall\n"
        :
        :
        : "rax", "rdi", "rcx", "r11", "memory"
    );
    
    /* Should not reach here */
    while (1);
}

/* Create and run a user process */
void user_process_create_and_run(void (*entry)(void)) {
    /* Allocate user stack */
    uint64_t user_stack = (uint64_t)kmalloc(USER_STACK_SIZE);
    if (!user_stack) {
        serial_debug("Failed to allocate user stack");
        return;
    }
    
    /* Stack grows down, so point to top */
    uint64_t user_stack_top = user_stack + USER_STACK_SIZE;
    
    serial_debug("Entering user mode...");
    
    /* Enter user mode */
    user_mode_enter((uint64_t)entry, user_stack_top);
    
    /* Should not return */
}
