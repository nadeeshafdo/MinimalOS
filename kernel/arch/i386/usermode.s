.section .text
.global enter_user_mode
.type enter_user_mode, @function

/* void enter_user_mode(void *entry_point, uint32_t user_stack_top) */
enter_user_mode:
    mov 4(%esp), %ebx    /* User EIP (entry point) */
    mov 8(%esp), %ecx    /* User ESP (stack top) */
    
    /* Disable interrupts before setting up stack */
    cli
    
    /* Set up data segments for user mode (0x20 | 3 = 0x23) */
    mov $0x23, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    
    /* Build IRET stack frame for Ring 3 transition */
    /* Stack layout for IRET to Ring 3: SS, ESP, EFLAGS, CS, EIP */
    
    push $0x23          /* SS - User Data Segment */
    push %ecx           /* ESP - User Stack Pointer */
    
    pushf               /* Get current EFLAGS */
    pop %eax
    or $0x200, %eax     /* Enable Interrupts (IF flag) */
    push %eax           /* EFLAGS */
    
    push $0x1B          /* CS - User Code Segment (0x18 | 3) */
    push %ebx           /* EIP - Entry Point */
    
    /* Jump to user mode! */
    iret
