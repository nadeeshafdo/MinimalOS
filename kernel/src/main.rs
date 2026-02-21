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
	MpRequest, RequestsStartMarker, RequestsEndMarker,
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

/// [089] Request SMP information from the bootloader.
#[used]
#[unsafe(link_section = ".requests")]
static SMP_REQUEST: MpRequest = MpRequest::new();

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
	// Use the BSP's per-core kernel RSP from the SMP CoreLocal.
	arch::syscall::init(arch::smp::bsp_core_local().kernel_rsp0());

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

	// Pre-allocate the shared higher-half region for window buffers
	// (PML4[384]).  This must happen before any process is created
	// so that every process's PML4 copy includes the entry.
	unsafe { memory::paging::init_shared_user_region(); }

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

	// Map and initialise the I/O APIC (routes external IRQs to the Local APIC).
	unsafe { memory::map_apic_mmio(hhdm_offset, khal::ioapic::IOAPIC_PHYS_BASE); }
	let (ioapic_id, ioapic_pins) = khal::ioapic::init(hhdm_offset);
	klog::info!("I/O APIC enabled (ID: {}, {} pins)", ioapic_id, ioapic_pins);

	// [024] The Heartbeat - Enable the Local APIC Timer
	klog::debug!("Enabling APIC timer...");
	khal::apic::enable_timer(
		khal::apic::TIMER_VECTOR,
		0x0020_0000,			  // Initial count (~2M, moderate frequency)
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

			// [080] Store framebuffer info for SYS_FB_INFO / SYS_MMAP.
			// fb.addr() is the HHDM-mapped virtual address; derive physical.
			let fb_phys = fb.addr() as u64 - hhdm_offset;
			task::window::set_fb_info(
				fb_phys,
				fb.width() as u32,
				fb.height() as u32,
				fb.pitch() as u32,
				fb.bpp() as u32,
			);
			klog::info!("[080] Framebuffer info stored: phys={:#x} {}x{}", fb_phys, fb.width(), fb.height());

			// Following commented block of code was written as part of `QUESTS.md`
			// to demonstrate basic framebuffer output before the console was implemented.
			// It's left here as a reference for how to use the framebuffer directly,
			// and to show some early debug output before the console was available.

			// [011] The Screen Wipe - Fill entire screen with blue
			// kdisplay::fill_screen(&fb, kdisplay::Color::BLUE);
			// klog::info!("[011] Screen filled with blue");

			// [015] Initialize framebuffer console
			kdisplay::init_console(&fb, kdisplay::Color::WHITE, kdisplay::Color::ASH);
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

	// ── [089] Wake Application Processors ─────────────────────
	if let Some(smp_response) = SMP_REQUEST.get_response() {
		unsafe { arch::smp::wake_aps(smp_response); }
	} else {
		klog::warn!("SMP: No SMP response from Limine — running single-core");
	}

	// ── [090] Activate SMP memory optimizations ───────────────
	// Per-core frame caches and heap arenas require smp::core_id()
	// to be valid, so they must be activated after SMP init.
	memory::pmm::activate_caches();
	unsafe { memory::heap::init_arenas(); }

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

	// ── [058] The Loader — Spawn init.elf via spawn_from_ramdisk ──
	//
	// With per-process page tables, init is spawned just like every
	// other process: create_user_page_table → map ELF → push to
	// scheduler.  No special-casing required.

	// Create a "kernel idle" process that represents the current
	// kernel thread.  The scheduler needs a "current" to switch
	// away from on the very first do_schedule().
	let cr3: u64;
	core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	let mut idle = task::process::Process::new("idle", cr3, 0, 0);
	idle.state = task::process::ProcessState::Running;
	{
		let mut sched = task::process::SCHEDULER.lock();
		sched.set_current(idle);
	}

	klog::info!("[058] Loading init.elf from ramdisk...");
	match task::process::spawn_from_ramdisk("init.elf", "") {
		Ok(pid) => klog::info!("[058] init.elf spawned (PID {})", pid),
		Err(e) => {
			klog::error!("[058] FATAL: failed to spawn init.elf: {}", e);
			loop { core::arch::asm!("cli; hlt"); }
		}
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
