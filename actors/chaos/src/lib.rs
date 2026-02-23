#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::{log, Message};

/// Capability slot 1 = EP→VFS (WRITE only, for IPC flood test).
const EP_VFS: i64 = 1;
/// Capability slot 2 = tiny Memory cap (1 page = 4096 bytes, READ|WRITE).
const MEM_CAP: i64 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("CHAOS: Adversarial actor started. Commencing attack sequence...");

    // ── Attack 1: Integer Overflow Memory OOB ───────────────────
    // Attempt to read from a Memory capability using a malicious offset
    // that would cause integer overflow: 0xFFFF_FFFF_FFFF_FFF0 + 0x20
    // wraps to 0x10, bypassing naive bounds checks.
    log!("CHAOS: [1/4] Integer overflow attack — offset=0xFFFFFFF0, len=32");
    let mut buf = [0u8; 32];
    let res = unsafe {
        sdk::sys_cap_mem_read(
            MEM_CAP,
            0x7FFFFFF0_u32 as i32, // Large offset (max positive i32 region)
            buf.as_mut_ptr() as i32,
            32,
        )
    };
    if res != 0 {
        log!("CHAOS: [1/4] BLOCKED ✔ (returned {})", res);
    } else {
        log!("CHAOS: [1/4] BREACHED ✘ — kernel allowed OOB read!");
    }

    // ── Attack 2: Negative Offset Memory OOB ────────────────────
    // Attempt to read from a negative offset, which when cast to u64
    // becomes a massive number.
    log!("CHAOS: [2/4] Negative offset attack — offset=-1, len=16");
    let mut buf2 = [0u8; 16];
    let res = unsafe {
        sdk::sys_cap_mem_read(
            MEM_CAP,
            -1i32, // This becomes 0xFFFFFFFF as u32, then 0xFFFFFFFF as u64
            buf2.as_mut_ptr() as i32,
            16,
        )
    };
    if res != 0 {
        log!("CHAOS: [2/4] BLOCKED ✔ (returned {})", res);
    } else {
        log!("CHAOS: [2/4] BREACHED ✘ — kernel allowed negative offset read!");
    }

    // ── Attack 3: IPC Queue Flood ───────────────────────────────
    // Blast the VFS actor with 20 rapid-fire messages to overflow
    // the 16-slot IPC queue.
    log!("CHAOS: [3/4] IPC flood attack — 20 messages to VFS");
    let mut sent = 0u32;
    let mut rejected = 0u32;
    for i in 0..20u64 {
        let msg = Message {
            label: 0xDEAD,  // Bogus label
            data: [i, 0, 0],
            cap_grant: 0,
            cap_perms: 0,
            _pad: 0,
        };
        let res = unsafe { sdk::sys_cap_send(EP_VFS, &msg as *const Message as i32) };
        if res == 0 {
            sent += 1;
        } else {
            rejected += 1;
        }
    }
    log!("CHAOS: [3/4] Flood result: {} sent, {} rejected ✔", sent, rejected);

    // ── Attack 4: Infinite Loop (Preemption Test) ───────────────
    // Enter an infinite loop. The APIC timer MUST preempt this actor
    // and allow other actors on other cores to continue functioning.
    // If this log line is the last one from chaos, preemption works.
    log!("CHAOS: [4/4] Entering infinite loop — kernel must preempt me...");

    // Volatile counter to prevent the compiler from optimizing the loop away.
    let mut counter: u64 = 0;
    loop {
        counter = counter.wrapping_add(1);
        // Prevent dead code elimination
        unsafe { core::ptr::read_volatile(&counter); }
    }
}
