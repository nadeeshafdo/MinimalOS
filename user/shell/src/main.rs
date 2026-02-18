//! MinimalOS shell — [069].
//!
//! A simple command-line shell that reads keyboard input via `sys_read`,
//! parses commands, and spawns programs via `sys_spawn`.

#![no_std]
#![no_main]

use core::arch::asm;

// ── Syscall numbers (must match kernel/src/arch/syscall.rs) ─────
const SYS_LOG: u64 = 0;
const SYS_EXIT: u64 = 1;
const SYS_YIELD: u64 = 2;
#[allow(dead_code)]
const SYS_SPAWN: u64 = 3;
const SYS_READ: u64 = 4;

// ── Syscall wrappers ────────────────────────────────────────────

#[inline(always)]
unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

#[inline(always)]
unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

#[inline(always)]
unsafe fn syscall0(nr: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

fn log(msg: &str) {
    unsafe { syscall2(SYS_LOG, msg.as_ptr() as u64, msg.len() as u64); }
}

fn exit(code: u64) -> ! {
    unsafe { syscall1(SYS_EXIT, code); }
    loop { core::hint::spin_loop(); }
}

fn yield_cpu() {
    unsafe { syscall0(SYS_YIELD); }
}

#[allow(dead_code)]
fn spawn(path: &str) -> u64 {
    unsafe { syscall2(SYS_SPAWN, path.as_ptr() as u64, path.len() as u64) }
}

/// Read one byte from STDIN.  Returns 0 if nothing available.
fn read_char() -> u8 {
    unsafe { syscall1(SYS_READ, 0) as u8 }
}

// ── Shell implementation ────────────────────────────────────────

const MAX_LINE: usize = 128;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    log("[069] MinimalOS shell started");
    log("Type 'help' for available commands.");

    let mut buf = [0u8; MAX_LINE];
    let mut pos: usize = 0;

    // Print initial prompt
    log("$ ");

    loop {
        let ch = read_char();
        if ch == 0 {
            // No input — yield CPU to avoid busy-waiting.
            yield_cpu();
            continue;
        }

        match ch {
            // Enter / newline
            b'\n' | b'\r' => {
                if pos > 0 {
                    let cmd = unsafe { core::str::from_utf8_unchecked(&buf[..pos]) };
                    handle_command(cmd);
                    pos = 0;
                }
                log("\n$ ");
            }
            // Backspace
            0x08 | 0x7F => {
                if pos > 0 {
                    pos -= 1;
                }
            }
            // Printable ASCII
            0x20..=0x7E => {
                if pos < MAX_LINE {
                    buf[pos] = ch;
                    pos += 1;
                }
            }
            _ => {}
        }
    }
}

fn handle_command(cmd: &str) {
    let cmd = cmd.trim_ascii();
    if cmd.is_empty() {
        return;
    }

    match cmd {
        "help" => {
            log("Available commands:");
            log("  help        — show this message");
            log("  hello       — print greeting");
            log("  exit        — exit the shell");
        }
        "hello" => {
            log("Hello from MinimalOS shell!");
        }
        "exit" => {
            log("Shell exiting...");
            exit(0);
        }
        _ => {
            // Unknown command — build message in a small buffer to
            // avoid multiple separate log calls.
            let mut msg = [0u8; 160];
            let prefix = b"Unknown command: ";
            let cmd_bytes = cmd.as_bytes();
            let total = prefix.len() + cmd_bytes.len();
            if total <= msg.len() {
                msg[..prefix.len()].copy_from_slice(prefix);
                msg[prefix.len()..total].copy_from_slice(cmd_bytes);
                let s = unsafe { core::str::from_utf8_unchecked(&msg[..total]) };
                log(s);
            } else {
                log("Unknown command (too long)");
            }
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    log("PANIC in shell!");
    exit(1);
}
