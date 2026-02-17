#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod arch;
mod memory;
mod task;
mod traps;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, HhdmRequest, RequestsStartMarker, RequestsEndMarker};

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

/// Request the Higher Half Direct Map offset.
#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

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

    // [019] The Loader - Initialize and load the IDT
    klog::debug!("Initializing IDT...");

    // [022] Silence the Old - Disable legacy 8259 PIC
    klog::debug!("Disabling legacy 8259 PIC...");
    khal::pic::disable();
    klog::info!("[022] Legacy PIC disabled (IRQs remapped to 32-47, all masked)");

    traps::init_idt();
    klog::info!("[019] IDT loaded successfully");

    // [023] Modern Times - Enable the Local APIC
    klog::debug!("Enabling Local APIC...");
    let hhdm_offset = HHDM_REQUEST.get_response()
        .expect("HHDM response not available")
        .offset();
    klog::debug!("HHDM offset: {:#x}", hhdm_offset);

    // Read APIC physical base from MSR
    let apic_low: u32;
    let apic_high: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") 0x1Bu32,
        out("eax") apic_low,
        out("edx") apic_high,
        options(nomem, nostack, preserves_flags)
    );
    let apic_phys = ((apic_high as u64) << 32 | apic_low as u64) & 0xFFFF_FFFF_FFFF_F000;
    klog::debug!("APIC phys base: {:#x}", apic_phys);

    // Map the APIC MMIO page into the HHDM virtual address space
    memory::map_apic_mmio(hhdm_offset, apic_phys);

    let apic_id = khal::apic::init(hhdm_offset);
    klog::info!("[023] Local APIC enabled (ID: {})", apic_id);

    // [020] Trap Card - Test breakpoint exception
    klog::debug!("Testing breakpoint exception...");
    unsafe {
        core::arch::asm!("int3", options(nomem, nostack));
    }
    klog::info!("[020] Breakpoint handler executed successfully");

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        let fb_count = framebuffer_response.framebuffers().count();
        klog::info!("Framebuffer available: {} framebuffer(s)", fb_count);

        if let Some(fb) = FRAMEBUFFER_REQUEST.get_response().unwrap().framebuffers().next() {
            klog::debug!("  Resolution: {}x{}", fb.width(), fb.height());
            klog::debug!("  Pitch: {}", fb.pitch());
            klog::debug!("  BPP: {}", fb.bpp());

            // [011] The Screen Wipe - Fill entire screen with blue
            kdisplay::fill_screen(&fb, kdisplay::Color::BLUE);
            klog::info!("[011] Screen filled with blue");

            // [015] Initialize framebuffer console
            kdisplay::init_console(&fb, kdisplay::Color::WHITE, kdisplay::Color::BLUE);
            klog::info!("[015] Framebuffer console initialized");

            // [014] Hello World
            kdisplay::kprintln!("Hello MinimalOS!");
            kdisplay::kprintln!();

            // [017] Formatting test
            kdisplay::kprintln!("Framebuffer: {}x{} @ {}bpp", fb.width(), fb.height(), fb.bpp());
            kdisplay::kprintln!("Pitch: {} bytes", fb.pitch());
            kdisplay::kprintln!("Magic: {:#010X}", 0xDEADBEEFu32);
            kdisplay::kprintln!();
            kdisplay::kprintln!("Kernel initialized successfully.");
            klog::info!("[017] Formatted output rendered");
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
            // Disable interrupts and halt to prevent spurious wakeups
            core::arch::asm!("cli; hlt", options(nomem, nostack));
        }
    }
}
