#include "gdt.h"
#include "../../include/types.h"

#define GDT_ENTRIES 6

static struct gdt_entry gdt[GDT_ENTRIES];
static struct gdt_ptr gdt_pointer;
static struct tss_entry tss;

// Assembly function to load GDT
extern void gdt_flush(u64 gdt_ptr_addr);

static void gdt_set_gate(i32 num, u64 base, u32 limit, u8 access, u8 gran) {
    gdt[num].base_low = (base & 0xFFFF);
    gdt[num].base_middle = (base >> 16) & 0xFF;
    gdt[num].base_high = (base >> 24) & 0xFF;
    
    gdt[num].limit_low = (limit & 0xFFFF);
    gdt[num].granularity = (limit >> 16) & 0x0F;
    gdt[num].granularity |= gran & 0xF0;
    gdt[num].access = access;
}

void gdt_init(void) {
    // Setup GDT pointer
    gdt_pointer.limit = (sizeof(struct gdt_entry) * GDT_ENTRIES) - 1;
    gdt_pointer.base = (u64)&gdt;
    
    // Null descriptor
    gdt_set_gate(0, 0, 0, 0, 0);
    
    // Kernel code segment (0x08)
    // Base = 0, Limit = 0xFFFFF, Access = 0x9A (present, ring 0, executable, readable)
    // Granularity = 0xA0 (64-bit, 4K granularity)
    gdt_set_gate(1, 0, 0xFFFFFFFF, 0x9A, 0xA0);
    
    // Kernel data segment (0x10)
    // Base = 0, Limit = 0xFFFFF, Access = 0x92 (present, ring 0, writable)
    // Granularity = 0xC0 (4K granularity)
    gdt_set_gate(2, 0, 0xFFFFFFFF, 0x92, 0xC0);
    
    // User code segment (0x18)
    // Access = 0xFA (present, ring 3, executable, readable)
    gdt_set_gate(3, 0, 0xFFFFFFFF, 0xFA, 0xA0);
    
    // User data segment (0x20)
    // Access = 0xF2 (present, ring 3, writable)
    gdt_set_gate(4, 0, 0xFFFFFFFF, 0xF2, 0xC0);
    
    // TSS segment will be setup later (entry 5)
    
    // Load the GDT
    gdt_flush((u64)&gdt_pointer);
}

void gdt_set_kernel_stack(uintptr stack_top) {
    tss.rsp0 = stack_top;
}
