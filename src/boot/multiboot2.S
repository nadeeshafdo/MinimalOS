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

    # Framebuffer tag (optional)
    .align 8
framebuffer_tag_start:
    .short 5                            # Type: framebuffer
    .short 0                            # Flags
    .long framebuffer_tag_end - framebuffer_tag_start
    .long 1024                          # Width
    .long 768                           # Height
    .long 32                            # Depth (bits per pixel)
framebuffer_tag_end:

    # End tag (required)
    .align 8
    .short 0                            # Type: end
    .short 0                            # Flags
    .long 8                             # Size
multiboot_header_end:
