//! MinimalOS init — the first user-mode process.
//!
//! This is compiled as a flat binary and loaded by the kernel.
//! It uses raw `syscall` instructions to communicate with the kernel.

#![no_std]
#![no_main]

use core::arch::asm;

// ── Syscall numbers (must match kernel/src/arch/syscall.rs) ─────
const SYS_LOG: u64 = 0;
const SYS_EXIT: u64 = 1;

/// Perform a syscall with two arguments.
#[inline(always)]
unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            // syscall clobbers rcx and r11
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Perform a syscall with one argument.
#[inline(always)]
unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            // syscall clobbers rcx and r11
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Log a message to the kernel serial console.
fn log(msg: &str) {
    unsafe {
        syscall2(SYS_LOG, msg.as_ptr() as u64, msg.len() as u64);
    }
}

/// Exit the process.
fn exit(code: u64) -> ! {
    unsafe {
        syscall1(SYS_EXIT, code);
    }
    // Should never reach here
    loop {
        core::hint::spin_loop();
    }
}

/// Entry point — must be at the very start of the binary.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    log("User process 'init' started!");
    log("Syscall round trip from loaded binary OK");
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    log("PANIC in user process!");
    exit(1);
}
