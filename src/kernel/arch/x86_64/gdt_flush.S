# GDT Assembly Helper
.section .text
.global gdt_flush

gdt_flush:
    # RDI contains the address of the GDT pointer
    lgdt (%rdi)
    
    # Reload code segment by doing a far return
    mov $0x08, %ax
    push %rax
    lea reload_cs(%rip), %rax
    push %rax
    lretq
    
reload_cs:
    # Reload data segments
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    ret
