#ifndef SYSCALL_H
#define SYSCALL_H

#include "../../include/types.h"

void syscall_init(void);
void syscall_set_kernel_stack(uintptr stack_top);

#endif // SYSCALL_H
