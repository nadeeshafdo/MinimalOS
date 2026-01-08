/* Multiboot header for GRUB bootloader with framebuffer support */
.set MAGIC,    0x1BADB002          /* Multiboot magic number */
.set FLAGS,    (1<<0 | 1<<1 | 1<<2) /* Align, meminfo, video mode */
.set CHECKSUM, -(MAGIC + FLAGS)    /* Checksum required by multiboot */

/* Video mode settings */
.set MODE_TYPE, 0                  /* 0 = linear framebuffer */
.set WIDTH,     1024               /* Requested width */
.set HEIGHT,    768                /* Requested height */
.set DEPTH,     32                 /* Bits per pixel */

/* Multiboot header must be in first 8KB and aligned on 4-byte boundary */
.section .multiboot
.align 4
.long MAGIC
.long FLAGS
.long CHECKSUM
.long 0, 0, 0, 0, 0                /* Unused address fields */
.long MODE_TYPE                    /* Video mode type */
.long WIDTH                        /* Width */
.long HEIGHT                       /* Height */
.long DEPTH                        /* Depth */

/* Allocate initial kernel stack (16KB) */
.section .bss
.align 16
stack_bottom:
.skip 16384                         /* 16KB stack */
stack_top:

/* Kernel entry point */
.section .text
.global _start
.type _start, @function

_start:
    /* Set up stack pointer */
    mov $stack_top, %esp
    
    /* Reset EFLAGS */
    pushl $0
    popf
    
    /* Push multiboot info structure pointer and magic value */
    pushl %ebx      /* Multiboot info struct */
    pushl %eax      /* Multiboot magic */
    
    /* Call kernel main function */
    call kernel_main
    
    /* If kernel_main returns, halt the CPU */
    cli             /* Disable interrupts */
1:  hlt             /* Halt CPU */
    jmp 1b          /* Loop in case of NMI */

/* Set the size of _start symbol for debugging */
.size _start, . - _start
