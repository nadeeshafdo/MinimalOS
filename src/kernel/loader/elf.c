#include "elf.h"
#include "../lib/string.h"
#include "../lib/printk.h"
#include "../mm/pmm.h"
#include "../mm/vmm.h"
#include "../process/process.h"

bool elf_validate(const void* elf_data) {
    if (elf_data == NULL) {
        return false;
    }
    
    elf64_header_t* header = (elf64_header_t*)elf_data;
    
    // Check magic number
    if (*(u32*)header->ident != ELF_MAGIC) {
        printk("[ELF] Invalid magic number: 0x%x\n", *(u32*)header->ident);
        return false;
    }
    
    // Check 64-bit class
    if (header->ident[4] != ELFCLASS64) {
        printk("[ELF] Not a 64-bit ELF\n");
        return false;
    }
    
    // Check little endian
    if (header->ident[5] != ELFDATA2LSB) {
        printk("[ELF] Not little endian\n");
        return false;
    }
    
    // Check x86-64 architecture
    if (header->machine != EM_X86_64) {
        printk("[ELF] Not x86-64 architecture (machine=%u)\n", header->machine);
        return false;
    }
    
    // Check executable or shared object
    if (header->type != ET_EXEC && header->type != ET_DYN) {
        printk("[ELF] Not executable (type=%u)\n", header->type);
        return false;
    }
    
    printk("[ELF] Valid ELF64 binary\n");
    printk("[ELF]   Type: %s\n", header->type == ET_EXEC ? "Executable" : "Shared Object");
    printk("[ELF]   Entry: 0x%lx\n", header->entry);
    printk("[ELF]   Program headers: %u\n", header->phnum);
    
    return true;
}

u64 elf_get_entry(const void* elf_data) {
    if (!elf_validate(elf_data)) {
        return 0;
    }
    
    elf64_header_t* header = (elf64_header_t*)elf_data;
    return header->entry;
}

int elf_load(process_t* proc, const void* elf_data, size_t size) {
    (void)size;  // Size is for future validation
    
    if (!elf_validate(elf_data)) {
        return -1;
    }
    
    elf64_header_t* header = (elf64_header_t*)elf_data;
    
    printk("[ELF] Loading binary into process '%s' (PID %u)\n", proc->name, proc->pid);
    
    // Iterate through program headers
    for (u16 i = 0; i < header->phnum; i++) {
        elf64_program_header_t* phdr = (elf64_program_header_t*)(
            (u8*)elf_data + header->phoff + i * header->phentsize
        );
        
        if (phdr->type != PT_LOAD) {
            continue;  // Only load PT_LOAD segments
        }
        
        printk("[ELF]   Segment %u: vaddr=0x%lx, memsz=%lu, filesz=%lu, flags=%c%c%c\n",
               i, phdr->vaddr, phdr->memsz, phdr->filesz,
               (phdr->flags & PF_R) ? 'R' : '-',
               (phdr->flags & PF_W) ? 'W' : '-',
               (phdr->flags & PF_X) ? 'X' : '-');
        
        // Calculate number of pages needed
        u64 start_page = phdr->vaddr & ~(PAGE_SIZE - 1);
        u64 end_addr = phdr->vaddr + phdr->memsz;
        u64 end_page = (end_addr + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
        u64 num_pages = (end_page - start_page) / PAGE_SIZE;
        
        // Allocate and map pages
        for (u64 p = 0; p < num_pages; p++) {
            u64 vaddr = start_page + p * PAGE_SIZE;
            uintptr frame = pmm_alloc_frame();
            
            if (frame == 0) {
                printk("[ELF] ERROR: Failed to allocate frame\n");
                return -1;
            }
            
            // Determine page flags
            u32 flags = PAGE_PRESENT | PAGE_USER;
            if (phdr->flags & PF_W) {
                flags |= PAGE_WRITE;
            }
            
            // Map page (will need to implement vmm_map_page for user space)
            vmm_map_page(proc->page_directory, vaddr, frame, flags);
        }
        
        // Copy segment data from ELF to memory
        // We must switch to the process's page directory to write to its virtual address space
        // because 0x400000 (User Virt) maps to 'frame' in User PD, 
        // but maps to Physical 0x400000 (Identity) in Kernel PD.
        
        page_directory_t* current_pd = vmm_get_current_directory();
        vmm_switch_directory(proc->page_directory);
        
        // Disable Write Protection (CR0.WP) to allow writing to RO pages (like code segments)
        u64 cr0;
        __asm__ volatile("mov %%cr0, %0" : "=r"(cr0));
        __asm__ volatile("mov %0, %%cr0" : : "r"(cr0 & ~0x10000));
        
        u8* dest = (u8*)phdr->vaddr;
        u8* src = (u8*)elf_data + phdr->offset;
        
        if (phdr->filesz > 0) {
            memcpy(dest, src, phdr->filesz);
        }
        
        // Zero out BSS section (memsz > filesz)
        if (phdr->memsz > phdr->filesz) {
            memset(dest + phdr->filesz, 0, phdr->memsz - phdr->filesz);
        }
        
        // Restore CR0 (re-enable Write Protection)
        __asm__ volatile("mov %0, %%cr0" : : "r"(cr0));
        
        vmm_switch_directory(current_pd);
    }
    
    // Setup user stack (1MB at high address)
    u64 user_stack_top = 0x7FFFFFFFE000;  // Just below 128TB
    u64 user_stack_pages = 256;  // 1MB stack
    
    printk("[ELF] Setting up user stack at 0x%lx (%lu pages)\n", 
           user_stack_top, user_stack_pages);
    
    for (u64 i = 0; i < user_stack_pages; i++) {
        uintptr frame = pmm_alloc_frame();
        if (frame == 0) {
            printk("[ELF] ERROR: Failed to allocate stack frame\n");
            return -1;
        }
        
        u64 vaddr = user_stack_top - (i + 1) * PAGE_SIZE;
        vmm_map_page(proc->page_directory, vaddr, frame, 
                     PAGE_PRESENT | PAGE_WRITE | PAGE_USER);
    }
    
    // Setup user mode context
    memset(proc->context, 0, sizeof(cpu_context_t));
    
    // We need to setup the KERNEL stack such that when context_switch returns,
    // it returns to 'enter_userspace' with the correct arguments initialized.
    
    // Setup kernel stack frame for return
    // Stack grows down from kernel_stack + KERNEL_STACK_SIZE
    // Note: proc->kernel_stack is the bottom (lowest address), so we add size
    u64* kstack_top = (u64*)(proc->kernel_stack + KERNEL_STACK_SIZE - 8);
    *kstack_top = (u64)enter_userspace;  // Return address for context_switch
    
    // Set RSP to point to the return address we just pushed
    proc->context->rsp = (u64)kstack_top;
    
    // Set arguments for enter_userspace(entry, stack)
    // context_switch restores these registers
    proc->context->rdi = header->entry;       // Arg 1: entry point
    proc->context->rsi = user_stack_top;      // Arg 2: user stack
    
    // Set other registers
    proc->context->rbp = 0;
    proc->context->rflags = 0x202;  // Interrupts enabled
    proc->context->cs = 0x08;       // Kernel code (we start in kernel wrapper)
    proc->context->ss = 0x10;       // Kernel data
    
    proc->user_stack = user_stack_top;
    process_set_state(proc, PROCESS_STATE_READY);
    
    printk("[ELF] Binary loaded successfully, entry point: 0x%lx\n", header->entry);
    printk("[ELF] Prepared transition to user mode via enter_userspace\n");
    
    return 0;
}
