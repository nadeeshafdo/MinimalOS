/**
 * MinimalOS - System Call Interface
 */

#ifndef ARCH_X86_64_SYSCALL_H
#define ARCH_X86_64_SYSCALL_H

#include <minimalos/types.h>

/* System call numbers */
#define SYS_EXIT 0
#define SYS_WRITE 1
#define SYS_SLEEP 2
#define SYS_GETPID 3

/* Initialize system call interface (MSRs) */
void syscall_init(void);

#endif /* ARCH_X86_64_SYSCALL_H */
