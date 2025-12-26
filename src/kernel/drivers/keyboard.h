#ifndef KEYBOARD_H
#define KEYBOARD_H

#include "../include/types.h"

/**
 * Initialize PS/2 keyboard driver
 * Registers IRQ1 handler and enables keyboard interrupts
 */
void keyboard_init(void);

/**
 * Get a character from the keyboard buffer (blocking)
 * @return ASCII character
 */
char keyboard_getchar(void);

/**
 * Check if keyboard buffer has data
 * @return true if data available, false otherwise
 */
bool keyboard_has_char(void);

#endif // KEYBOARD_H
