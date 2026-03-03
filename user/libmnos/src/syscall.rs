// =============================================================================
// libmnos — Raw Syscall Wrapper
// =============================================================================
//
// The lowest-level syscall primitive. All other syscall wrappers call this.
//
// On x86_64, the `syscall` instruction:
//   - Saves user RIP → RCX
//   - Saves user RFLAGS → R11
//   - Loads CS from STAR[47:32] (kernel CS)
//   - Loads RIP from LSTAR (kernel entry point)
//   - Clears RFLAGS bits per FMASK (IF → interrupts disabled in kernel)
//
// On return via `sysretq`:
//   - Loads RIP from RCX
//   - Loads RFLAGS from R11
//   - Loads CS/SS for Ring 3
//
// =============================================================================

/// Syscall error — wraps the non-zero return code from RAX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallError(pub u64);

/// Raw syscall with 4 arguments. Returns the RAX result.
///
/// This is the primitive used by all higher-level wrappers.
/// The caller is responsible for interpreting the result and any
/// output registers.
#[inline(always)]
pub unsafe fn syscall4(number: u64, arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            in("r10") arg3,
            lateout("rcx") _,   // Clobbered by CPU (saved user RIP)
            lateout("r11") _,   // Clobbered by CPU (saved user RFLAGS)
            options(nostack),
        );
    }
    result
}
