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
