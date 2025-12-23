#include <stdint.h>
#include <kernel/paging.h>
#include <kernel/pmm.h>
#include <kernel/isr.h>
#include <kernel/tty.h>

/* External symbols from linker */
extern uint32_t __kernel_start;
extern uint32_t __kernel_end;

/* Current page directory */
static page_directory_t *current_directory = 0;
static page_directory_t *kernel_directory = 0;

/* Page tables for kernel space (first 4MB identity mapped) */
static page_table_t kernel_page_tables[4] __attribute__((aligned(4096)));

/* Load page directory into CR3 */
static inline void load_page_directory(page_directory_t *dir) {
    __asm__ volatile ("mov %0, %%cr3" : : "r"((uint32_t)dir));
}

/* Enable paging in CR0 */
static inline void enable_paging(void) {
    uint32_t cr0;
    __asm__ volatile ("mov %%cr0, %0" : "=r"(cr0));
    cr0 |= 0x80000000;  /* Set PG bit */
    __asm__ volatile ("mov %0, %%cr0" : : "r"(cr0));
}

/* Page fault handler */
void page_fault_handler(struct registers *regs) {
    uint32_t faulting_address;
    __asm__ volatile ("mov %%cr2, %0" : "=r"(faulting_address));
    
    int present = !(regs->err_code & 0x1);
    int rw = regs->err_code & 0x2;
    int us = regs->err_code & 0x4;
    int reserved = regs->err_code & 0x8;
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
    terminal_writestring("\n*** PAGE FAULT ***\n");
    terminal_writestring("Faulting address: 0x");
    
    /* Print hex address */
    char hex[9];
    const char *digits = "0123456789ABCDEF";
    for (int i = 7; i >= 0; i--) {
        hex[i] = digits[faulting_address & 0xF];
        faulting_address >>= 4;
    }
    hex[8] = '\0';
    terminal_writestring(hex);
    terminal_writestring("\n");
    
    if (present) terminal_writestring("  - Page not present\n");
    if (rw) terminal_writestring("  - Write operation\n");
    if (us) terminal_writestring("  - User mode\n");
    if (reserved) terminal_writestring("  - Reserved bits set\n");
    
    terminal_writestring("System halted.\n");
    while(1) { __asm__ volatile("cli; hlt"); }
}

void paging_init(void) {
    /* Allocate page directory */
    kernel_directory = (page_directory_t*)pmm_alloc_frame();
    if (!kernel_directory) {
        terminal_writestring("Failed to allocate page directory!\n");
        return;
    }
    
    /* Clear page directory */
    for (int i = 0; i < 1024; i++) {
        kernel_directory->entries[i] = 0x00000002;  /* Not present, writable */
    }
    
    /* Identity map first 16MB (kernel space) */
    /* Note: WE ARE MAPPING EVERYTHING AS USER ACCESSIBLE FOR NOW TO SIMPLIFY TESTING */
    /* In a real OS, we would separate kernel and user space strictly. */
    for (int i = 0; i < 4; i++) {
        /* Set up page table */
        for (int j = 0; j < 1024; j++) {
            uint32_t addr = (i * 1024 + j) * PAGE_SIZE;
            kernel_page_tables[i].entries[j] = addr | PAGE_PRESENT | PAGE_WRITE | PAGE_USER;
        }
        
        /* Add page table to directory */
        kernel_directory->entries[i] = (uint32_t)&kernel_page_tables[i] | PAGE_PRESENT | PAGE_WRITE | PAGE_USER;
    }
    
    /* Register page fault handler */
    isr_register_handler(14, page_fault_handler);
    
    /* Switch to kernel page directory */
    current_directory = kernel_directory;
    load_page_directory(kernel_directory);
    
    /* Enable paging */
    enable_paging();
}

void paging_map(uint32_t virtual_addr, uint32_t physical_addr, uint32_t flags) {
    uint32_t pd_index = virtual_addr >> 22;
    uint32_t pt_index = (virtual_addr >> 12) & 0x3FF;
    
    page_table_t *table;
    
    /* Check if page table exists */
    if (!(current_directory->entries[pd_index] & PAGE_PRESENT)) {
        /* Allocate new page table */
        table = (page_table_t*)pmm_alloc_frame();
        if (!table) return;
        
        /* Clear page table */
        for (int i = 0; i < 1024; i++) {
            table->entries[i] = 0;
        }
        
        current_directory->entries[pd_index] = (uint32_t)table | PAGE_PRESENT | PAGE_WRITE | flags;
    } else {
        table = (page_table_t*)(current_directory->entries[pd_index] & 0xFFFFF000);
    }
    
    /* Map the page */
    table->entries[pt_index] = (physical_addr & 0xFFFFF000) | PAGE_PRESENT | flags;
    
    /* Flush TLB for this address */
    paging_flush_tlb(virtual_addr);
}

void paging_unmap(uint32_t virtual_addr) {
    uint32_t pd_index = virtual_addr >> 22;
    uint32_t pt_index = (virtual_addr >> 12) & 0x3FF;
    
    if (!(current_directory->entries[pd_index] & PAGE_PRESENT)) {
        return;
    }
    
    page_table_t *table = (page_table_t*)(current_directory->entries[pd_index] & 0xFFFFF000);
    table->entries[pt_index] = 0;
    
    paging_flush_tlb(virtual_addr);
}

uint32_t paging_get_physical(uint32_t virtual_addr) {
    uint32_t pd_index = virtual_addr >> 22;
    uint32_t pt_index = (virtual_addr >> 12) & 0x3FF;
    uint32_t offset = virtual_addr & 0xFFF;
    
    if (!(current_directory->entries[pd_index] & PAGE_PRESENT)) {
        return 0;
    }
    
    page_table_t *table = (page_table_t*)(current_directory->entries[pd_index] & 0xFFFFF000);
    
    if (!(table->entries[pt_index] & PAGE_PRESENT)) {
        return 0;
    }
    
    return (table->entries[pt_index] & 0xFFFFF000) + offset;
}

void paging_switch_directory(page_directory_t *dir) {
    current_directory = dir;
    load_page_directory(dir);
}

page_directory_t *paging_get_directory(void) {
    return current_directory;
}
