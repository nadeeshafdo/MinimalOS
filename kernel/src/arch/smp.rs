//! SMP (Symmetric Multiprocessing) infrastructure.
//!
//! Provides per-core local storage (`CoreLocal`), AP boot entry, and
//! core identification via the GS segment register.  Each core gets
//! its own GDT and TSS to avoid the TSS "Busy" bit #GP fault.

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::arch::gdt::{Gdt, Selectors};
use crate::arch::tss::Tss;

/// Maximum number of cores supported (Pentium N3710 has 4).
pub const MAX_CORES: usize = 4;

/// MSR addresses for GS base manipulation.
const IA32_GS_BASE: u32 = 0xC000_0101;
const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

/// Counter of APs that have completed their init sequence.
static AP_READY_COUNT: AtomicU32 = AtomicU32::new(0);

/// Total number of cores detected (set by BSP during init).
static CORE_COUNT: AtomicU32 = AtomicU32::new(1);

/// Gate flag: APs spin until the BSP sets this after actors are
/// fully spawned and wired.  Prevents APs from entering the
/// scheduler before capabilities are in place.
static AP_GO: AtomicBool = AtomicBool::new(false);

// ── Per-Core Local Storage ──────────────────────────────────────

/// Size of each core's kernel stack (16 KiB).
const CORE_STACK_SIZE: usize = 4096 * 4;

/// Size of each core's IST stack (16 KiB) for double fault handling.
const IST_STACK_SIZE: usize = 4096 * 4;

/// Per-core kernel stack.
#[repr(C, align(16))]
struct CoreStack {
	data: [u8; CORE_STACK_SIZE],
}

/// Per-core IST stack.
#[repr(C, align(16))]
struct IstStack {
	data: [u8; IST_STACK_SIZE],
}

/// Core-local data.  Each CPU core gets one of these, accessed via
/// the GS register for lock-free, zero-overhead per-core state.
///
/// The `core_id` field MUST be at offset 0 for fast GS-relative reads.
#[repr(C)]
pub struct CoreLocal {
	/// Core index (0 = BSP, 1-3 = APs).  **Must be at offset 0.**
	pub core_id: u32,
	/// Local APIC ID for this core.
	pub apic_id: u32,
	/// Per-core TSS (each core needs its own to avoid Busy-bit #GP).
	tss: Tss,
	/// Per-core GDT (contains a TSS descriptor pointing to `self.tss`).
	gdt: Gdt,
	/// Per-core selectors (identical values across cores, but stored
	/// locally for convenience).
	selectors: Selectors,
	/// Kernel stack for Ring 3 → Ring 0 transitions (RSP0).
	kernel_stack: CoreStack,
	/// IST stack for double fault handling.
	ist_stack: IstStack,
}

impl CoreLocal {
	/// Create a zeroed CoreLocal.  Must be initialised via `init()`.
	const fn zeroed() -> Self {
		Self {
			core_id: 0,
			apic_id: 0,
			tss: Tss::new(),
			gdt: Gdt::zeroed(),
			selectors: Selectors::zeroed(),
			kernel_stack: CoreStack { data: [0; CORE_STACK_SIZE] },
			ist_stack: IstStack { data: [0; IST_STACK_SIZE] },
		}
	}

	/// Initialise this CoreLocal for a specific core.
	///
	/// Sets up the TSS with per-core stacks, builds the GDT with a
	/// TSS descriptor pointing to this core's TSS, and stores selectors.
	fn init(&mut self, core_id: u32, apic_id: u32) {
		self.core_id = core_id;
		self.apic_id = apic_id;

		// Set up per-core RSP0 (kernel stack top, stacks grow down)
		let rsp0 = self.kernel_stack.data.as_ptr() as u64 + CORE_STACK_SIZE as u64;
		self.tss.rsp[0] = rsp0;

		// Set up IST1 for double fault handler
		let ist1 = self.ist_stack.data.as_ptr() as u64 + IST_STACK_SIZE as u64;
		self.tss.ist[0] = ist1;

		// Build GDT with this core's TSS
		let (gdt, selectors) = Gdt::new(&self.tss);
		self.gdt = gdt;
		self.selectors = selectors;
	}

	/// Load this core's GDT and TSS into the CPU.
	///
	/// # Safety
	/// The CoreLocal must outlive the CPU (it's in a static array, so it does).
	unsafe fn load_gdt(&self) {
		self.gdt.load_raw(&self.selectors);
	}

