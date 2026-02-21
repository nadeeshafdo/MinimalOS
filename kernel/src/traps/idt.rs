//! IDT initialization and management.

use crate::arch::idt::{Idt, EntryOptions, GateType};
use crate::arch::tss::Tss;
use core::sync::atomic::{AtomicPtr, Ordering};
use spin::Once;

use super::handlers;

/// Global IDT instance (shared by all cores — IDT has no "Busy" bit).
static IDT: Once<Idt> = Once::new();

/// Raw pointer to the BSP's TSS, for dynamic RSP0 updates during
/// context switch.  Updated when smp::init_bsp() sets up CoreLocal.
static TSS_PTR: AtomicPtr<Tss> = AtomicPtr::new(core::ptr::null_mut());

/// Initialize the GDT, TSS, and IDT for the BSP.
///
/// This sets up:
/// 1. Per-core GDT and TSS via `smp::init_bsp()`
/// 2. Shared IDT with all exception and interrupt handlers
pub fn init_idt() {
	// [089] Per-core GDT/TSS: the BSP's GDT and TSS are now managed
	// by the SMP subsystem in CoreLocal.  The old static TSS and GDT
	// (previously created here via spin::Once) are replaced.

	// First, disable legacy PIC before setting up interrupts.
	khal::pic::disable();
	klog::info!("[022] Legacy PIC disabled (IRQs remapped to 32-47, all masked)");

	// Create and load the BSP's per-core GDT/TSS via the SMP module.
	// We pass APIC ID 0 (BSP) — the real APIC ID is corrected after
	// APIC init, but the GDT/TSS doesn't depend on it.
	unsafe { crate::arch::smp::init_bsp(0); }

	// Store the BSP's TSS pointer for context-switch RSP0 updates.
	let tss_ptr = crate::arch::smp::bsp_tss_ptr();
	TSS_PTR.store(tss_ptr, Ordering::Relaxed);

	let selectors_kcode = 0x08u16; // Kernel code selector (same for all cores)

	klog::debug!("GDT loaded (CS=0x{:04x}, DS=0x{:04x}, TSS=0x{:04x})",
		0x08u16, 0x10u16, 0x28u16);
	klog::info!("[044] User segments defined (User CS=0x{:04x}, User DS=0x{:04x})",
		0x20u16 | 3, 0x18u16 | 3);
	klog::info!("[045] RSP0 set in TSS for Ring 3 -> Ring 0 transitions");

	// Create shared IDT
	let mut idt = Idt::new();
	let cs = selectors_kcode;

	// [020] Register breakpoint handler (INT 3)
	let breakpoint_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let bp_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame)
		= handlers::breakpoint_handler;
	idt.set_handler(3, bp_handler as usize, cs, breakpoint_options);

	// [021] Register double fault handler (INT 8) with IST1
	let double_fault_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt)
		.set_stack_index(handlers::DOUBLE_FAULT_IST_INDEX);

	let df_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame, u64) -> !
		= handlers::double_fault_handler;
	idt.set_handler(8, df_handler as usize, cs, double_fault_options);

	// [024] Register timer interrupt handler (vector 32)
	let timer_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let timer_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame)
		= handlers::timer_handler;
	idt.set_handler(khal::apic::TIMER_VECTOR, timer_handler as usize, cs, timer_options);

	// [023] Register spurious interrupt handler (vector 0xFF)
	let spurious_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let spur_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame)
		= handlers::spurious_handler;
	idt.set_handler(0xFF, spur_handler as usize, cs, spurious_options);

	// Register page fault handler (INT 14) for diagnostics
	let page_fault_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let pf_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame, x86_64::structures::idt::PageFaultErrorCode)
		= handlers::page_fault_handler;
	idt.set_handler(14, pf_handler as usize, cs, page_fault_options);

	// [039] Register keyboard interrupt handler (IRQ1 = vector 33)
	let keyboard_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let kb_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame)
		= handlers::keyboard_handler;
	idt.set_handler(khal::keyboard::KEYBOARD_VECTOR, kb_handler as usize, cs, keyboard_options);

	// [075] Register mouse interrupt handler (IRQ12 = vector 44)
	let mouse_options = EntryOptions::new()
		.set_present(true)
		.set_gate_type(GateType::Interrupt);

	let mouse_handler: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame)
		= handlers::mouse_handler;
	idt.set_handler(khal::mouse::MOUSE_VECTOR, mouse_handler as usize, cs, mouse_options);

	// Load IDT on BSP
	let idt_ref = IDT.call_once(|| idt);
	idt_ref.load();
}

/// Load the shared IDT on an Application Processor.
///
/// Called from `smp::ap_entry()`.  The IDT is shared — it was already
/// created by the BSP in `init_idt()`.  We just need to execute `lidt`.
pub fn load_idt_on_ap() {
	if let Some(idt_ref) = IDT.get() {
		// The IDT is in a static Once<>, so it has 'static lifetime.
		idt_ref.load();
	}
}

/// Get a reference to the global IDT.
#[allow(dead_code)]
pub fn get_idt() -> Option<&'static Idt> {
	IDT.get()
}

/// Get a raw mutable pointer to the current core's TSS.
///
/// For now this returns the BSP's TSS.  In the SMP future, this
/// should read from the per-core CoreLocal via GS.
pub fn tss_ptr() -> *mut Tss {
	TSS_PTR.load(Ordering::Relaxed)
}
