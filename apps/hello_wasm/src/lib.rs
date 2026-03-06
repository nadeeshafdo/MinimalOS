//! hello_wasm — WebAssembly module for MinimalOS SFI proof
//!
//! Sprint 10 Phase 2: Exports `add(a, b) -> i32` for pure computation proof.
//! Sprint 10 Phase 3: Exports `run_guest()` which calls the imported
//! `host_print(ptr, len)` host function to print a message through the
//! MinimalOS capability system (IoPort slot 2 → COM1 hardware).
//!
//! The execution chain:
//!   Wasm run_guest() → host_print(ptr, len) → wasmi host closure
//!   → read Wasm linear memory → write_byte() → SYS_PORT_OUT → COM1

#![no_std]

#[link(wasm_import_module = "env")]
extern "C" {
    /// Host function provided by Init (the Hypervisor).
    /// Reads `len` bytes from Wasm linear memory at `ptr` and prints
    /// them to COM1 via the IoPort capability.
    fn host_print(ptr: i32, len: i32);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Pure addition — Sprint 10 Phase 2 proof (computational isolation).
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Guest entry point — Sprint 10 Phase 3 proof (host bridge).
///
/// Constructs a string in Wasm linear memory and calls the imported
/// `host_print` host function to push it through the SFI boundary.
#[no_mangle]
pub extern "C" fn run_guest() {
    let msg = b"Hello from the Wasm Sandbox!\n";
    unsafe { host_print(msg.as_ptr() as i32, msg.len() as i32); }
}
