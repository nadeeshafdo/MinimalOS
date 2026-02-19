//! IDT initialization and management.

use crate::arch::gdt::Gdt;
use crate::arch::idt::{Idt, EntryOptions, GateType};
use crate::arch::tss::Tss;
use core::sync::atomic::{AtomicPtr, Ordering};
use spin::Once;

use super::handlers;

/// Global IDT instance.
static IDT: Once<Idt> = Once::new();

/// Global TSS instance.
static TSS: Once<Tss> = Once::new();

/// Global GDT instance.
static GDT: Once<Gdt> = Once::new();

/// Raw pointer to the TSS, set after init, for dynamic RSP0 updates.
static TSS_PTR: AtomicPtr<Tss> = AtomicPtr::new(core::ptr::null_mut());

/// Initialize the GDT, TSS, and IDT.
///
/// This sets up:
/// 1. TSS with IST1 pointing to a dedicated double fault stack
/// 2. GDT with kernel code, kernel data, and TSS descriptors
/// 3. IDT with breakpoint and double fault handlers
pub fn init_idt() {
	// [021] Initialize TSS with IST stacks
	let tss_ref = TSS.call_once(|| {
		let mut tss = Tss::new();
		tss.init();
		tss
	});

	// Store raw pointer for dynamic RSP0 updates during context switch.
	TSS_PTR.store(tss_ref as *const Tss as *mut Tss, Ordering::Relaxed);

	// [021] Initialize GDT with TSS descriptor
	let (gdt, selectors) = Gdt::new(tss_ref);
	let gdt_ref = GDT.call_once(|| gdt);

	// Load GDT and set segment registers
	unsafe {
		gdt_ref.load(&selectors);
	}
	klog::debug!("GDT loaded (CS=0x{:04x}, DS=0x{:04x}, TSS=0x{:04x})",
		selectors.kernel_code, selectors.kernel_data, selectors.tss);
	klog::info!("[044] User segments defined (User CS=0x{:04x}, User DS=0x{:04x})",
		selectors.user_code, selectors.user_data);
	klog::info!("[045] RSP0 set in TSS for Ring 3 -> Ring 0 transitions");

	// Create IDT
	let mut idt = Idt::new();
	let cs = selectors.kernel_code;

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

	// Load IDT
	let idt_ref = IDT.call_once(|| idt);
	idt_ref.load();
}

/// Get a reference to the global IDT.
#[allow(dead_code)]
pub fn get_idt() -> Option<&'static Idt> {
	IDT.get()
}

/// Get a raw mutable pointer to the TSS (for dynamic RSP0 updates).
pub fn tss_ptr() -> *mut Tss {
	TSS_PTR.load(Ordering::Relaxed)
}
