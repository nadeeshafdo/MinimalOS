// =============================================================================
// libmnos — IRQ Notification Syscall Wrapper
// =============================================================================
//
// Safe wrapper around SYS_WAIT_IRQ (5).
//
// Hardware interrupts are delivered to the kernel, not userspace. A user
// driver that needs interrupt notification calls sys_wait_irq() which blocks
// until the specified IRQ fires. The kernel validates the Interrupt
// capability before blocking the thread.
//
// =============================================================================

use crate::syscall::SyscallError;

/// Syscall number for interrupt wait.
const SYS_WAIT_IRQ: u64 = 5;

/// Blocks until the specified hardware interrupt fires.
///
/// # Arguments
/// - `slot`: CNode slot index containing an Interrupt capability.
///
/// # Returns
/// `Ok(())` when the IRQ fires, `Err(SyscallError)` on capability violation.
#[inline(always)]
pub fn sys_wait_irq(slot: u64) -> Result<(), SyscallError> {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") SYS_WAIT_IRQ => result,
            in("rdi") slot,
            in("rsi") 0u64,
            in("rdx") 0u64,
            in("r10") 0u64,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if result == 0 { Ok(()) } else { Err(SyscallError(result)) }
}
