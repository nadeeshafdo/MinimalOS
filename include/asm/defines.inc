; MinimalOS - Assembly Constants and Definitions

; Multiboot2 constants (values only, header in multiboot2.asm)
MULTIBOOT2_MAGIC        equ 0xE85250D6
MULTIBOOT2_ARCH_I386    equ 0

; Expected magic from bootloader
MULTIBOOT2_BOOTLOADER_MAGIC equ 0x36D76289

; GDT Selectors
NULL_SEL                equ 0x00
CODE32_SEL              equ 0x08
DATA32_SEL              equ 0x10
CODE64_SEL              equ 0x18
DATA64_SEL              equ 0x20
USER_DATA64_SEL         equ 0x28 | 3     ; Ring 3
USER_CODE64_SEL         equ 0x30 | 3     ; Ring 3
TSS_SEL                 equ 0x38

; Control register bits
CR0_PE                  equ (1 << 0)     ; Protected mode enable
CR0_MP                  equ (1 << 1)     ; Monitor coprocessor
CR0_EM                  equ (1 << 2)     ; Emulation
CR0_TS                  equ (1 << 3)     ; Task switched
CR0_ET                  equ (1 << 4)     ; Extension type
CR0_NE                  equ (1 << 5)     ; Numeric error
CR0_WP                  equ (1 << 16)    ; Write protect
CR0_AM                  equ (1 << 18)    ; Alignment mask
CR0_NW                  equ (1 << 29)    ; Not write-through
CR0_CD                  equ (1 << 30)    ; Cache disable
CR0_PG                  equ (1 << 31)    ; Paging

CR4_PAE                 equ (1 << 5)     ; PAE enable
CR4_PGE                 equ (1 << 7)     ; Page global enable
CR4_OSFXSR              equ (1 << 9)     ; FXSAVE/FXRSTOR support
CR4_OSXMMEXCPT          equ (1 << 10)    ; SIMD exceptions
CR4_OSXSAVE             equ (1 << 18)    ; XSAVE enable

; Model Specific Registers (MSRs)
IA32_EFER               equ 0xC0000080
IA32_EFER_SCE           equ (1 << 0)     ; System call extensions
IA32_EFER_LME           equ (1 << 8)     ; Long mode enable
IA32_EFER_LMA           equ (1 << 10)    ; Long mode active
IA32_EFER_NXE           equ (1 << 11)    ; Execute disable enable

IA32_STAR               equ 0xC0000081
IA32_LSTAR              equ 0xC0000082
IA32_CSTAR              equ 0xC0000083
IA32_FMASK              equ 0xC0000084

IA32_FS_BASE            equ 0xC0000100
IA32_GS_BASE            equ 0xC0000101
IA32_KERNEL_GS_BASE     equ 0xC0000102

IA32_APIC_BASE          equ 0x1B

; Page table flags
PTE_PRESENT             equ (1 << 0)
PTE_WRITABLE            equ (1 << 1)
PTE_USER                equ (1 << 2)
PTE_WRITETHROUGH        equ (1 << 3)
PTE_NOCACHE             equ (1 << 4)
PTE_ACCESSED            equ (1 << 5)
PTE_DIRTY               equ (1 << 6)
PTE_HUGE                equ (1 << 7)
PTE_GLOBAL              equ (1 << 8)

; Kernel addresses
KERNEL_VMA              equ 0xFFFFFFFF80000000
KERNEL_PHYS_BASE        equ 0x100000

; Stack size (16KB)
KERNEL_STACK_SIZE       equ 0x4000
