#ifndef _KERNEL_SYSCALL_H
#define _KERNEL_SYSCALL_H

#include <stdint.h>
#include <kernel/isr.h>

/* System call numbers */
#define SYS_EXIT    1
#define SYS_FORK    2
#define SYS_READ    3
#define SYS_WRITE   4
#define SYS_OPEN    5
#define SYS_CLOSE   6
#define SYS_EXECVE  11
#define SYS_YIELD   158

/* Initialize system calls */
void syscall_init(void);

/* System call handler */
void syscall_handler(struct registers *regs);

#endif /* _KERNEL_SYSCALL_H */
