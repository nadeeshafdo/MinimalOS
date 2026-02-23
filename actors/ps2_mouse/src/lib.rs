#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::log;

/// IrqLine { irq: 12 } — blocks until mouse interrupt fires.
const IRQ_CAP: i64 = 1;
/// IoPort { base: 0x60, count: 5 } — covers PS/2 data (0x60) and status/cmd (0x64).
const IOPORT_CAP: i64 = 2;
/// Endpoint → UI Server actor.
const UI_EP_CAP: i64 = 3;

/// Spin until the PS/2 controller input buffer is ready to accept a byte.
fn wait_write() {
    for _ in 0..10_000 {
        let status = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 4, 1) } as u8;
        if (status & 0b10) == 0 {
            return;
        }
    }
}

/// Spin until the PS/2 controller output buffer has data to read.
fn wait_read() {
    for _ in 0..10_000 {
        let status = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 4, 1) } as u8;
        if (status & 0b01) != 0 {
            return;
        }
    }
}

/// Write a command byte to the PS/2 controller command port (0x64).
fn write_cmd(cmd: u8) {
    wait_write();
    unsafe { sdk::sys_cap_io_write(IOPORT_CAP, 4, 1, cmd as i32); }
}

/// Write a data byte to the PS/2 controller data port (0x60).
fn write_data(data: u8) {
    wait_write();
    unsafe { sdk::sys_cap_io_write(IOPORT_CAP, 0, 1, data as i32); }
}

/// Read a data byte from the PS/2 controller data port (0x60).
fn read_data() -> u8 {
    wait_read();
    unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 0, 1) as u8 }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("Mouse driver started. Initialising PS/2 Aux...");

    // 1. Enable Auxiliary Device (mouse port).
    write_cmd(0xA8);

    // 2. Set Defaults — send via 0xD4 prefix (routes to mouse, not keyboard).
    write_cmd(0xD4);
    write_data(0xF6);
    let _ack1 = read_data(); // ACK (0xFA)

    // 3. Enable Data Reporting.
    write_cmd(0xD4);
    write_data(0xF4);
    let _ack2 = read_data(); // ACK (0xFA)

    log!("Mouse initialisation complete. Listening on IRQ 12...");

    let mut packet = [0u8; 3];
    let mut byte_count: usize = 0;

    loop {
        // Wait for the next mouse IRQ.
        unsafe { sdk::sys_cap_irq_wait(IRQ_CAP); }

        // Drain all pending bytes from the PS/2 output buffer.
        // A single mouse movement can deposit 1–3 bytes depending on
        // timing; reading them all prevents stale data on the next IRQ.
        loop {
            let status = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 4, 1) } as u8;
            if (status & 0x01) == 0 {
                break; // output buffer empty
            }

            let data = unsafe { sdk::sys_cap_io_read(IOPORT_CAP, 0, 1) } as u8;

            // Alignment guard: the first byte of a PS/2 mouse packet
            // MUST have bit 3 set.  If it doesn't, the stream is out
            // of sync — drop the byte and wait for re-alignment.
            if byte_count == 0 && (data & 0x08) == 0 {
                continue;
            }

            packet[byte_count] = data;
            byte_count += 1;

            if byte_count == 3 {
                let status_byte = packet[0];

                // X and Y deltas are 9-bit two's complement.
                // Bits 4 and 5 of the status byte carry the sign bits.
                let mut dx = packet[1] as i32;
                let mut dy = packet[2] as i32;

                if (status_byte & 0x10) != 0 { dx |= !0xFF; } // sign-extend X
                if (status_byte & 0x20) != 0 { dy |= !0xFF; } // sign-extend Y

                // PS/2 Y-axis: up is positive.  Screen Y-axis: down is positive.
                dy = -dy;

                let buttons = (status_byte & 0x01) as u64       // left
                            | (((status_byte & 0x02) as u64) >> 1) << 1;  // right

                let msg = sdk::Message {
                    label: sdk::MOUSE_EVENT,
                    data: [dx as u64, dy as u64, buttons],
                    cap_grant: 0,
                    cap_perms: 0,
                    _pad: 0,
                };
                unsafe {
                    sdk::sys_cap_send(UI_EP_CAP, &msg as *const sdk::Message as i32);
                }

                byte_count = 0;
            }
        }
    }
}
