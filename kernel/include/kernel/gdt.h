/* GDT header for x86_64 */
#ifndef KERNEL_GDT_H
#define KERNEL_GDT_H

#include <stdint.h>

/* GDT entry structure (8 bytes) */
struct gdt_entry {
  uint16_t limit_low;
  uint16_t base_low;
  uint8_t base_middle;
  uint8_t access;
  uint8_t granularity;
  uint8_t base_high;
} __attribute__((packed));

/* Upper portion of TSS descriptor (for 64-bit mode) */
struct gdt_entry_upper {
  uint32_t base_upper;
  uint32_t reserved;
} __attribute__((packed));

/* GDT pointer structure */
struct gdt_ptr {
  uint16_t limit;
  uint64_t base;
} __attribute__((packed));

/* TSS entry structure for x86_64 */
struct tss_entry {
  uint32_t reserved0;
  uint64_t rsp0; /* Stack pointer for ring 0 */
  uint64_t rsp1; /* Stack pointer for ring 1 */
  uint64_t rsp2; /* Stack pointer for ring 2 */
  uint64_t reserved1;
  uint64_t ist1; /* Interrupt Stack Table 1 */
  uint64_t ist2;
  uint64_t ist3;
  uint64_t ist4;
  uint64_t ist5;
  uint64_t ist6;
  uint64_t ist7;
  uint64_t reserved2;
  uint16_t reserved3;
  uint16_t iomap_base;
} __attribute__((packed));

/* Segment selectors */
#define GDT_KERNEL_CODE 0x08
#define GDT_KERNEL_DATA 0x10
#define GDT_USER_CODE 0x18
#define GDT_USER_DATA 0x20
#define GDT_TSS 0x28

/* Functions */
void gdt_init(void);
void tss_set_stack(uint64_t rsp0);

#endif
