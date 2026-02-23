#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;
mod arch;
mod cap;
mod ipc;
mod memory;
mod task;
mod traps;
mod wasm;

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

			// Display output is now the responsibility of Wasm UI actors.
			// The kernel only stores FbInfo for capability-based access.
		}
	} else {
		klog::warn!("No framebuffer available");
	}

	// [038] PS/2 Controller - Read status register
	let ps2_status = khal::keyboard::read_status();
	klog::info!("[038] PS/2 status register: {:#04x} (output_full={})",
		ps2_status, ps2_status & 0x01);

	// Drain any stale bytes from the PS/2 output buffer.
	while khal::keyboard::read_status() & 0x01 != 0 {
		let _ = khal::keyboard::read_scancode();
	}

	// [039][040] Initialise keyboard state machine (pc-keyboard crate)
	khal::keyboard::init();

	// ── [075] PS/2 Mouse init ──────────────────────────────────
	// Init mouse BEFORE enabling keyboard IRQ1.  Mouse init sends
	// command bytes through the shared PS/2 controller (port 0x60),
	// and stray ACK/ID bytes can trigger IRQ1 if it's already enabled,
	// causing them to be misinterpreted as keyboard scancodes.
	khal::mouse::init();

	// Drain any bytes left by mouse init before enabling IRQs.
	while khal::keyboard::read_status() & 0x01 != 0 {
		let _ = khal::keyboard::read_scancode();
	}

	// Now enable both IRQs — buffer is clean.
	khal::keyboard::enable_irq();
	klog::info!("[039] Keyboard IRQ1 enabled (vector {})", khal::keyboard::KEYBOARD_VECTOR);
	klog::info!("[041] Keyboard echo active — type to see characters on screen");
	khal::mouse::enable_irq();
	klog::info!("[075] PS/2 Mouse initialised (IRQ12, vector {})", khal::mouse::MOUSE_VECTOR);

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

	// Store ramdisk globally so wasm actors can be loaded from it.
	wasm::init_ramdisk(rd_base, rd_size);

	// Diagnostic TAR listing (quests [054]-[057]) has been purged.
	// The VFS wasm actor is now responsible for filesystem operations.

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

	// ── CRITICAL SECTION: disable interrupts from spawn through the
	// first do_schedule().  Actors must not be preempted before the
	// Endpoint wiring is complete.
	core::arch::asm!("cli", options(nomem, nostack));

	// ── Phase 9: Spawn all Wasm actors ─────────────────────────
	// No more ELF.  Every user-space entity is a Wasm actor running
	// inside the kernel's address space via tinywasm.
	use cap::{ObjectKind, perms};

	let rd_phys = rd_base as u64 - hhdm_offset;
	let rd_pages = (rd_size + 0xFFF) / 0x1000;
	let (fb_phys, fb_pages) = match task::window::get_fb_info() {
		Some(info) => {
			let fb_bytes = info.pitch as usize * info.height as usize;
			(info.phys_addr, (fb_bytes + 0xFFF) / 0x1000)
		}
		None => (0u64, 0usize),
	};

	// 1. Spawn VFS actor — gets RAMDisk at slot 1 (READ|GRANT so it can delegate).
	klog::info!("[wasm] Spawning vfs.wasm...");
	let vfs_pid = wasm::spawn_wasm("vfs.wasm", |caps| {
		let rd_cap = ObjectKind::Memory { phys: rd_phys, pages: rd_pages };
		caps.insert_at(1, rd_cap, perms::READ | perms::GRANT);
		klog::info!("[wasm] VFS: RAMDisk cap at slot 1 (READ|GRANT)");
	}).expect("FATAL: failed to spawn vfs.wasm");

	// 2. Spawn UI Server actor — gets Framebuffer at slot 2 (WRITE|GRANT).
	klog::info!("[wasm] Spawning ui_server.wasm...");
	let ui_pid = wasm::spawn_wasm("ui_server.wasm", |caps| {
		if fb_pages > 0 {
			let fb_obj = ObjectKind::Memory { phys: fb_phys, pages: fb_pages };
			caps.insert_at(2, fb_obj, perms::WRITE | perms::GRANT);
			klog::info!("[wasm] UI: Framebuffer cap at slot 2 (WRITE|GRANT)");
		}
	}).expect("FATAL: failed to spawn ui_server.wasm");

	// 3. Spawn Shell actor — caps are injected below.
	klog::info!("[wasm] Spawning shell.wasm...");
	let shell_pid = wasm::spawn_wasm("shell.wasm", |_caps| {
		// Endpoints will be injected in the post-spawn wiring step.
	}).expect("FATAL: failed to spawn shell.wasm");

	// 4. Post-Spawn Endpoint Injection — short-lived lock.
	//    All actors are in the ready queue but interrupts are disabled,
	//    so none of them can execute yet.
	{
		let mut sched = task::process::SCHEDULER.lock();

		// Shell: Slot 1 = EP→VFS, Slot 2 = EP→UI
		if let Some(shell) = sched.get_process_mut(shell_pid) {
			shell.caps.insert_at(1, ObjectKind::Endpoint { target_actor_id: vfs_pid }, perms::WRITE);
			shell.caps.insert_at(2, ObjectKind::Endpoint { target_actor_id: ui_pid }, perms::WRITE);
			klog::info!("[wasm] Shell caps: {}", shell.caps.summary());
		}

		// VFS: Slot 2 = EP→Shell, Slot 3 = EP→UI (reply routing)
		if let Some(vfs) = sched.get_process_mut(vfs_pid) {
			vfs.caps.insert_at(2, ObjectKind::Endpoint { target_actor_id: shell_pid }, perms::WRITE);
			vfs.caps.insert_at(3, ObjectKind::Endpoint { target_actor_id: ui_pid }, perms::WRITE);
			klog::info!("[wasm] VFS caps: {}", vfs.caps.summary());
		}

		// UI Server: Slot 1 = EP→VFS, Slot 3 = EP→Shell
		if let Some(ui) = sched.get_process_mut(ui_pid) {
			ui.caps.insert_at(1, ObjectKind::Endpoint { target_actor_id: vfs_pid }, perms::WRITE);
			ui.caps.insert_at(3, ObjectKind::Endpoint { target_actor_id: shell_pid }, perms::WRITE);
			klog::info!("[wasm] UI caps: {}", ui.caps.summary());
		}
	}

	klog::info!("[wasm] All actors spawned and wired:");
	klog::info!("[wasm]   VFS  (PID {}) — RAMDisk + EP→Shell + EP→UI", vfs_pid);
	klog::info!("[wasm]   UI   (PID {}) — EP→VFS + Framebuffer + EP→Shell", ui_pid);
	klog::info!("[wasm]   Shell(PID {}) — EP→VFS + EP→UI", shell_pid);

	// Release the AP gate — APs can now enter the scheduler and
	// pick up fully-wired actors from the ready queue.
	arch::smp::signal_ap_go();

	// Perform the first schedule — context-switches from idle into
	// the first ready actor's kernel stack trampoline.
	klog::info!("[064] Starting scheduler...");
	unsafe {
		task::process::do_schedule();
	}

	// If we return here (all tasks exited), idle loop.
	klog::info!("All tasks completed — entering idle loop");
	loop {
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