	/// Return the top of this core's kernel stack (for syscall RSP setup).
	pub fn kernel_rsp0(&self) -> u64 {
		self.kernel_stack.data.as_ptr() as u64 + CORE_STACK_SIZE as u64
	}
}

/// Static array of per-core data.  Lives for `'static`.
static mut CORE_LOCALS: [CoreLocal; MAX_CORES] = [
	CoreLocal::zeroed(),
	CoreLocal::zeroed(),
	CoreLocal::zeroed(),
	CoreLocal::zeroed(),
];

// ── MSR helpers ─────────────────────────────────────────────────

#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
	let low = value as u32;
	let high = (value >> 32) as u32;
	asm!(
		"wrmsr",
		in("ecx") msr,
		in("eax") low,
		in("edx") high,
		options(nomem, nostack, preserves_flags)
	);
}

// ── Public API ──────────────────────────────────────────────────

/// Initialise the BSP's CoreLocal and set the GS base.
///
/// Called early in `main.rs` after the IDT is loaded.  This replaces
/// the previous approach of creating the GDT/TSS in `traps/idt.rs`.
///
/// # Safety
/// Must be called exactly once on the BSP before any AP is woken.
pub unsafe fn init_bsp(bsp_apic_id: u32) {
	let cl = &mut CORE_LOCALS[0];
	cl.init(0, bsp_apic_id);
	cl.load_gdt();

	// Set GS base to point to this core's CoreLocal.
	// Use IA32_GS_BASE for now; when syscall entry uses swapgs,
	// it will swap between user GS and IA32_KERNEL_GS_BASE.
	let cl_ptr = cl as *const CoreLocal as u64;
	wrmsr(IA32_GS_BASE, cl_ptr);
	// Also set KERNEL_GS_BASE so swapgs in future syscall entry works.
	wrmsr(IA32_KERNEL_GS_BASE, cl_ptr);

	klog::info!("SMP: BSP (core 0, APIC {}) CoreLocal at {:#x}", bsp_apic_id, cl_ptr);
}

/// Return a reference to the BSP's CoreLocal (core 0).
///
/// This is used by code that needs to access the TSS pointer
/// (e.g., context switch RSP0 updates) before the per-core GS
/// path is fully wired up.
pub fn bsp_core_local() -> &'static CoreLocal {
	unsafe { &CORE_LOCALS[0] }
}

/// Return a raw pointer to the BSP's TSS (for context switch RSP0 updates).
pub fn bsp_tss_ptr() -> *mut Tss {
	unsafe { &mut CORE_LOCALS[0].tss as *mut Tss }
}

/// Get the current core's ID via the GS register.
///
/// This reads the first u32 at the GS base, which is the `core_id`
/// field of `CoreLocal` (guaranteed at offset 0 by `#[repr(C)]`).
#[inline]
#[allow(dead_code)]
pub fn core_id() -> u32 {
	let id: u32;
	unsafe {
		asm!(
			"mov {:e}, gs:[0]",
			out(reg) id,
			options(nomem, nostack, preserves_flags)
		);
	}
	id
}

/// Return the total number of cores online.
#[allow(dead_code)]
pub fn core_count() -> u32 {
	CORE_COUNT.load(Ordering::Relaxed)
}

/// Signal APs that it is safe to enter the scheduler.
///
/// Called by the BSP after all Wasm actors have been spawned
/// and their capabilities fully wired.
pub fn signal_ap_go() {
	AP_GO.store(true, Ordering::Release);
}

