#include <stdint.h>
#include <kernel/syscall.h>
#include <kernel/isr.h>
#include <kernel/idt.h>
#include <kernel/tty.h>
#include <kernel/process.h>

/* Prototype for syscall functions */
typedef void (*syscall_fn)(struct registers *regs);

/* System call definitions */
static void sys_exit(struct registers *regs) {
    int status = regs->ebx;
    process_exit(status);
}

static void sys_write(struct registers *regs) {
    int fd = regs->ebx;
    const char *buf = (const char*)regs->ecx;
    size_t count = regs->edx;
    
    if (fd == 1 || fd == 2) { /* stdout or stderr */
        for (size_t i = 0; i < count; i++) {
            terminal_putchar(buf[i]);
        }
    }
}

/*
static void sys_yield(struct registers *regs) {
    (void)regs;
    // Force a task switch 
    // This requires calling the scheduler manually or triggering interrupt 
    // For now, just a placeholder 
}
*/

/* Define syscall table */
static void *syscalls[] = {
    0,           /* 0 - unused */
    sys_exit,    /* 1 - exit */
    0,           /* 2 - fork */
    0,           /* 3 - read */
    sys_write,   /* 4 - write */
    0,           /* 5 - open */
    0,           /* 6 - close */
    /* ... */
};

#define NUM_SYSCALLS (sizeof(syscalls) / sizeof(void*))

void syscall_handler(struct registers *regs) {
    /* Check if syscall number is valid */
    if (regs->eax >= NUM_SYSCALLS) {
        return;
    }
    
    /* Get syscall function */
    syscall_fn handler = syscalls[regs->eax];
    
    /* Call handler if it exists */
    if (handler) {
        handler(regs);
    }
}

void syscall_init(void) {
    /* Register syscall handler for interrupt 0x80 */
    isr_register_handler(0x80, syscall_handler);
}
