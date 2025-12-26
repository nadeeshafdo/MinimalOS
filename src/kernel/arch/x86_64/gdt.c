#include "gdt.h"
#include "../../include/types.h"
#include "../../lib/string.h"

#define GDT_ENTRIES 7

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

static void write_tss(i32 num, u64 esp0) {
    // Determine base and limit
    u64 base = (u64)&tss;
    u64 limit = sizeof(tss);
    
    // Low 8 bytes
    gdt_set_gate(num, base, limit, 0x89, 0x00); // 0x89 = Present, Executable, Accessed
    
    // High 8 bytes (Base Upper)
    // Manually set next entry since gdt_set_gate only does 8 bytes
    // struct gdt_entry is 8 bytes.
    // We treat GDT as u64 array for simplicity or cast
    // Or just use the struct fields for the next entry
    
    // Structure of High 8 bytes for TSS:
    // Base 63:32 at offset 0
    // Reserved/Zero at offset 4
    // We can interpret the next gdt_entry as the high part
    
    gdt[num + 1].limit_low = (base >> 32) & 0xFFFF;
    gdt[num + 1].base_low = (base >> 48) & 0xFFFF;
    // other fields zero
    gdt[num + 1].base_middle = 0;
    gdt[num + 1].access = 0;
    gdt[num + 1].granularity = 0;
    gdt[num + 1].base_high = 0;
    
    // Initialize TSS structure
    memset(&tss, 0, sizeof(tss));
    tss.rsp0 = esp0;
    tss.iomap_base = sizeof(tss); // No I/O map
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
    
    // TSS segment (entry 5 and 6)
    // Initial kernel stack (will be updated by scheduler)
    write_tss(5, 0);
    
    // Load the GDT
    gdt_flush((u64)&gdt_pointer);
    
    // Load TSS (LTR)
    // Segment selector: Index 5 (0x28) | RPL 0/3? 
    // Usually LTR uses RPL 0 or 3? Intel logic:
    // "The source operand is a segment selector... RPL is ignored"
    // So 0x28.
    __asm__ volatile("ltr %%ax" : : "a"(0x28));
}

void gdt_set_kernel_stack(uintptr stack_top) {
    tss.rsp0 = stack_top;
}
