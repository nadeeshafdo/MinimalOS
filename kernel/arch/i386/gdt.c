#include <stdint.h>
#include <kernel/gdt.h>

/* GDT entries */
#define GDT_ENTRIES 6
static struct gdt_entry gdt[GDT_ENTRIES];
static struct gdt_ptr gdt_p;
static struct tss_entry tss_entry;

/* External assembly function to load GDT and TR */
extern void gdt_flush(uint32_t);
extern void tss_flush(void);

/* Set a GDT entry */
static void gdt_set_gate(int32_t num, uint32_t base, uint32_t limit, uint8_t access, uint8_t gran) {
    gdt[num].base_low = (base & 0xFFFF);
    gdt[num].base_middle = (base >> 16) & 0xFF;
    gdt[num].base_high = (base >> 24) & 0xFF;
    
    gdt[num].limit_low = (limit & 0xFFFF);
    gdt[num].granularity = ((limit >> 16) & 0x0F) | (gran & 0xF0);
    gdt[num].access = access;
}

/* Initialize TSS */
static void write_tss(int32_t num, uint16_t ss0, uint32_t esp0) {
    uint32_t base = (uint32_t)&tss_entry;
    uint32_t limit = sizeof(tss_entry);
    
    /* Add TSS descriptor to GDT */
    gdt_set_gate(num, base, limit, 0xE9, 0x00);
    
    /* Zero out TSS */
    uint8_t *p = (uint8_t*)&tss_entry;
    for (uint32_t i = 0; i < sizeof(tss_entry); i++) {
        p[i] = 0;
    }
    
    tss_entry.ss0 = ss0;
    tss_entry.esp0 = esp0;
    
    /* Set IO map base to end of TSS (disable IO bitmap) */
    tss_entry.iomap_base = sizeof(tss_entry);
}

void tss_set_stack(uint32_t esp0) {
    tss_entry.esp0 = esp0;
}

void gdt_init(void) {
    gdt_p.limit = (sizeof(struct gdt_entry) * GDT_ENTRIES) - 1;
    gdt_p.base = (uint32_t)&gdt;
    
    /* NULL descriptor */
    gdt_set_gate(0, 0, 0, 0, 0);
    
    /* Code segment - kernel mode (ring 0) */
    /* Base = 0, Limit = 0xFFFFFFFF
       Access: Present, Ring 0, Code, Execute/Read
       Granularity: 4KB blocks, 32-bit */
    gdt_set_gate(1, 0, 0xFFFFFFFF, 0x9A, 0xCF);
    
    /* Data segment - kernel mode (ring 0) */
    gdt_set_gate(2, 0, 0xFFFFFFFF, 0x92, 0xCF);
    
    /* Code segment - user mode (ring 3) */
    gdt_set_gate(3, 0, 0xFFFFFFFF, 0xFA, 0xCF);
    
    /* Data segment - user mode (ring 3) */
    gdt_set_gate(4, 0, 0xFFFFFFFF, 0xF2, 0xCF);
    
    /* TSS segment */
    write_tss(5, 0x10, 0);  /* Kernel data segment is 0x10 */
    
    /* Load the GDT */
    gdt_flush((uint32_t)&gdt_p);
    
    /* Load the Task Register (TR) */
    tss_flush();
}
