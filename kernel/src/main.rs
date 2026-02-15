#![no_std]
#![no_main]

mod arch;
mod memory;
mod task;
mod traps;

use limine::BaseRevision;
use limine::request::FramebufferRequest;

/// Base revision supported by this kernel.
#[used]
#[link_section = ".limine_requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

/// Request a framebuffer from the bootloader.
#[used]
#[link_section = ".limine_requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

/// Kernel entry point called by the Limine bootloader.
#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    assert!(BASE_REVISION.is_supported());

    if let Some(_framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        // Framebuffer is available for use
    }

    loop {
        core::arch::asm!("hlt");
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
