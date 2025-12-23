#ifndef _KERNEL_SHELL_H
#define _KERNEL_SHELL_H

#include <stdint.h>

/* Initialize the shell */
void shell_init(void);

/* Process a character from keyboard */
void shell_input(char c);

/* Run the shell main loop */
void shell_run(void);

#endif /* _KERNEL_SHELL_H */
