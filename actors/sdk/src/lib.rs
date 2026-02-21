#![no_std]

extern "C" {
    fn sys_log(ptr: i32, len: i32);
    fn sys_exit(code: i32);
    fn sys_cap_send(ep: i64, msg: i32) -> i64;
    fn sys_cap_recv(buf: i32) -> i64;
    fn sys_cap_mem_read(cap: i64, off: i32, dst: i32, len: i32) -> i64;
    fn sys_cap_mem_write(cap: i64, off: i32, src: i32, len: i32) -> i64;
}

pub struct Logger;

impl core::fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            sys_log(s.as_ptr() as i32, s.len() as i32);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::Logger, core::format_args!($($arg)*));
    };
}

pub fn exit(code: i32) -> ! {
    unsafe { sys_exit(code); }
    loop { core::hint::spin_loop(); }
}

pub fn mem_read(cap: u64, offset: u64, dst: &mut [u8]) -> Result<(), ()> {
    let res = unsafe {
        sys_cap_mem_read(cap as i64, offset as i32, dst.as_mut_ptr() as i32, dst.len() as i32)
    };
    if res == 0 { Ok(()) } else { Err(()) }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log!("WASM PANIC: {}", info);
    exit(-1);
}
