#ifndef _TSS_H
#define _TSS_H

#include <stdint.h>

/* Task State Segment for x86_64 */
typedef struct {
    uint32_t reserved0;
    uint64_t rsp0;      /* Ring 0 stack pointer */
    uint64_t rsp1;      /* Ring 1 stack pointer */
    uint64_t rsp2;      /* Ring 2 stack pointer */
    uint64_t reserved1;
    uint64_t ist1;      /* Interrupt Stack Table 1 */
    uint64_t ist2;
    uint64_t ist3;
    uint64_t ist4;
    uint64_t ist5;
    uint64_t ist6;
    uint64_t ist7;
    uint64_t reserved2;
    uint16_t reserved3;
    uint16_t iopb_offset;
} __attribute__((packed)) tss_t;

/* Initialize TSS */
void tss_init(void);

/* Set kernel stack for Ring 0 (used when returning from Ring 3) */
void tss_set_kernel_stack(uint64_t stack);

#endif /* _TSS_H */
