#![no_std]

// ── IPC Protocol Labels ─────────────────────────────────────────

/// Shell → VFS: "read this file" (filename in data, null-terminated)
pub const VFS_READ_REQ: u64 = 1;
/// VFS → Shell: "here's the data" (data[0]=offset, data[1]=size, cap_grant=Memory cap)
pub const VFS_READ_REPLY: u64 = 2;
/// Shell → UI: "draw these bytes" (reserved for Phase 10)
pub const UI_DRAW_REQ: u64 = 3;

// ── IPC Message ─────────────────────────────────────────────────

/// A discrete IPC message — 48 bytes, matching kernel `ipc::Message` layout.
///
/// `data` carries up to 24 bytes of inline payload (e.g. a null-terminated
/// filename).  `cap_grant` enables zero-copy capability transfer: set it
/// to the composite handle of a cap you hold with GRANT permission, and
/// the kernel will clone it into the receiver's CapTable during delivery.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Message {
    pub label: u64,
    pub data: [u64; 3],
    /// Composite handle of capability to transfer (0 = none).
    pub cap_grant: u64,
    /// Permission narrowing mask for the transfer.
    pub cap_perms: u32,
    pub _pad: u32,
}

impl Message {
    pub const fn empty() -> Self {
        Self {
            label: 0,
            data: [0; 3],
            cap_grant: 0,
            cap_perms: 0,
            _pad: 0,
        }
    }
}

// ── FFI Declarations ────────────────────────────────────────────

extern "C" {
    pub fn sys_log(ptr: i32, len: i32);
    pub fn sys_exit(code: i32);
    pub fn sys_cap_send(ep: i64, msg: i32) -> i64;
    pub fn sys_cap_recv(buf: i32) -> i64;
    pub fn sys_cap_mem_read(cap: i64, off: i32, dst: i32, len: i32) -> i64;
    pub fn sys_cap_mem_write(cap: i64, off: i32, src: i32, len: i32) -> i64;
}

// ── Buffered Logger ─────────────────────────────────────────────

/// A 256-byte stack buffer that collects an entire `log!()` call
/// before flushing to `sys_log` in a single shot.
pub struct Logger {
    buf: [u8; 256],
    pos: usize,
}

impl Logger {
    pub const fn new() -> Self {
        Self { buf: [0u8; 256], pos: 0 }
    }

    /// Flush the buffer to the kernel log.
    pub fn flush(&mut self) {
        if self.pos > 0 {
            unsafe { sys_log(self.buf.as_ptr() as i32, self.pos as i32); }
            self.pos = 0;
        }
    }
}

impl core::fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = b;
                self.pos += 1;
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        let mut logger = $crate::Logger::new();
        let _ = core::fmt::Write::write_fmt(&mut logger, core::format_args!($($arg)*));
        logger.flush();
    }};
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { sys_exit(-1); }
    loop {}
}
