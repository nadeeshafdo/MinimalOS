// =============================================================================
// MinimalOS NextGen — Kernel Panic Handler
// =============================================================================
//
// When Rust code panics (assert failure, unwrap on None, explicit panic!()),
// this handler is called. Since we're `#![no_std]`, we must provide our own.
//
// PHILOSOPHY:
//   In MinimalOS NextGen, panics in the kernel are FATAL. They indicate a
//   kernel bug — violated invariants, corrupted state, impossible conditions.
//   Unlike userspace panics (which kill one process), a kernel panic means
//   something is fundamentally wrong with the system's trusted computing base.
//
// WHAT WE DO:
//   1. Print the panic message and location to serial (our most reliable output)
//   2. Halt all CPU cores permanently
//   3. Never return
//
// WHAT WE DON'T DO (yet):
//   - Stack unwinding (we compile with panic=abort)
//   - Core dumps
//   - Crash logging to disk
//   - Automatic reboot
//   These are future improvements. For now, halting and printing is correct.
//
// WHY halt_forever() AND NOT A REBOOT?
//   During development, we WANT the system to freeze on panic so we can:
//   - Read the error message on serial output
//   - Attach a debugger
//   - Examine the state of memory
//   Automatic reboots would make debugging much harder.
//
// =============================================================================

use crate::arch::cpu;
use crate::kprintln;
use core::panic::PanicInfo;

/// The kernel panic handler.
///
/// This function is called by the Rust runtime whenever a panic occurs.
/// It is marked `#[panic_handler]` which tells the compiler to use it
/// as the global panic handler for this `#![no_std]` binary.
///
/// # Arguments
/// - `info`: Contains the panic message and source location (file:line).
///
/// # Never Returns
/// This function halts the CPU permanently. The `-> !` return type
/// tells the compiler this function diverges.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // =======================================================================
    // CRITICAL: We're in a panic — the kernel state may be corrupted.
    // We use the serial port directly (not through the spinlock) to avoid
    // potential deadlock if the panic occurred while the serial lock was held.
    //
    // Yes, this means output might be garbled if another core is also
    // printing. That's acceptable — getting SOME output is better than
    // deadlocking silently.
    // =======================================================================

    // Print the panic header.
    // We use kprintln! which goes through the spinlock. If this deadlocks,
    // at least the serial FIFO should have flushed some partial output.
    // A future improvement could write directly to the serial port without
    // the lock.
    kprintln!();
    kprintln!("==========================================================");
    kprintln!("  KERNEL PANIC — MinimalOS NextGen");
    kprintln!("==========================================================");

    // Print the panic location (file and line number) if available.
    if let Some(location) = info.location() {
        kprintln!("  Location: {}:{}", location.file(), location.line());
    } else {
        kprintln!("  Location: <unknown>");
    }

    // Print the panic message.
    // PanicInfo::message() returns the format arguments passed to panic!().
    kprintln!("  Message: {}", info.message());

    kprintln!("==========================================================");
    kprintln!("  System halted. Reboot required.");
    kprintln!("==========================================================");

    // Halt forever — disable interrupts and loop on HLT.
    // No interrupt can wake us, no code can run after this.
    cpu::halt_forever()
}
