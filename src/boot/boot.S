# 32-bit Protected Mode Entry Point
# Handles the transition from 32-bit to 64-bit long mode

.code32
.section .text.boot

.extern kernel_main
.global _start

_start:
    # GRUB loads us here in 32-bit protected mode
    # EBX contains the physical address of the multiboot2 information structure
    
    # Save multiboot info pointer
    mov %ebx, %edi
    
    # Disable interrupts
    cli
    
    # Setup temporary stack
    mov $stack_top, %esp
    
    # Clear direction flag
    cld
    
    # Check if CPU supports long mode
    call check_long_mode
    
    # Setup paging for long mode
    call setup_paging
    
    # Load GDT
    lgdt (gdt64_pointer)
    
    # Enable PAE (Physical Address Extension)
    mov %cr4, %eax
    or $0x20, %eax                     # Set PAE bit
    mov %eax, %cr4
    
    # Load PML4 table address into CR3
    mov $pml4_table, %eax
    mov %eax, %cr3
    
    # Enable long mode in EFER MSR
    mov $0xC0000080, %ecx              # EFER MSR number
    rdmsr
    or $0x100, %eax                    # Set LME bit
    wrmsr
    
    # Enable paging and write protect
    mov %cr0, %eax
    or $0x80010000, %eax               # Set PG (paging) and WP bits
    mov %eax, %cr0
    
    # Jump to 64-bit code segment
    ljmp $0x08, $long_mode_start

check_long_mode:
    # Check if CPUID is supported
    pushfl
    pop %eax
    mov %eax, %ecx
    xor $0x200000, %eax                # Flip ID bit
    push %eax
    popfl
    pushfl
    pop %eax
    push %ecx
    popfl
    cmp %ecx, %eax
    je no_long_mode
    
    # Check for extended functions
    mov $0x80000000, %eax
    cpuid
    cmp $0x80000001, %eax
    jb no_long_mode
    
    # Check for long mode
    mov $0x80000001, %eax
    cpuid
    test $0x20000000, %edx             # LM bit in EDX
    jz no_long_mode
    
    ret

no_long_mode:
    # Print "NL" (No Long mode) to indicate error
    mov $0x4E4C, 0xB8000
    hlt

setup_paging:
    # Clear page tables - need to clear in dwords, not bytes
    mov $pml4_table, %edi
    xor %eax, %eax
    mov $1024, %ecx                    # 3 tables * 4096 bytes / 4 = 3072 dwords, round to 1024*3
    cld
    rep stosl
    
    # Setup PML4[0] -> PDPT (identity mapping only)
    mov $pdp_table, %eax
    or $0x03, %eax                     # Present + Writable
    mov %eax, pml4_table
    
    # Setup PDPT[0] -> PD
    mov $pd_table, %eax
    or $0x03, %eax
    mov %eax, pdp_table
    
    # Identity map first 8MB using 2MB pages (4 entries) to cover multiboot data
    mov $pd_table, %edi
    mov $0x83, %eax                    # Present + Writable + Huge (2MB page)
    mov $4, %ecx                       # 4 entries = 8MB
    
.map_loop:
    mov %eax, (%edi)
    add $0x200000, %eax                # Next 2MB
    add $8, %edi
    loop .map_loop
    
    ret

# Temporary GDT for 64-bit mode
.align 16
gdt64:
    .quad 0x0000000000000000           # Null descriptor
    .quad 0x00209A0000000000           # Code segment (exec/read)
    .quad 0x0000920000000000           # Data segment (read/write)
gdt64_end:

gdt64_pointer:
    .word gdt64_end - gdt64 - 1
    .long gdt64

# Page tables (aligned to 4KB)
.section .bss
.align 4096
pml4_table:
    .skip 4096
pdp_table:
    .skip 4096
pd_table:
    .skip 4096

# Temporary boot stack (16KB)
.section .bss
.align 16
stack_bottom:
    .skip 16384
stack_top:
