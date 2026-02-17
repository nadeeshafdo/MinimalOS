//! Interrupt and exception handlers.

use x86_64::structures::idt::InterruptStackFrame;

/// IST index used for the double fault handler.
pub const DOUBLE_FAULT_IST_INDEX: u8 = 1;

/// Breakpoint exception handler (INT 3).
///
/// This is a trap-type exception triggered by the `int3` instruction.
/// It's commonly used for debugging.
pub extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    klog::info!("[020] Breakpoint exception triggered!");
}

/// Double Fault exception handler (INT 8).
///
/// A double fault occurs when the CPU fails to invoke an exception handler.
/// This is typically caused by a stack overflow or a missing IDT entry.
/// This handler runs on a dedicated IST stack to ensure it can execute
/// even when the kernel stack is corrupted.
///
/// A double fault is an abort - it cannot be recovered from.
pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    klog::error!("=== DOUBLE FAULT ===");
    klog::error!("Error code: {}", error_code);
    klog::error!("{:#?}", stack_frame);
    klog::error!("System halted.");

    loop {
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

/// Timer interrupt handler (vector 32).
///
/// Fired periodically by the Local APIC Timer. This is the kernel's
/// heartbeat — it drives preemptive scheduling and timekeeping.
/// An EOI must be sent to the APIC at the end of every timer interrupt.
pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    // [025] Tick Tock - Print a dot to the screen on every timer tick.
    // Uses try_lock to avoid deadlock if main thread holds the console lock.
    kdisplay::console_try_write_fmt(format_args!("."));

    // Send End of Interrupt to the APIC
    khal::apic::eoi();
}

/// Spurious interrupt handler (vector 0xFF).
///
/// The APIC may deliver spurious interrupts when the interrupt condition
/// disappears before the CPU acknowledges it. These should be silently
/// ignored — no EOI is sent for spurious interrupts.
pub extern "x86-interrupt" fn spurious_handler(_stack_frame: InterruptStackFrame) {
    // Spurious interrupts require NO end-of-interrupt signal.
    // Simply return.
}

/// Page Fault exception handler (INT 14).
///
/// Temporary diagnostic handler to identify page fault causes.
/// Prints the faulting address (CR2) and error code.
pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    let cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
    }
    klog::error!("=== PAGE FAULT ===");
    klog::error!("Faulting address (CR2): {:#018x}", cr2);
    klog::error!("Error code: {:?}", error_code);
    klog::error!("{:#?}", stack_frame);
    klog::error!("System halted.");

    loop {
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }
    }
}
