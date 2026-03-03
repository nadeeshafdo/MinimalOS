// =============================================================================
// libmnos — Port I/O Syscall Wrappers
// =============================================================================
//
// Safe wrappers around SYS_PORT_OUT (3) and SYS_PORT_IN (4).
//
// Ring 3 code cannot execute IN/OUT instructions directly — the CPU raises
// #GP. Instead, userspace drivers use these syscalls, which the kernel
// validates against the IoPort capability before performing the privileged
// I/O port access.
//
// This is the capability-correct approach: the kernel mediates ALL hardware
// access, and each driver only gets access to the specific port range it
// needs (e.g., COM1 at 0x3F8-0x3FF).
//
// =============================================================================

use crate::syscall::SyscallError;

/// Syscall number for port I/O write.
const SYS_PORT_OUT: u64 = 3;

/// Syscall number for port I/O read.
const SYS_PORT_IN: u64 = 4;

/// Writes a byte to a hardware I/O port.
///
/// # Arguments
/// - `slot`:  CNode slot index containing an IoPort capability with WRITE.
/// - `port`:  16-bit I/O port address.
/// - `value`: Byte value to write (low 8 bits).
///
/// # Returns
/// `Ok(())` on success, `Err(SyscallError)` on capability violation.
#[inline(always)]
pub fn sys_port_out(slot: u64, port: u16, value: u8) -> Result<(), SyscallError> {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") SYS_PORT_OUT => result,
            in("rdi") slot,
            in("rsi") port as u64,
            in("rdx") value as u64,
            in("r10") 0u64,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if result == 0 { Ok(()) } else { Err(SyscallError(result)) }
}

/// Reads a byte from a hardware I/O port.
///
/// # Arguments
/// - `slot`: CNode slot index containing an IoPort capability with READ.
/// - `port`: 16-bit I/O port address.
///
/// # Returns
/// `Ok(byte)` with the read value, or `Err(SyscallError)`.
#[inline(always)]
pub fn sys_port_in(slot: u64, port: u16) -> Result<u8, SyscallError> {
    let result: u64;
    let value: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") SYS_PORT_IN => result,
            inlateout("rdi") slot => value,
            in("rsi") port as u64,
            lateout("rdx") _,
            lateout("r10") _,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if result == 0 { Ok(value as u8) } else { Err(SyscallError(result)) }
}
