#ifndef _KEYBOARD_H
#define _KEYBOARD_H

#include <stdint.h>

/* Initialize keyboard driver */
void keyboard_init(void);

/* Check if a key is available */
int keyboard_available(void);

/* Get next character from keyboard buffer (blocking) */
char keyboard_getchar(void);

/* Get next character if available, 0 otherwise (non-blocking) */
char keyboard_try_getchar(void);

#endif /* _KEYBOARD_H */
