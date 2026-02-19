#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;
mod arch;
mod fs;
mod memory;
mod task;
mod traps;

use limine::BaseRevision;
use limine::modules::InternalModule;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, ModuleRequest,
    RequestsStartMarker, RequestsEndMarker,
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

/// Internal module: the RAMDisk tar archive.
static RAMDISK_MODULE: InternalModule =
    InternalModule::new().with_path(c"/boot/ramdisk.tar");

/// Request the bootloader to load the ramdisk module.
#[used]
#[unsafe(link_section = ".requests")]
static MODULE_REQUEST: ModuleRequest =
    ModuleRequest::new().with_internal_modules(&[&RAMDISK_MODULE]);

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

    // [039][040] Initialise keyboard state machine (pc-keyboard crate)
    khal::keyboard::init();

    // Enable keyboard IRQ1
    khal::keyboard::enable_irq();
    klog::info!("[039] Keyboard IRQ1 enabled (vector {})", khal::keyboard::KEYBOARD_VECTOR);
    klog::info!("[041] Keyboard echo active — type to see characters on screen");
    // \u{2500}\u{2500} [075] PS/2 Mouse init \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}
    khal::mouse::init();
    khal::mouse::enable_irq();
    klog::info!("[075] PS/2 Mouse initialised (IRQ12, vector {})", khal::mouse::MOUSE_VECTOR);

    // [077] Initialise software cursor (after framebuffer + mouse).
    if let Some(fb) = FRAMEBUFFER_REQUEST.get_response().and_then(|r| r.framebuffers().next()) {
        unsafe { kdisplay::init_cursor(&fb); }
        klog::info!("[077] Software cursor initialised (XOR sprite, 12x19)");
    }

    klog::info!("Kernel initialized successfully");

    // ── [053] The Disk — Detect the RAMDisk module ─────────────
    let module_response = MODULE_REQUEST.get_response()
        .expect("Limine module response not available");
    let modules = module_response.modules();
    klog::info!("[053] Limine modules loaded: {} module(s)", modules.len());

    let ramdisk_file = modules.first()
        .expect("No modules loaded — ramdisk.tar missing from ISO");
    let rd_base = ramdisk_file.addr();
    let rd_size = ramdisk_file.size() as usize;
    klog::info!(
        "[053] RAMDisk detected: base={:p}, size={} bytes ({} sectors)",
        rd_base, rd_size, rd_size / 512,
    );

    // Store ramdisk globally so sys_spawn can access it later.
    fs::ramdisk::init(rd_base, rd_size);

    let ramdisk = fs::ramdisk::get().expect("ramdisk not stored");

    // ── [054] The Block — Read a raw sector ────────────────────
    if let Some(sector0) = ramdisk.read_sector(0) {
        klog::info!(
            "[054] Sector 0 read OK — first 16 bytes: {:02x?}",
            &sector0[..16],
        );
    } else {
        klog::error!("[054] Failed to read sector 0!");
    }

    // ── [055] The Structure — TAR filesystem parser ────────────
    klog::info!("[055] Parsing USTAR tar archive...");
    let mut entry_count = 0usize;
    let iter = unsafe { fs::tar::TarIter::new(ramdisk) };
    for entry in iter {
        klog::info!(
            "[055]   entry: name={:?} size={} type={}",
            entry.name,
            entry.size,
            entry.typeflag as char,
        );
        entry_count += 1;
    }
    klog::info!("[055] TAR archive contains {} entries", entry_count);

    // ── [056] The Listing — ls /  ──────────────────────────────
    klog::info!("[056] ls /:");
    let iter = unsafe { fs::tar::TarIter::new(ramdisk) };
    for entry in iter {
        let name = entry.name.strip_prefix("./").unwrap_or(entry.name);
        if name.is_empty() {
            continue; // skip the root "./" entry
        }
        let kind = if entry.typeflag == b'5' { "DIR " } else { "FILE" };
        klog::info!("[056]   {} {:>6} {}", kind, entry.size, name);
    }

    // ── [057] The Reader — cat hello.txt ───────────────────────
    klog::info!("[057] cat hello.txt:");
    if let Some(entry) = fs::tar::find_file(ramdisk, "hello.txt") {
        if let Ok(text) = core::str::from_utf8(entry.data) {
            for line in text.lines() {
                klog::info!("[057]   {}", line);
            }
        } else {
            klog::error!("[057] hello.txt: not valid UTF-8");
        }
    } else {
        klog::error!("[057] hello.txt: file not found");
    }

    // ── [058] The Loader — Load ELF from ramdisk ────────────────
    //
    // 1. Find init.elf in the tar archive.
    // 2. Parse the ELF header and program headers.
    // 3. Map PT_LOAD segments into user address space.
    // 4. Allocate a user stack.
    // 5. Create a Process and push it to the scheduler.

    let elf_entry = fs::tar::find_file(ramdisk, "init.elf")
        .expect("[058] init.elf not found in ramdisk");
    klog::info!("[058] Found init.elf in ramdisk ({} bytes)", elf_entry.size);

    let elf = fs::elf::parse(elf_entry.data)
        .expect("[058] Failed to parse init.elf");
    klog::info!("[058] ELF entry point: {:#x}", elf.entry);

    // Map each PT_LOAD segment into user space.
    for phdr in elf.phdrs {
        if !phdr.is_load() {
            continue;
        }

        let vaddr = phdr.p_vaddr;
        let memsz = phdr.p_memsz as usize;
        let filesz = phdr.p_filesz as usize;
        let offset = phdr.p_offset as usize;
        let flags = phdr.p_flags;

        klog::info!(
            "[058]   PT_LOAD: vaddr={:#x} filesz={} memsz={} flags={:#x}",
            vaddr, filesz, memsz, flags,
        );

        // Allocate and map pages for this segment.
        let page_start = vaddr & !0xFFF;
        let page_end = (vaddr + memsz as u64 + 0xFFF) & !0xFFF;
        let num_pages = ((page_end - page_start) / 4096) as usize;

        for i in 0..num_pages {
            let page_virt = page_start + (i as u64) * 4096;
            // Only allocate if this page isn't already mapped (segments may share pages).
            if memory::paging::translate(page_virt).is_none() {
                let phys = memory::pmm::alloc_frame()
                    .expect("alloc user ELF page");
                memory::paging::map_page(
                    page_virt,
                    phys,
                    memory::paging::PageFlags::USER_RW,
                );
                // Zero the page first (for BSS / partial pages).
                unsafe {
                    core::ptr::write_bytes(page_virt as *mut u8, 0, 4096);
                }
            }
        }

        // Copy file data into the mapped pages.
        if filesz > 0 {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    elf.data.as_ptr().add(offset),
                    vaddr as *mut u8,
                    filesz,
                );
            }
        }
    }

    // Allocate and map user stack page.
    let user_stack_virt: u64 = 0x80_0000; // 8 MiB
    let user_stack_phys = memory::pmm::alloc_frame()
        .expect("alloc user stack page");
    memory::paging::map_page(
        user_stack_virt,
        user_stack_phys,
        memory::paging::PageFlags::USER_RW,
    );

    // User stack grows downward — point to top of the page.
    let user_rsp = user_stack_virt + 4096;

    let entry_point = elf.entry;
    klog::info!("[058] ELF entry={:#x}, user RSP={:#x}", entry_point, user_rsp);

    // ── [061]-[063] Create init process via scheduler ───────────
    let cr3: u64;
    core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));

    let mut init_proc = task::process::Process::new("init", cr3, entry_point, user_rsp);
    init_proc.prepare_initial_stack();
    klog::info!("[061] PCB created: PID={}, name=\"init\"", init_proc.pid);
    klog::info!("[062] Context switch assembly ready");

    // Create a "kernel idle" process that represents the current kernel
    // thread.  The scheduler needs a "current" to switch away from.
    let mut idle = task::process::Process::new("idle", cr3, 0, 0);
    idle.state = task::process::ProcessState::Running;

    {
        let mut sched = task::process::SCHEDULER.lock();
        sched.set_current(idle);
        sched.push(init_proc);
        klog::info!("[063] Scheduler ready: {} task(s)", sched.task_count());
    }

    // Perform the first schedule — this will context-switch from idle
    // into init's prepared kernel stack, which returns to the
    // task_entry_trampoline and then iretqs to Ring 3.
    klog::info!("[064] Starting scheduler...");
    unsafe {
        task::process::do_schedule();
    }

    // If we return here (all tasks exited), idle loop.
    klog::info!("All tasks completed — entering idle loop");
    loop {
        // The context switch may return with IF=0 (e.g. from a timer
        // interrupt context).  We must re-enable interrupts before HLT,
        // otherwise the timer can never fire and we halt forever.
        // "sti; hlt" is idiomatic on x86 — the instruction boundary
        // between STI and HLT allows exactly one interrupt to arrive.
        core::arch::asm!("sti; hlt", options(nomem, nostack));
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
