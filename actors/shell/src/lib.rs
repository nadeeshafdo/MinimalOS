#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::{log, Message};

/// Endpoint to VFS actor (seeded by kernel at slot 1).
const EP_VFS: i64 = 1;
/// Endpoint to UI Server actor (seeded by kernel at slot 2).  Phase 10.
const _EP_UI: i64 = 2;

/// Pack a null-terminated filename into the 24-byte `data` field of a Message.
fn pack_filename(name: &[u8]) -> [u64; 3] {
    let mut buf = [0u8; 24];
    let len = name.len().min(23); // 23 chars + NUL
    buf[..len].copy_from_slice(&name[..len]);
    // buf[len] is already 0 (NUL terminator)

    // Reinterpret as [u64; 3] — same endianness, repr(C).
    let d0 = u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
    let d1 = u64::from_le_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);
    let d2 = u64::from_le_bytes([buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23]]);
    [d0, d1, d2]
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("Shell actor started.");

    // ── Read hello.txt via VFS IPC ──────────────────────────────
    log!("Shell: requesting hello.txt from VFS...");

    let req = Message {
        label: sdk::VFS_READ_REQ,
        data: pack_filename(b"hello.txt"),
        cap_grant: 0,
        cap_perms: 0,
        _pad: 0,
    };

    // Send VFS_READ_REQ to VFS via Endpoint at slot 1.
    let send_result = unsafe { sdk::sys_cap_send(EP_VFS, &req as *const Message as i32) };
    if send_result != 0 {
        log!("Shell: ERROR — sys_cap_send to VFS failed ({})", send_result as i64);
        unsafe { sdk::sys_exit(1); }
        return;
    }
    log!("Shell: VFS_READ_REQ sent, waiting for reply...");

    // Block until VFS responds.
    let mut reply = Message::empty();
    let recv_result = unsafe { sdk::sys_cap_recv(&mut reply as *mut Message as i32) };
    if recv_result != 0 {
        log!("Shell: ERROR — sys_cap_recv failed ({})", recv_result as i64);
        unsafe { sdk::sys_exit(1); }
        return;
    }

    if reply.label != sdk::VFS_READ_REPLY {
        log!("Shell: ERROR — unexpected reply label {}", reply.label);
        unsafe { sdk::sys_exit(1); }
        return;
    }

    let file_offset = reply.data[0] as i32;
    let file_size = reply.data[1] as usize;
    let mem_cap = reply.cap_grant as i64;

    log!("Shell: VFS replied — offset={}, size={}, cap={}", file_offset, file_size, mem_cap);

    if file_size == 0 || file_size > 4096 {
        log!("Shell: ERROR — invalid file size {}", file_size);
        unsafe { sdk::sys_exit(1); }
        return;
    }

    // Read the file contents from the granted Memory capability.
    let mut buf = [0u8; 256];
    let read_len = file_size.min(buf.len());
    let read_result = unsafe {
        sdk::sys_cap_mem_read(mem_cap, file_offset, buf.as_mut_ptr() as i32, read_len as i32)
    };
    if read_result != 0 {
        log!("Shell: ERROR — sys_cap_mem_read failed ({})", read_result as i64);
        unsafe { sdk::sys_exit(1); }
        return;
    }

    // Log the file contents.
    if let Ok(text) = core::str::from_utf8(&buf[..read_len]) {
        log!("Shell: === hello.txt ({} bytes) ===", read_len);
        log!("{}", text);
        log!("Shell: === end ===");
    } else {
        log!("Shell: ERROR — file is not valid UTF-8");
    }

    log!("Shell: capability IPC test complete.");
    unsafe { sdk::sys_exit(0); }
}
