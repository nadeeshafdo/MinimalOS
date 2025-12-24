/* Task State Segment (TSS) for x86_64 */

#include <stdint.h>
#include "tss.h"
#include "kheap.h"

/* TSS instance */
static tss_t tss __attribute__((aligned(16)));

/* Kernel stack for returning from Ring 3 */
static uint64_t kernel_stack = 0;

/* GDT entries (defined in boot.asm, we need to add TSS descriptor) */
extern void gdt_load_tss(uint64_t tss_addr);

void tss_init(void) {
    /* Clear TSS */
    uint8_t *p = (uint8_t *)&tss;
    for (int i = 0; i < (int)sizeof(tss_t); i++) p[i] = 0;
    
    /* Allocate kernel stack for Ring 3 returns */
    kernel_stack = (uint64_t)kmalloc(16384) + 16384;  /* 16KB stack, top */
    
    tss.rsp0 = kernel_stack;
    tss.iopb_offset = sizeof(tss_t);  /* No I/O permission bitmap */
    
    /* Load TSS into GDT and load TR */
    gdt_load_tss((uint64_t)&tss);
}

void tss_set_kernel_stack(uint64_t stack) {
    tss.rsp0 = stack;
}
