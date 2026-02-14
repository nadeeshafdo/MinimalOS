# Drivers

## Overview
MinimalOS includes basic drivers for essential hardware: video output, keyboard input, and system timing.

## VESA Framebuffer
The framebuffer driver enables graphical output by writing directly to video memory.
- **Initialization**: Retrieves framebuffer information (address, resolution, pitch, bpp) from the Limine bootloader response.
- **Features**:
  - `fb_putpixel`: Writes a 32-bit color value to a specific (x, y) coordinate.
  - `fb_clear`: Fills the entire screen with a color.
  - `fb_scroll`: Moves screen content up to make room for new text (terminal scrolling).
- **Text Rendering**: The `tty` module layers on top of the framebuffer, using a bitmap font to render characters.

## PS/2 Keyboard
The keyboard driver handles user input via the PS/2 controller.
- **Type**: Interrupt-driven (IRQ 1).
- **Communication**: Reads scancodes from I/O port `0x60`.
- **Scancode Set**: Key release is detected by checking the high bit (0x80).
- **Features**:
  - Maintains a circular buffer for input characters.
  - Handles Shift key state for upper/lower case characters.
  - Converts raw scancodes to ASCII.
  - Feeds input directly to the shell.

## Programmable Interval Timer (PIT)
The PIT is used to generate periodic system ticks for the scheduler.
- **Type**: Interrupt-driven (IRQ 0).
- **Configuration**:
  - Channel 0.
  - Frequency: 100 Hz.
  - Mode: Rate Generator.
- **Functionality**:
  - Updates the system tick counter.
  - Triggers `scheduler_tick()` to drive process preemption.
