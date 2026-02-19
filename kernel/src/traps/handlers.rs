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
    // Send End of Interrupt to the APIC *first*, so the timer can
    // continue firing even if schedule() switches to a different task.
    khal::apic::eoi();

    // [072] Increment the global tick counter.
    crate::task::clock::tick();

    // [064] The Slice — preemptive scheduling on timer tick.
    // Use try_lock to avoid deadlock if the scheduler is already held
    // (e.g. inside sys_yield or sys_spawn).
    if let Some(sched) = crate::task::process::SCHEDULER.try_lock() {
        let count = sched.task_count();
        drop(sched); // Release lock before context switch
        if count > 1 {
            unsafe { crate::task::process::do_schedule() };
        }
    }
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

/// Keyboard interrupt handler (IRQ1 = vector 33).
///
/// Reads the scancode from the PS/2 data port, feeds it through the
/// `pc-keyboard` state machine, echoes the character to the console,
/// and pushes it into the kernel input buffer for `sys_read`.
pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    let status = khal::keyboard::read_status();
    if status & 0x01 != 0 {
        let scancode = khal::keyboard::read_scancode();

        // Feed through the pc-keyboard state machine (handles Shift,
        // CapsLock, extended keys, key-release, etc.)
        if let Some(ch) = khal::keyboard::handle_scancode(scancode) {
            // [041] Echo + [042] Backspace
            kdisplay::console_try_put_char(ch);
            // [068] Push into kernel input ring buffer for sys_read(STDIN)
            crate::task::input::push_char(ch);
        }
    }

    // Send EOI to PIC1
    khal::keyboard::send_eoi();
}
