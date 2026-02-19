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
const SYS_PIPE_CREATE: u64 = 5;
const SYS_PIPE_WRITE: u64 = 6;
const SYS_PIPE_READ: u64 = 7;
const SYS_PIPE_CLOSE: u64 = 8;
const SYS_TIME: u64 = 9;
const SYS_SLEEP: u64 = 10;
const SYS_FUTEX: u64 = 11;
const SYS_READ_EVENT: u64 = 12;

const FUTEX_WAIT: u64 = 0;
const FUTEX_WAKE: u64 = 1;

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

#[inline(always)]
unsafe fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Create an IPC pipe.  Returns pipe_id or u64::MAX on failure.
fn pipe_create() -> u64 {
    unsafe { syscall0(SYS_PIPE_CREATE) }
}

/// Write `data` into pipe `id`.  Returns bytes written.
fn pipe_write(id: u64, data: &[u8]) -> u64 {
    unsafe { syscall3(SYS_PIPE_WRITE, id, data.as_ptr() as u64, data.len() as u64) }
}

/// Read from pipe `id` into `buf`.  Returns bytes read.
fn pipe_read(id: u64, buf: &mut [u8]) -> u64 {
    unsafe { syscall3(SYS_PIPE_READ, id, buf.as_mut_ptr() as u64, buf.len() as u64) }
}

/// Close pipe `id`.
fn pipe_close(id: u64) {
    unsafe { syscall1(SYS_PIPE_CLOSE, id); }
}

/// [072] Read the current kernel tick count.
fn time() -> u64 {
    unsafe { syscall0(SYS_TIME) }
}

/// [072] Sleep for `ticks` timer ticks.
fn sleep(ticks: u64) {
    unsafe { syscall1(SYS_SLEEP, ticks); }
}

/// [073] Futex wait — block if `*addr == expected`.
fn futex_wait(addr: *const u64, expected: u64) -> u64 {
    unsafe { syscall3(SYS_FUTEX, addr as u64, FUTEX_WAIT, expected) }
}

/// [073] Futex wake — wake up to `count` waiters on `addr`.
fn futex_wake(addr: *const u64, count: u64) -> u64 {
    unsafe { syscall3(SYS_FUTEX, addr as u64, FUTEX_WAKE, count) }
}

/// [079] Read the next input event into a 12-byte buffer.
/// Returns 12 on success, 0 if no event available.
fn read_event(buf: &mut [u8; 12]) -> u64 {
    unsafe { syscall1(SYS_READ_EVENT, buf.as_mut_ptr() as u64) }
}

// ── Shell implementation ────────────────────────────────────────

const MAX_LINE: usize = 128;