/// Wake all Application Processors using the Limine SMP protocol.
///
/// # Arguments
/// * `smp_response` — The Limine SMP response containing CPU info.
/// * `hhdm_offset` — HHDM offset (stored globally for AP use).
///
/// The BSP should call this after completing its full init sequence.
/// Each AP will:
/// 1. Init its CoreLocal (GDT, TSS, stacks)
/// 2. Load the shared IDT
/// 3. Set GS base to its CoreLocal
/// 4. Init its Local APIC
/// 5. Start its APIC timer (for preemption)
/// 6. Enter the scheduler (picks up ready tasks)
pub unsafe fn wake_aps(smp_response: &limine::response::MpResponse) {
	let cpus = smp_response.cpus();
	let total = cpus.len().min(MAX_CORES);
	CORE_COUNT.store(total as u32, Ordering::Relaxed);

	klog::info!("SMP: {} CPUs detected, waking {} APs...", cpus.len(), total - 1);

	// The BSP is always cpu[0] in the Limine SMP response (the one
	// with `lapic_id` matching the BSP APIC ID).  We skip it.
	let mut ap_index: u32 = 1;
	for cpu in cpus.iter() {
		if cpu.id == smp_response.bsp_lapic_id() {
			continue; // Skip BSP
		}
		if ap_index as usize >= MAX_CORES {
			break;
		}

		klog::debug!("SMP: Waking AP {} (LAPIC ID {})", ap_index, cpu.id);

		// Initialise the CoreLocal for this AP *before* waking it.
		// The APIC driver uses a global APIC_BASE that was set by the
		// BSP — all cores share the same APIC MMIO virtual address.
		let cl = &mut CORE_LOCALS[ap_index as usize];
		cl.init(ap_index, cpu.id);

		// Set the AP's goto_address — Limine will jump the AP to
		// this function with its argument being the `SmpInfo` pointer.
		cpu.goto_address.write(ap_entry);

		ap_index += 1;
	}

	// Wait for all APs to report ready.
	let expected = (total - 1) as u32;
	let mut spins: u64 = 0;
	while AP_READY_COUNT.load(Ordering::Acquire) < expected {
		core::hint::spin_loop();
		spins += 1;
		if spins > 100_000_000 {
			klog::warn!("SMP: Timed out waiting for APs ({}/{} ready)",
				AP_READY_COUNT.load(Ordering::Relaxed), expected);
			break;
		}
	}

	let ready = AP_READY_COUNT.load(Ordering::Relaxed);
	klog::info!("SMP: All {} cores online ({} APs ready)", total, ready);
}

// ── AP Entry Point ──────────────────────────────────────────────

/// Entry point for each Application Processor.
///
/// Called by Limine's SMP protocol.  The AP arrives here with:
/// - Paging enabled (same CR3 as BSP)
/// - Interrupts disabled
/// - A temporary stack provided by Limine
///
/// We must set up this core's GDT, TSS, IDT, APIC, and GS base.
extern "C" fn ap_entry(smp_info: &limine::mp::Cpu) -> ! {
	let lapic_id = smp_info.id;

	// Determine which CoreLocal slot we are.
	// We search CORE_LOCALS for the matching APIC ID.
	let core_idx = unsafe {
		let mut idx = 0usize;
		for i in 1..MAX_CORES {
			if CORE_LOCALS[i].apic_id == lapic_id {
				idx = i;
				break;
			}
		}
		idx
	};

	if core_idx == 0 {
		// Could not find our slot — should not happen.
		loop { unsafe { asm!("cli; hlt", options(nomem, nostack)); } }
	}

	unsafe {
		let cl = &CORE_LOCALS[core_idx];

		// 1. Load per-core GDT and TSS.
		cl.load_gdt();

		// 2. Load the shared IDT (BSP already built it).
		crate::traps::load_idt_on_ap();

		// 3. Set GS base to our CoreLocal.
		let cl_ptr = cl as *const CoreLocal as u64;
		wrmsr(IA32_GS_BASE, cl_ptr);
		wrmsr(IA32_KERNEL_GS_BASE, cl_ptr);

		// 4. Disable legacy PIC (idempotent, but safe).
		khal::pic::disable();

		// 5. Init Local APIC (reads from IA32_APIC_BASE MSR, writes
		//    SVR and TPR — these are per-core APIC registers).
		let hhdm = crate::memory::paging::hhdm_offset();
		khal::apic::init(hhdm);

		// Enable the APIC timer on this AP so it receives preemption
		// interrupts and can participate in scheduling.
		khal::apic::init_timer();

		klog::info!("SMP: AP {} (APIC ID {}) started — GDT, TSS, IDT, APIC, Timer OK",
			core_idx, lapic_id);

		// Signal that we're ready.
		AP_READY_COUNT.fetch_add(1, Ordering::Release);

		// Spin-wait with interrupts DISABLED until the BSP signals
		// that all actors are spawned and wired.  We must NOT sti
		// before this point — otherwise timer interrupts would enter
		// do_schedule() and pick up partially-wired actors.
		while !AP_GO.load(Ordering::Acquire) {
			core::hint::spin_loop();
		}

		// NOW enable interrupts and enter the scheduler.
		asm!("sti", options(nomem, nostack));
		crate::task::process::do_schedule();

		// If do_schedule returned (no ready tasks), idle until the
		// APIC timer fires and the handler reschedules us.
		loop {
			asm!("hlt", options(nomem, nostack));
		}
	}
}
