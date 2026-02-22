#![no_std]

extern "C" {
    pub fn sys_log(ptr: i32, len: i32);
    pub fn sys_exit(code: i32);
    pub fn sys_cap_send(ep: i64, msg: i32) -> i64;
    pub fn sys_cap_recv(buf: i32) -> i64;
    pub fn sys_cap_mem_read(cap: i64, off: i32, dst: i32, len: i32) -> i64;
    pub fn sys_cap_mem_write(cap: i64, off: i32, src: i32, len: i32) -> i64;
}

pub struct Logger;

impl core::fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe { sys_log(s.as_ptr() as i32, s.len() as i32); }
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::Logger, core::format_args!($($arg)*));
    };
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { sys_exit(-1); }
    loop {}
}
