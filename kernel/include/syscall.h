#ifndef _SYSCALL_H
#define _SYSCALL_H

#include <stdint.h>

/* Syscall numbers */
#define SYS_READ    0
#define SYS_WRITE   1
#define SYS_EXIT    2
#define SYS_GETPID  3
#define SYS_YIELD   4
#define SYS_SLEEP   5

/* Initialize syscall interface */
void syscall_init(void);

/* Syscall handler (called from assembly) */
uint64_t syscall_handler(uint64_t num, uint64_t arg1, uint64_t arg2, uint64_t arg3);

#endif /* _SYSCALL_H */