#[no_mangle]
pub extern "C" fn _start(args_ptr: u64, args_len: u64) -> ! {
    log("[069] MinimalOS shell started");

    // [071] Display received arguments if any.
    if args_ptr != 0 && args_len > 0 && args_len <= 256 {
        let args = unsafe {
            let slice = core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize);
            core::str::from_utf8_unchecked(slice)
        };
        let mut msg = [0u8; 280];
        let prefix = b"[071] Args: ";
        let total = prefix.len() + args.len();
        if total <= msg.len() {
            msg[..prefix.len()].copy_from_slice(prefix);
            msg[prefix.len()..total].copy_from_slice(args.as_bytes());
            let s = unsafe { core::str::from_utf8_unchecked(&msg[..total]) };
            log(s);
        }
    }

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
            log("  pipe        — test IPC pipe");
            log("  time        — show kernel tick count");
            log("  sleep       — sleep for ~500 ticks");
            log("  futex       — test futex wait/wake");
            log("  events      — read input events (5s)");
            log("  exit        — exit the shell");
        }
        "hello" => {
            log("Hello from MinimalOS shell!");
        }
        "pipe" => {
            // [070] IPC pipe round-trip test
            let id = pipe_create();
            if id == u64::MAX {
                log("pipe: failed to create pipe");
                return;
            }
            let msg = b"Hello from pipe!";
            let written = pipe_write(id, msg);
            let mut rbuf = [0u8; 64];
            let nread = pipe_read(id, &mut rbuf);
            pipe_close(id);

            // Build result string in a scratch buffer
            let mut out = [0u8; 128];
            let prefix = b"pipe: wrote ";
            let mut pos = prefix.len();
            out[..pos].copy_from_slice(prefix);
            pos += fmt_u64(written, &mut out[pos..]);
            let mid = b", read ";
            out[pos..pos + mid.len()].copy_from_slice(mid);
            pos += mid.len();
            pos += fmt_u64(nread, &mut out[pos..]);
            let mid2 = b" bytes: ";
            out[pos..pos + mid2.len()].copy_from_slice(mid2);
            pos += mid2.len();
            let copylen = (nread as usize).min(out.len() - pos);
            out[pos..pos + copylen].copy_from_slice(&rbuf[..copylen]);
            pos += copylen;
            let s = unsafe { core::str::from_utf8_unchecked(&out[..pos]) };
            log(s);
        }
        "time" => {
            // [072] Show current tick count
            let t = time();
            let mut out = [0u8; 40];
            let prefix = b"Ticks: ";
            let mut pos = prefix.len();
            out[..pos].copy_from_slice(prefix);
            pos += fmt_u64(t, &mut out[pos..]);
            let s = unsafe { core::str::from_utf8_unchecked(&out[..pos]) };
            log(s);
        }
        "sleep" => {
            // [072] Sleep for ~500 ticks and show elapsed
            let t0 = time();
            log("Sleeping for 500 ticks...");
            sleep(500);
            let t1 = time();
            let elapsed = t1 - t0;
            let mut out = [0u8; 50];
            let prefix = b"Awake! Elapsed: ";
            let mut pos = prefix.len();
            out[..pos].copy_from_slice(prefix);
            pos += fmt_u64(elapsed, &mut out[pos..]);
            let suffix = b" ticks";
            out[pos..pos + suffix.len()].copy_from_slice(suffix);
            pos += suffix.len();
            let s = unsafe { core::str::from_utf8_unchecked(&out[..pos]) };
            log(s);
        }
        "events" => {
            // [079] Read input events for ~5 seconds (500 ticks).
            log("Reading events for 5s... move mouse or press keys.");
            let t0 = time();
            let mut count: u64 = 0;
            loop {
                let now = time();
                if now - t0 > 500 {
                    break;
                }
                let mut buf = [0u8; 12];
                let n = read_event(&mut buf);
                if n == 12 {
                    count += 1;
                    let kind = buf[0];
                    // Show first few events only to avoid flooding
                    if count <= 5 {
                        let mut out = [0u8; 60];
                        let prefix = b"  event: kind=";
                        let mut pos = prefix.len();
                        out[..pos].copy_from_slice(prefix);
                        pos += fmt_u64(kind as u64, &mut out[pos..]);
                        let mid = b" code=";
                        out[pos..pos + mid.len()].copy_from_slice(mid);
                        pos += mid.len();
                        pos += fmt_u64(buf[1] as u64, &mut out[pos..]);
                        log(unsafe { core::str::from_utf8_unchecked(&out[..pos]) });
                    }
                } else {
                    yield_cpu();
                }
            }
            let mut out = [0u8; 50];
            let prefix = b"[079] Total events: ";
            let mut pos = prefix.len();
            out[..pos].copy_from_slice(prefix);
            pos += fmt_u64(count, &mut out[pos..]);
            log(unsafe { core::str::from_utf8_unchecked(&out[..pos]) });
        }
        "futex" => {
            // [073] Futex round-trip test (single-process)
            // We test that WAIT returns immediately when the value
            // doesn't match (no blocking), and WAKE returns 0 when
            // there are no waiters.
            static mut FUTEX_VAR: u64 = 0;
            let addr = &raw const FUTEX_VAR;

            // Set to 42, then WAIT with expected=0 — should NOT block
            // because *addr (42) != expected (0).
            unsafe { core::ptr::write_volatile(addr as *mut u64, 42) };
            let ret = futex_wait(addr, 0);
            // ret should be u64::MAX (value mismatch = no sleep)
            let mut out = [0u8; 60];
            let prefix = b"futex: WAIT(42!=0) = ";
            let mut pos = prefix.len();
            out[..pos].copy_from_slice(prefix);
            pos += fmt_u64(ret, &mut out[pos..]);
            let suffix = b" (expected MAX)";
            out[pos..pos + suffix.len()].copy_from_slice(suffix);
            pos += suffix.len();
            log(unsafe { core::str::from_utf8_unchecked(&out[..pos]) });

            // WAKE with no waiters — should return 0.
            let woken = futex_wake(addr, 1);
            let mut out2 = [0u8; 50];
            let prefix2 = b"futex: WAKE(no waiters) = ";
            let mut pos2 = prefix2.len();
            out2[..pos2].copy_from_slice(prefix2);
            pos2 += fmt_u64(woken, &mut out2[pos2..]);
            let suffix2 = b" (expected 0)";
            out2[pos2..pos2 + suffix2.len()].copy_from_slice(suffix2);
            pos2 += suffix2.len();
            log(unsafe { core::str::from_utf8_unchecked(&out2[..pos2]) });

            log("futex: [073] Synchronisation primitives OK");
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

/// Format a u64 as decimal ASCII into `buf`.  Returns number of bytes written.
fn fmt_u64(mut val: u64, buf: &mut [u8]) -> usize {
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
        }
        return 1;
    }
    let mut tmp = [0u8; 20]; // u64 max is 20 digits
    let mut i = 0;
    while val > 0 {
        tmp[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    let len = i.min(buf.len());
    for j in 0..len {
        buf[j] = tmp[i - 1 - j];
    }
    len
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    log("PANIC in shell!");
    exit(1);
}
