#ifndef GDT_H
#define GDT_H

#include "../../include/types.h"

// GDT Entry structure
struct gdt_entry {
    u16 limit_low;
    u16 base_low;
    u8  base_middle;
    u8  access;
    u8  granularity;
    u8  base_high;
} __attribute__((packed));

// GDT Pointer structure
struct gdt_ptr {
    u16 limit;
    u64 base;
} __attribute__((packed));

// TSS Entry for x86_64
struct tss_entry {
    u32 reserved0;
    u64 rsp0;        // Stack pointer for ring 0
    u64 rsp1;        // Stack pointer for ring 1
    u64 rsp2;        // Stack pointer for ring 2
    u64 reserved1;
    u64 ist1;        // Interrupt Stack Table entries
    u64 ist2;
    u64 ist3;
    u64 ist4;
    u64 ist5;
    u64 ist6;
    u64 ist7;
    u64 reserved2;
    u16 reserved3;
    u16 iomap_base;
} __attribute__((packed));

void gdt_init(void);
void gdt_set_kernel_stack(uintptr stack_top);

#endif // GDT_H
