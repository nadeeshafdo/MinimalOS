// =============================================================================
// serial_drv — Ring 3 COM1 Serial Port Driver
// =============================================================================
//
// This is the first userspace driver for MinimalOS NextGen. It runs in
// Ring 3 (CPL=3) and drives the 16550 UART (COM1) using capability-gated
// syscalls provided by libmnos.
//
// CAPABILITY LAYOUT (set up by the kernel before spawn):
//   Slot 0: IoPort { base: 0x3F8, size: 8 } — COM1 registers (READ | WRITE)
//   Slot 1: Endpoint { id: 2 }              — Command channel (READ)
//   Slot 2: Interrupt { irq: 4 }            — COM1 IRQ (optional, for RX)
//
// ARCHITECTURE:
//   1. On startup, writes a hello banner to COM1 via SYS_PORT_OUT
//   2. Enters a command loop: receives IPC messages via SYS_RECV
//   3. Each message carries a character to write to COM1
//   4. Writes the character using the polled TX path
//
// This proves the full microkernel pipeline:
//   Ring 3 user code → SYSCALL → capability validation → I/O port access
//   Ring 3 user code → SYSCALL → capability validation → IPC receive
//
// =============================================================================

#![no_std]
#![no_main]

// =============================================================================
// Constants
// =============================================================================

/// CNode slot 0: IoPort capability for COM1 (0x3F8, size 8).
const IO_SLOT: u64 = 0;

/// CNode slot 1: Endpoint capability for receiving commands.
const EP_SLOT: u64 = 1;

/// COM1 data register (Transmit Holding / Receive Buffer).
const COM1_DATA: u16 = 0x3F8;

/// COM1 Line Status Register.
const COM1_LSR: u16 = 0x3FD;

/// LSR bit 5: Transmit Holding Register Empty.
const LSR_TX_EMPTY: u8 = 1 << 5;

/// IPC label for "print character" command.
const CMD_PRINT_CHAR: u64 = 0x01;

// =============================================================================
// Driver Entry Point
// =============================================================================

/// Entry point — the kernel jumps here via IRETQ into Ring 3.
///
/// The function runs at virtual address 0x400000 (set by linker.ld).
/// All hardware access goes through libmnos syscall wrappers.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    // Phase 1: Write hello banner to COM1 via capability-gated port I/O.
    //
    // This proves: Ring 3 → SYSCALL → IoPort capability → OUT instruction.
    // Each character requires 2 syscalls: port_in (check LSR) + port_out (write).
    let banner = b"\r\n[serial_drv] Hello from Ring 3 Serial Driver!\r\n";
    for &byte in banner.iter() {
        write_byte(byte);
    }

    // Phase 2: Enter the IPC command loop.
    //
    // This proves: Ring 3 IPC receive + port I/O in the same driver.
    // The kernel (or another user thread) sends CMD_PRINT_CHAR messages
    // with a character in data0. We write each character to COM1.
    let entering = b"[serial_drv] Entering IPC command loop...\r\n";
    for &byte in entering.iter() {
        write_byte(byte);
    }

    loop {
        match libmnos::ipc::sys_recv(EP_SLOT) {
            Ok(msg) => {
                if msg.label == CMD_PRINT_CHAR {
                    write_byte(msg.data0 as u8);
                }
                // Unknown labels are silently ignored
            }
            Err(_) => {
                // Capability error — shouldn't happen if kernel set up correctly.
                // Can't kprintln from Ring 3, so just continue.
            }
        }
    }
}

// =============================================================================
// Serial I/O Helpers
// =============================================================================

/// Writes a single byte to COM1 using the polled TX path.
///
/// 1. Spins until LSR bit 5 (TX empty) is set via SYS_PORT_IN
/// 2. Writes the byte to the data register via SYS_PORT_OUT
///
/// Two syscalls per byte — correct for a capability-based microkernel.
#[inline(always)]
fn write_byte(byte: u8) {
    // Wait for Transmit Holding Register Empty
    loop {
        match libmnos::io::sys_port_in(IO_SLOT, COM1_LSR) {
            Ok(lsr) if lsr & LSR_TX_EMPTY != 0 => break,
            Ok(_) => {} // Not ready yet, spin
            Err(_) => return, // Shouldn't happen
        }
    }

    // Write the byte
    let _ = libmnos::io::sys_port_out(IO_SLOT, COM1_DATA, byte);
}

// =============================================================================
// Panic Handler (required for #![no_std] binaries)
// =============================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Can't print from Ring 3 without capabilities.
    // Just loop forever. In a future sprint, we'd send an IPC to a
    // crash reporter service.
    loop {
        // Spin — can't use HLT in Ring 3
        core::hint::spin_loop();
    }
}
