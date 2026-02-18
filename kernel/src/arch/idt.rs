//! Interrupt Descriptor Table (IDT) for x86_64.
//!
//! The IDT is a data structure used by x86_64 processors to determine the
//! correct response to interrupts and exceptions.

use core::arch::asm;
use core::mem::size_of;

/// Number of entries in the IDT.
/// x86_64 supports 256 interrupt vectors (0-255).
const IDT_ENTRIES: usize = 256;

/// Privilege level used in segment selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum PrivilegeLevel {
    /// Ring 0 (kernel mode).
    Ring0 = 0,
    /// Ring 1 (rarely used).
    Ring1 = 1,
    /// Ring 2 (rarely used).
    Ring2 = 2,
    /// Ring 3 (user mode).
    Ring3 = 3,
}

/// Gate type for IDT entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum GateType {
    /// Interrupt gate - disables interrupts on entry.
    Interrupt = 0b1110,
    /// Trap gate - does not disable interrupts on entry.
    Trap = 0b1111,
}

/// Options for an IDT entry.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct EntryOptions(u16);

#[allow(dead_code)]
impl EntryOptions {
    /// Create a new EntryOptions with default values.
    ///
    /// Default: present=false, DPL=Ring0, gate_type=Interrupt, IST=0
    #[inline]
    pub const fn new() -> Self {
        Self(0b0000_1110_0000_0000)
    }

    /// Set the present bit. Must be set for valid entries.
    #[inline]
    pub const fn set_present(mut self, present: bool) -> Self {
        if present {
            self.0 |= 1 << 15;
        } else {
            self.0 &= !(1 << 15);
        }
        self
    }

    /// Set the Descriptor Privilege Level (DPL).
    /// Determines which privilege level can invoke this interrupt via `int` instruction.
    #[inline]
    pub const fn set_privilege_level(mut self, dpl: PrivilegeLevel) -> Self {
        self.0 = (self.0 & 0x9FFF) | ((dpl as u16) << 13);
        self
    }

    /// Set the gate type (interrupt or trap).
    #[inline]
    pub const fn set_gate_type(mut self, gate_type: GateType) -> Self {
        self.0 = (self.0 & 0xF0FF) | ((gate_type as u16) << 8);
        self
    }

    /// Set the Interrupt Stack Table (IST) index.
    /// IST is used for safe exception handling (e.g., double fault).
    /// Valid values: 0 (no IST) or 1-7 (IST index).
    #[inline]
    pub const fn set_stack_index(mut self, ist_index: u8) -> Self {
        self.0 = (self.0 & 0xFFF8) | (ist_index as u16 & 0x7);
        self
    }

    /// Get the raw value.
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

impl Default for EntryOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry in the Interrupt Descriptor Table (IDT).
///
/// Format (128 bits / 16 bytes):
/// - Bits 0-15:   Offset bits 0-15
/// - Bits 16-31:  Code segment selector
/// - Bits 32-47:  Options (IST, gate type, DPL, present)
/// - Bits 48-63:  Offset bits 16-31
/// - Bits 64-95:  Offset bits 32-63
/// - Bits 96-127: Reserved (must be 0)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: EntryOptions,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

#[allow(dead_code)]
impl IdtEntry {
    /// Create a new IDT entry that is not present (disabled).
    #[inline]
    pub const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            options: EntryOptions::new(),
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Create a new IDT entry pointing to a handler function.
    ///
    /// # Arguments
    ///
    /// * `handler` - Function pointer to the interrupt handler
    /// * `selector` - Code segment selector (usually 0x08 for kernel code)
    /// * `options` - Entry options (gate type, privilege level, IST, etc.)
    #[inline]
    pub const fn new(handler: usize, selector: u16, options: EntryOptions) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            options,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    /// Set the handler function for this entry.
    #[inline]
    pub fn set_handler(&mut self, handler: usize) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
    }

    /// Set the options for this entry.
    #[inline]
    pub fn set_options(&mut self, options: EntryOptions) {
        self.options = options;
    }

    /// Get the handler address.
    #[inline]
    pub fn handler(&self) -> usize {
        (self.offset_low as usize)
            | ((self.offset_mid as usize) << 16)
            | ((self.offset_high as usize) << 32)
    }
}

/// The Interrupt Descriptor Table (IDT).
///
/// Contains 256 entries for all possible interrupt vectors.
#[repr(C, align(16))]
pub struct Idt {
    entries: [IdtEntry; IDT_ENTRIES],
}

#[allow(dead_code)]
impl Idt {
    /// Create a new IDT with all entries marked as missing.
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); IDT_ENTRIES],
        }
    }

    /// Set an interrupt handler for a specific vector.
    ///
    /// # Arguments
    ///
    /// * `vector` - Interrupt vector number (0-255)
    /// * `handler` - Function pointer to the handler
    /// * `selector` - Code segment selector
    /// * `options` - Entry options
    pub fn set_handler(
        &mut self,
        vector: u8,
        handler: usize,
        selector: u16,
        options: EntryOptions,
    ) {
        self.entries[vector as usize] = IdtEntry::new(handler, selector, options);
    }

    /// Get a reference to an IDT entry.
    #[inline]
    pub fn entry(&self, vector: u8) -> &IdtEntry {
        &self.entries[vector as usize]
    }

    /// Get a mutable reference to an IDT entry.
    #[inline]
    pub fn entry_mut(&mut self, vector: u8) -> &mut IdtEntry {
        &mut self.entries[vector as usize]
    }

    /// Load this IDT into the CPU using the `lidt` instruction.
    pub fn load(&'static self) {
        let ptr = IdtPointer {
            limit: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            asm!(
                "lidt [{}]",
                in(reg) &ptr,
                options(readonly, nostack, preserves_flags)
            );
        }
    }
}

/// Pointer structure for the `lidt` instruction.
///
/// Format (80 bits / 10 bytes):
/// - Bits 0-15:  Limit (size of IDT - 1)
/// - Bits 16-79: Base address of the IDT
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}
