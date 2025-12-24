/* System Call Implementation */

#include <stdint.h>
#include "syscall.h"
#include "process.h"
#include "timer.h"
#include "keyboard.h"

/* External VGA functions (defined in kernel.c) */
extern void putchar(char c);
extern void puts(const char *s);

/* Assembly init function */
extern void syscall_init_asm(void);

void syscall_init(void) {
    syscall_init_asm();
}

/* Write to console */
static uint64_t sys_write(uint64_t fd, const char *buf, uint64_t count) {
    (void)fd;  /* Ignore fd, always write to console */
    
    for (uint64_t i = 0; i < count; i++) {
        putchar(buf[i]);
    }
    
    return count;
}

/* Read from keyboard */
static uint64_t sys_read(uint64_t fd, char *buf, uint64_t count) {
    (void)fd;  /* Ignore fd, always read from keyboard */
    
    for (uint64_t i = 0; i < count; i++) {
        buf[i] = keyboard_getchar();
        if (buf[i] == '\n') {
            return i + 1;
        }
    }
    
    return count;
}

/* Exit current process */
static void sys_exit(uint64_t status) {
    (void)status;
    process_exit();
}

/* Get current process ID */
static uint64_t sys_getpid(void) {
    process_t *p = process_current();
    return p ? p->pid : 0;
}

/* Yield to scheduler */
static void sys_yield(void) {
    process_yield();
}

/* Sleep for milliseconds */
static void sys_sleep(uint64_t ms) {
    timer_sleep(ms);
}

/* Syscall dispatcher */
uint64_t syscall_handler(uint64_t num, uint64_t arg1, uint64_t arg2, uint64_t arg3) {
    switch (num) {
        case SYS_READ:
            return sys_read(arg1, (char *)arg2, arg3);
        
        case SYS_WRITE:
            return sys_write(arg1, (const char *)arg2, arg3);
        
        case SYS_EXIT:
            sys_exit(arg1);
            return 0;  /* Never reached */
        
        case SYS_GETPID:
            return sys_getpid();
        
        case SYS_YIELD:
            sys_yield();
            return 0;
        
        case SYS_SLEEP:
            sys_sleep(arg1);
            return 0;
        
        default:
            return (uint64_t)-1;  /* Invalid syscall */
    }
}
