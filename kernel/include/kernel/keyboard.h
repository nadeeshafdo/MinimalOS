#ifndef _KERNEL_KEYBOARD_H
#define _KERNEL_KEYBOARD_H

#include <stdint.h>

/* Initialize keyboard driver */
void keyboard_init(void);

/* Get character from keyboard buffer (0 if empty) */
char keyboard_getchar(void);

#endif /* _KERNEL_KEYBOARD_H */
