//! Interrupt and exception handlers.

use x86_64::structures::idt::InterruptStackFrame;

/// Breakpoint exception handler (INT 3).
///
/// This is a trap-type exception triggered by the `int3` instruction.
/// It's commonly used for debugging.
///
/// # Safety
///
/// This function must only be called by the CPU as an interrupt handler.
pub extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    klog::info!("[020] Breakpoint exception triggered!");
}
