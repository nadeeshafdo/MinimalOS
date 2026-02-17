//! IDT initialization and management.

use crate::arch::idt::Idt;
use spin::Once;

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
    let idt = Idt::new();

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
