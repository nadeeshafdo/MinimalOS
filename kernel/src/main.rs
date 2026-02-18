#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;
mod arch;
mod memory;
mod task;
mod traps;

use limine::BaseRevision;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, RequestsStartMarker, RequestsEndMarker,
};

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

/// Request the memory map from the bootloader.
#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

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

    // [046] The Hotline — Enable syscall/sysret via MSRs
    arch::syscall::init(arch::tss::Tss::kernel_rsp0());

    // [023] Modern Times - Enable the Local APIC
    klog::debug!("Enabling Local APIC...");
    let hhdm_offset = HHDM_REQUEST.get_response()
        .expect("HHDM response not available")
        .offset();
    klog::debug!("HHDM offset: {:#x}", hhdm_offset);

    // [027] The Census - Iterate memory map and calculate total RAM
    let mmap_response = MEMORY_MAP_REQUEST.get_response()
        .expect("Memory map response not available");
    let (_total_ram, _usable_ram) = memory::census(mmap_response.entries());

    // [028] The Accountant - Initialize the bitmap physical memory manager
    memory::pmm::init(hhdm_offset, mmap_response.entries());

    // [029] Mine! - Allocate a physical frame and verify
    let free_before = memory::pmm::free_frame_count();
    let frame = memory::pmm::alloc_frame().expect("pmm_alloc_frame() returned None");
    let free_after = memory::pmm::free_frame_count();
    klog::info!(
        "[029] pmm_alloc_frame() = {:#x} (free: {} -> {})",
        frame, free_before, free_after,
    );

    // [030] Return It! - Free the frame and verify the bitmap updates
    memory::pmm::free_frame(frame);
    let free_restored = memory::pmm::free_frame_count();
    klog::info!(
        "[030] pmm_free_frame({:#x}) OK (free: {} -> {})",
        frame, free_after, free_restored,
    );

    // [031] Higher Plane - Initialize the paging subsystem
    memory::paging::init(hhdm_offset);

    // [032] The Mapper + [033] The Translator - Map a page and translate it back
    let test_virt: u64 = 0xFFFF_9000_0000_0000; // a virtual address in upper-half
    let test_phys = memory::pmm::alloc_frame().expect("alloc for map_page test");
    memory::paging::map_page(test_virt, test_phys, memory::paging::PageFlags::KERNEL_RW);
    let translated = memory::paging::translate(test_virt)
        .expect("translate returned None after map_page");
    assert_eq!(translated, test_phys, "virt_to_phys mismatch!");
    klog::info!(
        "[032] map_page({:#x} -> {:#x}) OK",
        test_virt, test_phys,
    );
    klog::info!(
        "[033] translate({:#x}) = {:#x} \u{2714}",
        test_virt, translated,
    );
    // Clean up: free the test frame
    memory::pmm::free_frame(test_phys);

    // [034] The Heap - Initialize kernel heap allocator
    memory::heap::init();

    // [035] Dynamic Power - Test Box::new
    let boxed = alloc::boxed::Box::new(42u64);
    assert_eq!(*boxed, 42);
    klog::info!("[035] Box::new(42) = {} at {:p} \u{2714}", *boxed, &*boxed);
    drop(boxed);

    // [036] Vectorization - Test Vec
    let mut v = alloc::vec::Vec::new();
    v.push(1i32);
    v.push(2);
    v.push(3);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0] + v[1] + v[2], 6);
    klog::info!("[036] Vec<i32> = {:?}, sum = {} \u{2714}", v.as_slice(), v.iter().sum::<i32>());
    drop(v);

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

    // [024] The Heartbeat - Enable the Local APIC Timer
    klog::debug!("Enabling APIC timer...");
    khal::apic::enable_timer(
        khal::apic::TIMER_VECTOR,
        0x0020_0000,              // Initial count (~2M, moderate frequency)
        khal::apic::TimerDivide::By16,
    );
    klog::info!("[024] APIC Timer enabled (vector {}, periodic mode)", khal::apic::TIMER_VECTOR);

    // Enable CPU interrupts so the timer (and other APIC interrupts) can fire
    core::arch::asm!("sti", options(nomem, nostack));
    klog::debug!("Interrupts enabled (STI)");

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
            kdisplay::kprintln!("RAM: {} MiB usable / {} MiB total",
                _usable_ram / (1024 * 1024), _total_ram / (1024 * 1024));
            kdisplay::kprintln!();
            kdisplay::kprintln!("Kernel initialized successfully.");
            klog::info!("[017] Formatted output rendered");

            // [025] Tick Tock - Print heartbeat label (dots come from timer handler)
            kdisplay::kprintln!();
            kdisplay::kprint!("[025] Heartbeat: ");
        }
    } else {
        klog::warn!("No framebuffer available");
    }

    // [038] PS/2 Controller - Read status register
    let ps2_status = khal::keyboard::read_status();
    klog::info!("[038] PS/2 status register: {:#04x} (output_full={})",
        ps2_status, ps2_status & 0x01);

    // [039] Key Down - Enable keyboard IRQ1
    khal::keyboard::enable_irq();
    klog::info!("[039] Keyboard IRQ1 enabled (vector {})", khal::keyboard::KEYBOARD_VECTOR);
    klog::info!("[041] Keyboard echo active — type to see characters on screen");

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
