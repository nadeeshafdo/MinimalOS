/* 64-bit Global Descriptor Table (GDT) for x86_64 */
#include <kernel/gdt.h>
#include <stdint.h>

/* GDT entries - x86_64 needs specific descriptors */
#define GDT_ENTRIES 7

static struct gdt_entry gdt[GDT_ENTRIES];
static struct gdt_ptr gdt_p;
static struct tss_entry tss_entry;

/* External assembly function to load GDT and TSS */
extern void gdt_flush(uint64_t);
extern void tss_flush(void);

/* Set a GDT entry */
static void gdt_set_gate(int32_t num, uint32_t base, uint32_t limit,
                         uint8_t access, uint8_t gran) {
  gdt[num].base_low = (base & 0xFFFF);
  gdt[num].base_middle = (base >> 16) & 0xFF;
  gdt[num].base_high = (base >> 24) & 0xFF;

  gdt[num].limit_low = (limit & 0xFFFF);
  gdt[num].granularity = ((limit >> 16) & 0x0F) | (gran & 0xF0);
  gdt[num].access = access;
}

/* Initialize TSS */
static void write_tss(int32_t num, uint64_t rsp0) {
  uint64_t base = (uint64_t)&tss_entry;
  uint32_t limit = sizeof(tss_entry) - 1;

  /* TSS descriptor in 64-bit mode is 16 bytes (two GDT entries) */
  /* Lower 8 bytes */
  gdt[num].limit_low = limit & 0xFFFF;
  gdt[num].base_low = base & 0xFFFF;
  gdt[num].base_middle = (base >> 16) & 0xFF;
  gdt[num].access = 0x89; /* Present, ring 0, TSS (available) */
  gdt[num].granularity = ((limit >> 16) & 0x0F);
  gdt[num].base_high = (base >> 24) & 0xFF;

  /* Upper 8 bytes (next entry) - contains high 32 bits of base */
  struct gdt_entry_upper *upper = (struct gdt_entry_upper *)&gdt[num + 1];
  upper->base_upper = (base >> 32) & 0xFFFFFFFF;
  upper->reserved = 0;

  /* Zero out TSS */
  uint8_t *p = (uint8_t *)&tss_entry;
  for (uint32_t i = 0; i < sizeof(tss_entry); i++) {
    p[i] = 0;
  }

  tss_entry.rsp0 = rsp0;

  /* Set IO map base to end of TSS (disable IO bitmap) */
  tss_entry.iomap_base = sizeof(tss_entry);
}

void tss_set_stack(uint64_t rsp0) { tss_entry.rsp0 = rsp0; }

void gdt_init(void) {
  gdt_p.limit = (sizeof(struct gdt_entry) * GDT_ENTRIES) - 1;
  gdt_p.base = (uint64_t)&gdt;

  /* Entry 0: NULL descriptor */
  gdt_set_gate(0, 0, 0, 0, 0);

  /* Entry 1: Kernel Code Segment (64-bit) */
  /* Base = 0, Limit = 0 (ignored in long mode)
     Access: Present, Ring 0, Code, Execute/Read
     Flags: Long mode (L=1), Granularity 4KB */
  gdt[1].limit_low = 0;
  gdt[1].base_low = 0;
  gdt[1].base_middle = 0;
  gdt[1].access =
      0x9A; /* Present, Ring 0, Code segment, executable, readable */
  gdt[1].granularity = 0x20; /* L=1 (long mode), D=0, G=0 */
  gdt[1].base_high = 0;

  /* Entry 2: Kernel Data Segment (64-bit) */
  gdt[2].limit_low = 0;
  gdt[2].base_low = 0;
  gdt[2].base_middle = 0;
  gdt[2].access = 0x92; /* Present, Ring 0, Data segment, writable */
  gdt[2].granularity = 0x00;
  gdt[2].base_high = 0;

  /* Entry 3: User Code Segment (64-bit) */
  gdt[3].limit_low = 0;
  gdt[3].base_low = 0;
  gdt[3].base_middle = 0;
  gdt[3].access =
      0xFA; /* Present, Ring 3, Code segment, executable, readable */
  gdt[3].granularity = 0x20; /* L=1 (long mode) */
  gdt[3].base_high = 0;

  /* Entry 4: User Data Segment (64-bit) */
  gdt[4].limit_low = 0;
  gdt[4].base_low = 0;
  gdt[4].base_middle = 0;
  gdt[4].access = 0xF2; /* Present, Ring 3, Data segment, writable */
  gdt[4].granularity = 0x00;
  gdt[4].base_high = 0;

  /* Entry 5-6: TSS (takes two entries in 64-bit mode) */
  write_tss(5, 0);

  /* Load the GDT */
  gdt_flush((uint64_t)&gdt_p);

  /* Load the Task Register (TR) */
  tss_flush();
}
