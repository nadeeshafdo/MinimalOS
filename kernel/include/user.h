#ifndef _USER_H
#define _USER_H

#include <stdint.h>

/* User code/data segment selectors (with RPL=3) */
#define USER_CODE_SEG   0x18 | 3   /* 0x1B */
#define USER_DATA_SEG   0x20 | 3   /* 0x23 */

/* Jump to user mode */
void user_mode_enter(uint64_t entry, uint64_t user_stack);

/* Simple demo user program (embedded) */
void user_demo_program(void);

#endif /* _USER_H */
