#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::log;

/// IrqLine { irq: 1 } — blocks until keyboard interrupt fires.
const IRQ_CAP: i64 = 1;
/// IoPort { base: 0x60, count: 5 } — covers PS/2 data (0x60) and status/cmd (0x64).
const IOPORT_CAP: i64 = 2;
/// Endpoint → Shell actor.
const SHELL_EP_CAP: i64 = 3;

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("Keyboard driver started. Listening on IRQ 1...");

    // Drain any boot-time data left in the PS/2 output buffer.
    drain_buffer();

    loop {
        // 1. Check if there's already data before blocking.
        //    Edge-triggered IRQs can be lost between io_read and
        //    the next irq_wait call, so always poll first.
        let status = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 4, 1) } as u8;
        if status & 0x01 == 0 {
            // Output buffer empty — safe to wait for next edge.
            unsafe { sdk::sys_cap_irq_wait(IRQ_CAP); }
        }

        // 2. Drain ALL pending bytes from the PS/2 output buffer.
        //    A single `sendkey` generates both press and release scancodes;
        //    reading them all prevents stale data from blocking the next IRQ.
        drain_buffer();
    }
}

/// Read and process every byte currently in the PS/2 output buffer.
fn drain_buffer() {
    for _ in 0..16 {
        // Check status register (port 0x64, offset 4): bit 0 = output buffer full.
        let status = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 4, 1) } as u8;
        if status & 0x01 == 0 {
            break; // buffer empty
        }

        // Read the data register (port 0x60, offset 0).
        let scancode = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 0, 1) } as u8;

        // Only forward key-press events (scancodes < 0x80).
        if scancode < 0x80 && scancode != 0 {
            log!("KBD: scancode 0x{:02X}", scancode);

            let msg = sdk::Message {
                label: sdk::KEY_EVENT,
                data: [scancode as u64, 0, 0],
                cap_grant: 0,
                cap_perms: 0,
                _pad: 0,
            };
            unsafe { sdk::sys_cap_send(SHELL_EP_CAP, &msg as *const sdk::Message as i32); }
        }
    }
}
