// =============================================================================
// libmnos — IPC Syscall Wrappers
// =============================================================================
//
// Safe wrappers around SYS_SEND (1) and SYS_RECV (2).
//
// IPC is the fundamental communication mechanism in MinimalOS. All inter-
// process communication flows through capability-gated Endpoints.
//
// =============================================================================

use crate::syscall::SyscallError;

/// Syscall number for IPC send.
const SYS_SEND: u64 = 1;

/// Syscall number for IPC receive.
const SYS_RECV: u64 = 2;

/// A received IPC message.
#[derive(Debug, Clone, Copy)]
pub struct RecvMessage {
    /// Message label (identifies message type / command).
    pub label: u64,
    /// Data register 0.
    pub data0: u64,
    /// Data register 1.
    pub data1: u64,
    /// Data register 2.
    pub data2: u64,
}

/// Sends an IPC message through a capability-referenced endpoint.
///
/// # Arguments
/// - `slot`:  CNode slot index containing an Endpoint capability with WRITE.
/// - `label`: Message label (identifies the message type).
/// - `data0`: First data word.
/// - `data1`: Second data word.
///
/// # Returns
/// `Ok(())` on success, `Err(SyscallError)` if capability validation fails.
#[inline(always)]
pub fn sys_send(slot: u64, label: u64, data0: u64, data1: u64) -> Result<(), SyscallError> {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") SYS_SEND => result,
            in("rdi") slot,
            in("rsi") label,
            in("rdx") data0,
            in("r10") data1,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if result == 0 { Ok(()) } else { Err(SyscallError(result)) }
}

/// Receives an IPC message from a capability-referenced endpoint.
///
/// This call blocks until a sender arrives and delivers a message.
///
/// # Arguments
/// - `slot`: CNode slot index containing an Endpoint capability with READ.
///
/// # Returns
/// `Ok(RecvMessage)` with the received message data, or `Err(SyscallError)`.
#[inline(always)]
pub fn sys_recv(slot: u64) -> Result<RecvMessage, SyscallError> {
    let result: u64;
    let label: u64;
    let data0: u64;
    let data1: u64;
    let data2: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") SYS_RECV => result,
            inlateout("rdi") slot => label,
            lateout("rsi") data0,
            lateout("rdx") data1,
            lateout("r10") data2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if result == 0 {
        Ok(RecvMessage { label, data0, data1, data2 })
    } else {
        Err(SyscallError(result))
    }
}
