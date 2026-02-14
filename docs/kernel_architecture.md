# Kernel Architecture

## Overview
MinimalOS is a 64-bit x86_64 operating system kernel that relies on the Limine bootloader for BIOS and UEFI support. The kernel is designed to be simple, utilizing a synchronous initialization process followed by a task scheduler.

## Boot Flow
The boot process is handled by the Limine bootloader, which performs the following steps:
1. Loads the kernel into memory.
2. Sets up 64-bit Long Mode.
3. Provides memory map, framebuffer, and HHDM (Higher Half Direct Map) information via the Limine protocol.
4. Jumps to the kernel entry point `kmain`.

### Kernel Entry (`kmain`)
Located in `kernel/kernel.c`, the `kmain` function serves as the central orchestration point for system initialization.
1. **Limine Handshake**: Verifies the implementation of the Limine Base Revision.
2. **HHDM Retrieval**: Fetches the offset for Higher Half Direct Map to facilitate physical memory access.
3. **Framebuffer Initialization**: Sets up the VESA framebuffer for graphical text output.
4. **GDT/IDT Setup**: Initializes the Global Descriptor Table and Interrupt Descriptor Table.
5. **Interrupts**: Configures ISRs (Interrupt Service Routines) and IRQs (Interrupt Requests) via the PIC (Programmable Interrupt Controller).
6. **Drivers**: Initializes Timer (PIT) and Keyboard (PS/2).
7. **Memory**: Sets up the Physical Memory Manager (PMM) and Kernel Heap.
8. **Scheduler**: Initializes process management and starts the first shell task.
9. **Idle Loop**: Enters a halt loop if the scheduler returns (though the scheduler typically takes over).

## CPU Architecture Setup

### Global Descriptor Table (GDT)
The GDT is configured in `kernel/arch/x86_64/gdt.c`. It defines the following segments:
- **Null Descriptor**: Required 0x00 entry.
- **Kernel Code (0x08)**: 64-bit code segment, ring 0.
- **Kernel Data (0x10)**: 64-bit data segment, ring 0.
- **User Code (0x18)**: 64-bit code segment, ring 3.
- **User Data (0x20)**: 64-bit data segment, ring 3.
- **TSS (Task State Segment)**: Used for stack switching during interrupts in user mode.

### Interrupt Descriptor Table (IDT)
The IDT is set up in `kernel/arch/x86_64/idt.c` and supports 256 entries:
- **0-31**: CPU Exceptions (ISRs).
- **32-47**: Hardware Interrupts (IRQs) remapped from the PIC.
- **128 (0x80)**: System Call handler.

### Interrupt Handling
- **ISR**: Handles CPU exceptions (e.g., Page Fault, General Protection Fault). Defined in `kernel/arch/x86_64/isr.c`.
- **IRQ**: Handles hardware events. The PIC is remapped to offset 32 to avoid conflicts with CPU exceptions.
  - **IRQ 0**: Timer (100Hz).
  - **IRQ 1**: Keyboard.
