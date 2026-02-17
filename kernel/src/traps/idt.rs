//! IDT initialization and management.

use crate::arch::idt::{Idt, EntryOptions, GateType};
use spin::Once;

use super::handlers;

/// Global IDT instance.
///
/// This is initialized once at kernel startup and loaded into the CPU.
static IDT: Once<Idt> = Once::new();

/// Initialize and load the Interrupt Descriptor Table.
///
/// This function creates a new IDT with all entries initially set to missing
/// (not present), then loads it into the CPU using the `lidt` instruction.
///
/// After loading, the IDT is ready to receive handlers for specific interrupt
/// vectors (which will be added in subsequent achievements).
pub fn init_idt() {
    // Create a new IDT with all entries marked as missing
    let mut idt = Idt::new();

    // [020] Register breakpoint handler (INT 3)
    let breakpoint_options = EntryOptions::new()
        .set_present(true)
        .set_gate_type(GateType::Interrupt);
    
    let handler_fn: extern "x86-interrupt" fn(x86_64::structures::idt::InterruptStackFrame) = handlers::breakpoint_handler;
    let handler_addr = handler_fn as usize;
    
    // Get the current code segment selector from CS register
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {:x}, cs", out(reg) cs);
    }
    
    idt.set_handler(
        3, // Breakpoint vector
        handler_addr,
        cs, // Current kernel code segment
        breakpoint_options,
    );

    // Initialize the global IDT
    let idt_ref = IDT.call_once(|| idt);

    // Load the IDT into the CPU using the lidt instruction
    idt_ref.load();
}

/// Get a reference to the global IDT.
///
/// Returns `None` if the IDT has not been initialized yet.
#[allow(dead_code)]
pub fn get_idt() -> Option<&'static Idt> {
    IDT.get()
}
