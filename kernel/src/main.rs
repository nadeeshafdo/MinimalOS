#![no_std]
#![no_main]

mod arch;
mod memory;
mod task;
mod traps;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsStartMarker, RequestsEndMarker};

/// Limine requests start marker.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

/// Base revision supported by this kernel.
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

/// Request a framebuffer from the bootloader.
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

/// Limine requests end marker.
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

/// Kernel entry point called by the Limine bootloader.
#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    // Initialize serial port for logging
    klog::init();

    klog::info!("MinimalOS kernel starting...");
    klog::debug!("Checking Limine base revision...");

    if !BASE_REVISION.is_supported() {
        klog::error!("Limine base revision not supported!");
        loop {
            core::arch::asm!("hlt");
        }
    }

    klog::info!("Limine base revision OK");

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        let fb_count = framebuffer_response.framebuffers().count();
        klog::info!("Framebuffer available: {} framebuffer(s)", fb_count);

        if let Some(fb) = FRAMEBUFFER_REQUEST.get_response().unwrap().framebuffers().next() {
            klog::debug!("  Resolution: {}x{}", fb.width(), fb.height());
            klog::debug!("  Pitch: {}", fb.pitch());
            klog::debug!("  BPP: {}", fb.bpp());
        }
    } else {
        klog::warn!("No framebuffer available");
    }

    klog::info!("Kernel initialized successfully");
    klog::info!("Entering idle loop...");

    loop {
        core::arch::asm!("hlt");
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    klog::error!("KERNEL PANIC!");
    klog::error!("{}", info);
    
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
