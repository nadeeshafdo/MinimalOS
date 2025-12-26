# Multiboot2 Header
# This header allows GRUB to recognize and boot our kernel

.section .multiboot
.align 8

multiboot_header_start:
    # Magic number
    .long 0xE85250D6                    # Multiboot2 magic number
    
    # Architecture (0 = i386 protected mode)
    .long 0
    
    # Header length
    .long multiboot_header_end - multiboot_header_start
    
    # Checksum
    .long -(0xE85250D6 + 0 + (multiboot_header_end - multiboot_header_start))

    # End tag (required)
    .align 8
    .short 0                            # Type: end
    .short 0                            # Flags
    .long 8                             # Size
multiboot_header_end:
