// =============================================================================
// MinimalOS NextGen — Interrupt Descriptor Table (IDT)
// =============================================================================
//
// The IDT maps interrupt/exception vectors (0–255) to handler functions.
// When an interrupt fires, the CPU:
//   1. Looks up the handler in IDT[vector]
//   2. Pushes SS, RSP, RFLAGS, CS, RIP onto the IST/kernel stack
//   3. If error code: pushes it too
//   4. Jumps to the handler
//
// Our handler stubs then:
//   5. Push remaining GPRs to create an InterruptFrame
//   6. Check CS to detect Ring 3 origin → swapgs if needed
//   7. Call the Rust dispatcher with System V ABI (frame ptr in RDI)
//   8. Reverse the swapgs
//   9. Pop GPRs, skip vector+error, iretq
//
// SWAPGS RATIONALE:
// =================
// x86_64 uses the GS segment base for per-CPU data. In Ring 0, GS.base
// points to kernel per-CPU data. In Ring 3, GS.base holds user TLS.
// The SWAPGS instruction atomically swaps GS.base with IA32_KERNEL_GS_BASE.
//
// If an interrupt fires while in Ring 3:
//   - We MUST swapgs to get kernel GS before accessing per-CPU data
//   - We MUST swapgs again before iretq to restore user GS
//
// If an interrupt fires while in Ring 0:
//   - GS is already the kernel's — DO NOT swapgs (double swap = wrong GS)
//
// We detect the origin by checking the CS value pushed by the CPU.
// If CS & 3 == 3, we came from Ring 3 → swapgs.
//
// CS OFFSET IN OUR FRAME:
//   After our stub pushes: 15 GPRs (120 bytes) + vector (8) + error_code (8)
//   Plus CPU-pushed RIP (8), then CS at [rsp + 120 + 16 + 8] = [rsp + 144]
//
// =============================================================================

use core::arch::naked_asm;

use crate::arch::cpu;
use crate::arch::x86_64::gdt;
use crate::kprintln;

// =============================================================================
// IDT Entry
// =============================================================================

/// A single IDT entry (gate descriptor) — 16 bytes.
///
/// Layout (Intel SDM Vol 3A, Figure 6-7):
///   Bytes  0-1:  offset bits 0-15
///   Bytes  2-3:  segment selector (always kernel CS)
///   Byte   4:    IST index (0 = no dedicated stack, 1-7 = IST entry)
///   Byte   5:    type/attributes:
///                  bit 7: present
///                  bits 5-6: DPL (0 = kernel only)
///                  bit 4: zero
///                  bits 0-3: gate type (0xE = interrupt gate, 0xF = trap gate)
///   Bytes  6-7:  offset bits 16-31
///   Bytes  8-11: offset bits 32-63
///   Bytes 12-15: reserved (zero)
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    offset_lo: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_hi: u32,
    reserved: u32,
}

impl IdtEntry {
    /// A non-present (zeroed) entry.
    const EMPTY: Self = Self {
        offset_lo: 0,
        selector: 0,
        ist: 0,
        type_attr: 0,
        offset_mid: 0,
        offset_hi: 0,
        reserved: 0,
    };

    /// Creates an interrupt gate entry pointing to `handler`.
    ///
    /// Interrupt gates automatically clear IF (disable interrupts) on entry.
    /// This prevents re-entrant interrupt handling, which is what we want
    /// for exception handlers and most IRQ handlers.
    ///
    /// # Parameters
    /// - `handler`: address of the naked assembly stub
    /// - `ist_index`: 0 = normal stack, 1-7 = use IST stack from TSS
    fn interrupt_gate(handler: u64, ist_index: u8) -> Self {
        Self {
            offset_lo: handler as u16,
            selector: gdt::KERNEL_CS,
            ist: ist_index & 0x07,
            type_attr: 0x8E, // Present=1, DPL=0, Type=0xE (interrupt gate)
            offset_mid: (handler >> 16) as u16,
            offset_hi: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

// =============================================================================
// Interrupt Frame
// =============================================================================

/// The complete CPU context saved by our interrupt stubs.
///
/// This struct represents the exact stack layout after our assembly stubs
/// push all general-purpose registers. The Rust dispatcher receives a
/// pointer to this struct in RDI (System V ABI first argument).
///
/// Fields are ordered bottom-to-top on the stack (first pushed = highest offset).
#[repr(C)]
#[derive(Debug)]
pub struct InterruptFrame {
    // Pushed by our assembly stub (in this order):
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    // Pushed by our stub:
    pub vector: u64,
    pub error_code: u64,
    // Pushed by CPU on interrupt:
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// =============================================================================
// Exception stub macros
// =============================================================================
//
// Two variants:
//   exception_stub!    — for exceptions that do NOT push an error code
//                        (we push a fake 0 to keep the frame uniform)
//   exception_stub_err! — for exceptions that DO push an error code
//                          (CPU pushes it before our stub runs)

/// Generates a naked exception stub for vectors WITHOUT a CPU error code.
/// Pushes a fake 0 as the error code to keep InterruptFrame uniform.
macro_rules! exception_stub {
    ($name:ident, $vector:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            naked_asm!(
                "push 0",                           // Fake error code
                concat!("push ", $vector),           // Vector number
                "push rax", "push rbx", "push rcx", "push rdx",
                "push rsi", "push rdi", "push rbp",
                "push r8",  "push r9",  "push r10", "push r11",
                "push r12", "push r13", "push r14", "push r15",
                // CS is at [rsp + 144]: 15 GPRs×8 + vec(8) + err(8) + RIP(8) = 144
                "test qword ptr [rsp + 144], 3",    // Check RPL bits of CS
                "jz 1f",
                "swapgs",                           // Ring 3 origin → swap to kernel GS
                "1:",
                "mov rdi, rsp",                     // InterruptFrame* → RDI (System V ABI)
                "call exception_dispatch",
                "test qword ptr [rsp + 144], 3",
                "jz 2f",
                "swapgs",                           // Restore user GS before iretq
                "2:",
                "pop r15", "pop r14", "pop r13", "pop r12",
                "pop r11", "pop r10", "pop r9",  "pop r8",
                "pop rbp", "pop rdi", "pop rsi", "pop rdx",
                "pop rcx", "pop rbx", "pop rax",
                "add rsp, 16",                      // Drop vector + error_code
                "iretq",
            );
        }
    };
}

/// Generates a naked exception stub for vectors WITH a CPU error code.
/// The CPU has already pushed the error code before we get control.
macro_rules! exception_stub_err {
    ($name:ident, $vector:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            naked_asm!(
                // Error code already on stack from CPU
                concat!("push ", $vector),           // Vector number
                "push rax", "push rbx", "push rcx", "push rdx",
                "push rsi", "push rdi", "push rbp",
                "push r8",  "push r9",  "push r10", "push r11",
                "push r12", "push r13", "push r14", "push r15",
                "test qword ptr [rsp + 144], 3",
                "jz 1f",
                "swapgs",
                "1:",
                "mov rdi, rsp",
                "call exception_dispatch",
                "test qword ptr [rsp + 144], 3",
                "jz 2f",
                "swapgs",
                "2:",
                "pop r15", "pop r14", "pop r13", "pop r12",
                "pop r11", "pop r10", "pop r9",  "pop r8",
                "pop rbp", "pop rdi", "pop rsi", "pop rdx",
                "pop rcx", "pop rbx", "pop rax",
                "add rsp, 16",
                "iretq",
            );
        }
    };
}

/// Generates a naked IRQ stub (no CPU error code, used for hardware interrupts).
/// Same swapgs/ABI pattern as exception stubs.
macro_rules! irq_stub {
    ($name:ident, $vector:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            naked_asm!(
                "push 0",                           // No error code for IRQs
                concat!("push ", $vector),
                "push rax", "push rbx", "push rcx", "push rdx",
                "push rsi", "push rdi", "push rbp",
                "push r8",  "push r9",  "push r10", "push r11",
                "push r12", "push r13", "push r14", "push r15",
                "test qword ptr [rsp + 144], 3",
                "jz 1f",
                "swapgs",
                "1:",
                "mov rdi, rsp",
                "call irq_dispatch",
                "test qword ptr [rsp + 144], 3",
                "jz 2f",
                "swapgs",
                "2:",
                "pop r15", "pop r14", "pop r13", "pop r12",
                "pop r11", "pop r10", "pop r9",  "pop r8",
                "pop rbp", "pop rdi", "pop rsi", "pop rdx",
                "pop rcx", "pop rbx", "pop rax",
                "add rsp, 16",
                "iretq",
            );
        }
    };
}

// =============================================================================
// Stub instantiation
// =============================================================================

// Exception stubs (no error code pushed by CPU)
exception_stub!(divide_error_stub, "0");       // #DE - Vector 0
exception_stub!(debug_stub, "1");              // #DB - Vector 1
exception_stub!(nmi_stub, "2");                // NMI - Vector 2
exception_stub!(breakpoint_stub, "3");         // #BP - Vector 3
exception_stub!(overflow_stub, "4");           // #OF - Vector 4
exception_stub!(bound_range_stub, "5");        // #BR - Vector 5
exception_stub!(invalid_opcode_stub, "6");     // #UD - Vector 6
exception_stub!(device_not_avail_stub, "7");   // #NM - Vector 7

// Exception stubs (error code pushed by CPU)
exception_stub_err!(double_fault_stub, "8");   // #DF - Vector 8 (IST1)
exception_stub_err!(invalid_tss_stub, "10");   // #TS - Vector 10
exception_stub_err!(segment_not_present_stub, "11"); // #NP - Vector 11
exception_stub_err!(stack_fault_stub, "12");   // #SS - Vector 12
exception_stub_err!(gpf_stub, "13");           // #GP - Vector 13
exception_stub_err!(page_fault_stub, "14");    // #PF - Vector 14

exception_stub!(x87_fp_stub, "16");            // #MF - Vector 16
exception_stub_err!(alignment_check_stub, "17"); // #AC - Vector 17
exception_stub!(machine_check_stub, "18");     // #MC - Vector 18
exception_stub!(simd_fp_stub, "19");           // #XM - Vector 19

// IRQ stubs (vectors 32–47)
irq_stub!(irq_0_stub, "32");    // LAPIC timer
irq_stub!(irq_1_stub, "33");    // Keyboard (I/O APIC)
irq_stub!(irq_2_stub, "34");    // Cascade (unused with APIC)
irq_stub!(irq_3_stub, "35");    // COM2
irq_stub!(irq_4_stub, "36");    // COM1
irq_stub!(irq_5_stub, "37");    // LPT2 / sound
irq_stub!(irq_6_stub, "38");    // Floppy
irq_stub!(irq_7_stub, "39");    // LPT1 / spurious
irq_stub!(irq_8_stub, "40");    // RTC
irq_stub!(irq_9_stub, "41");    // ACPI
irq_stub!(irq_10_stub, "42");   // Available
irq_stub!(irq_11_stub, "43");   // Available
irq_stub!(irq_12_stub, "44");   // PS/2 Mouse
irq_stub!(irq_13_stub, "45");   // FPU
irq_stub!(irq_14_stub, "46");   // Primary IDE
irq_stub!(irq_15_stub, "47");   // Secondary IDE

// Spurious interrupt vector (255)
irq_stub!(spurious_stub, "255");

// =============================================================================
// Exception dispatcher (called from assembly stubs)
// =============================================================================

/// Central exception dispatcher — called by all exception stubs.
///
/// Receives the full InterruptFrame via RDI (System V ABI).
/// Handles each exception vector appropriately.
#[unsafe(no_mangle)]
pub extern "C" fn exception_dispatch(frame: &InterruptFrame) {
    match frame.vector {
        0 => {
            kprintln!();
            kprintln!("EXCEPTION: #DE Divide Error");
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  CS:     {:#06X}", frame.cs);
            kprintln!("  RFLAGS: {:#018X}", frame.rflags);
            kprintln!("  RSP:    {:#018X}", frame.rsp);
            cpu::halt_forever();
        }

        6 => {
            kprintln!();
            kprintln!("EXCEPTION: #UD Invalid Opcode");
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  CS:     {:#06X}", frame.cs);
            cpu::halt_forever();
        }

        8 => {
            // Double fault — we're on IST1, so logging should work even if
            // the kernel stack is trashed.
            kprintln!();
            kprintln!("==========================================================");
            kprintln!("  EXCEPTION: #DF DOUBLE FAULT — UNRECOVERABLE");
            kprintln!("==========================================================");
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  CS:     {:#06X}", frame.cs);
            kprintln!("  RFLAGS: {:#018X}", frame.rflags);
            kprintln!("  RSP:    {:#018X}", frame.rsp);
            kprintln!("  SS:     {:#06X}", frame.ss);
            kprintln!("  Error:  {:#018X}", frame.error_code);
            kprintln!();
            kprintln!("  RAX={:#018X}  RBX={:#018X}", frame.rax, frame.rbx);
            kprintln!("  RCX={:#018X}  RDX={:#018X}", frame.rcx, frame.rdx);
            kprintln!("  RSI={:#018X}  RDI={:#018X}", frame.rsi, frame.rdi);
            kprintln!("  RBP={:#018X}  R8 ={:#018X}", frame.rbp, frame.r8);
            kprintln!("  R9 ={:#018X}  R10={:#018X}", frame.r9, frame.r10);
            kprintln!("  R11={:#018X}  R12={:#018X}", frame.r11, frame.r12);
            kprintln!("  R13={:#018X}  R14={:#018X}", frame.r13, frame.r14);
            kprintln!("  R15={:#018X}", frame.r15);
            kprintln!("==========================================================");
            cpu::halt_forever();
        }

        13 => {
            kprintln!();
            kprintln!("EXCEPTION: #GP General Protection Fault");
            kprintln!("  Error code: {:#06X}", frame.error_code);
            if frame.error_code != 0 {
                kprintln!("    Selector index: {}", (frame.error_code >> 3) & 0x1FFF);
                kprintln!("    Table: {}", match (frame.error_code >> 1) & 0x03 {
                    0 => "GDT",
                    1 => "IDT",
                    2 => "LDT",
                    3 => "IDT",
                    _ => "unknown",
                });
                kprintln!("    External: {}", if frame.error_code & 1 != 0 { "yes" } else { "no" });
            }
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  CS:     {:#06X}", frame.cs);
            kprintln!("  RSP:    {:#018X}", frame.rsp);
            cpu::halt_forever();
        }

        14 => {
            let cr2 = cpu::read_cr2();
            let err = frame.error_code;
            kprintln!();
            kprintln!("EXCEPTION: #PF Page Fault");
            kprintln!("  Faulting address (CR2): {:#018X}", cr2);
            kprintln!("  Error code: {:#06X}", err);
            kprintln!("    {}",   if err & 1 != 0 { "Protection violation" } else { "Page not present" });
            kprintln!("    {}",   if err & 2 != 0 { "Write access" } else { "Read access" });
            kprintln!("    {}",   if err & 4 != 0 { "User mode" } else { "Supervisor mode" });
            kprintln!("    {}",   if err & 8 != 0 { "Reserved bit set" } else { "No reserved bit violation" });
            kprintln!("    {}",   if err & 16 != 0 { "Instruction fetch (NX)" } else { "Data access" });
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  RSP:    {:#018X}", frame.rsp);
            cpu::halt_forever();
        }

        v @ 0..=31 => {
            kprintln!();
            kprintln!("EXCEPTION: Unhandled exception vector {}", v);
            kprintln!("  RIP:    {:#018X}", frame.rip);
            kprintln!("  Error:  {:#018X}", frame.error_code);
            cpu::halt_forever();
        }

        v => {
            kprintln!("EXCEPTION: Unknown vector {} (should be handled as IRQ)", v);
            cpu::halt_forever();
        }
    }
}

/// Central IRQ dispatcher — called by all IRQ stubs.
///
/// After handling, sends EOI to the LAPIC (required for all APIC-delivered interrupts).
#[unsafe(no_mangle)]
pub extern "C" fn irq_dispatch(frame: &InterruptFrame) {
    let vector = frame.vector;

    match vector {
        32 => {
            // LAPIC timer interrupt — preemptive scheduler tick.
            //
            // CRITICAL: Send EOI BEFORE calling schedule().
            // If we call schedule() first, switch_context will jump to another
            // thread before reaching the EOI at the bottom. The LAPIC will never
            // receive the End-of-Interrupt signal and will never fire again.
            //
            // This is safe because interrupt gates set IF=0 — no re-entrant
            // interrupt can occur between EOI and the context switch.
            crate::arch::lapic::eoi();

            // Trigger the context switch (picks next thread, swaps RSP)
            unsafe { crate::sched::scheduler::schedule(); }

            // Return early — do NOT fall through to the second EOI below
            return;
        }

        255 => {
            // Spurious interrupt — do NOT send EOI.
            // The LAPIC generates these when the interrupt is no longer pending
            // by the time the CPU acknowledges it. Just return.
            return;
        }

        v => {
            kprintln!("[irq] Hardware interrupt: vector {}", v);
        }
    }

    // Send EOI (End of Interrupt) to LAPIC.
    // SAFETY: LAPIC must be initialized before any interrupts fire.
    // We initialize LAPIC before enabling interrupts in main.rs.
    crate::arch::lapic::eoi();
}

// =============================================================================
// IDT table and initialization
// =============================================================================

/// The IDT — 256 entries, each 16 bytes = 4 KiB.
static mut IDT: [IdtEntry; 256] = [IdtEntry::EMPTY; 256];

/// The IDTR value passed to `lidt`.
#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

/// Sets up the IDT with all exception and IRQ handlers, then loads it via `lidt`.
///
/// Must be called after GDT init (we reference KERNEL_CS in the entries).
pub fn init() {
    // Helper: cast a naked function to its address as u64.
    // We go through *const () to avoid the "direct cast of function item" warning.
    macro_rules! handler_addr {
        ($fn:ident) => { $fn as *const () as u64 };
    }

    // SAFETY: We're in single-threaded early boot. The IDT is only written here
    // and then loaded via LIDT. The pointer is valid for the lifetime of the kernel.
    let idt = unsafe { &mut *core::ptr::addr_of_mut!(IDT) };

    // =====================================================================
    // Exception handlers (vectors 0–31)
    // =====================================================================
    idt[0]  = IdtEntry::interrupt_gate(handler_addr!(divide_error_stub), 0);
    idt[1]  = IdtEntry::interrupt_gate(handler_addr!(debug_stub), 0);
    idt[2]  = IdtEntry::interrupt_gate(handler_addr!(nmi_stub), 0);
    idt[3]  = IdtEntry::interrupt_gate(handler_addr!(breakpoint_stub), 0);
    idt[4]  = IdtEntry::interrupt_gate(handler_addr!(overflow_stub), 0);
    idt[5]  = IdtEntry::interrupt_gate(handler_addr!(bound_range_stub), 0);
    idt[6]  = IdtEntry::interrupt_gate(handler_addr!(invalid_opcode_stub), 0);
    idt[7]  = IdtEntry::interrupt_gate(handler_addr!(device_not_avail_stub), 0);
    idt[8]  = IdtEntry::interrupt_gate(handler_addr!(double_fault_stub), 1); // IST1!
    // Vector 9: coprocessor segment overrun (reserved on x86_64)
    idt[10] = IdtEntry::interrupt_gate(handler_addr!(invalid_tss_stub), 0);
    idt[11] = IdtEntry::interrupt_gate(handler_addr!(segment_not_present_stub), 0);
    idt[12] = IdtEntry::interrupt_gate(handler_addr!(stack_fault_stub), 0);
    idt[13] = IdtEntry::interrupt_gate(handler_addr!(gpf_stub), 0);
    idt[14] = IdtEntry::interrupt_gate(handler_addr!(page_fault_stub), 0);
    // Vector 15: reserved
    idt[16] = IdtEntry::interrupt_gate(handler_addr!(x87_fp_stub), 0);
    idt[17] = IdtEntry::interrupt_gate(handler_addr!(alignment_check_stub), 0);
    idt[18] = IdtEntry::interrupt_gate(handler_addr!(machine_check_stub), 0);
    idt[19] = IdtEntry::interrupt_gate(handler_addr!(simd_fp_stub), 0);

    // =====================================================================
    // IRQ handlers (vectors 32–47)
    // =====================================================================
    idt[32] = IdtEntry::interrupt_gate(handler_addr!(irq_0_stub), 0);   // LAPIC timer
    idt[33] = IdtEntry::interrupt_gate(handler_addr!(irq_1_stub), 0);   // Keyboard
    idt[34] = IdtEntry::interrupt_gate(handler_addr!(irq_2_stub), 0);   // Cascade
    idt[35] = IdtEntry::interrupt_gate(handler_addr!(irq_3_stub), 0);   // COM2
    idt[36] = IdtEntry::interrupt_gate(handler_addr!(irq_4_stub), 0);   // COM1
    idt[37] = IdtEntry::interrupt_gate(handler_addr!(irq_5_stub), 0);   // LPT2
    idt[38] = IdtEntry::interrupt_gate(handler_addr!(irq_6_stub), 0);   // Floppy
    idt[39] = IdtEntry::interrupt_gate(handler_addr!(irq_7_stub), 0);   // LPT1
    idt[40] = IdtEntry::interrupt_gate(handler_addr!(irq_8_stub), 0);   // RTC
    idt[41] = IdtEntry::interrupt_gate(handler_addr!(irq_9_stub), 0);   // ACPI
    idt[42] = IdtEntry::interrupt_gate(handler_addr!(irq_10_stub), 0);
    idt[43] = IdtEntry::interrupt_gate(handler_addr!(irq_11_stub), 0);
    idt[44] = IdtEntry::interrupt_gate(handler_addr!(irq_12_stub), 0);  // PS/2 Mouse
    idt[45] = IdtEntry::interrupt_gate(handler_addr!(irq_13_stub), 0);
    idt[46] = IdtEntry::interrupt_gate(handler_addr!(irq_14_stub), 0);
    idt[47] = IdtEntry::interrupt_gate(handler_addr!(irq_15_stub), 0);

    // =====================================================================
    // Spurious interrupt vector (255)
    // =====================================================================
    idt[255] = IdtEntry::interrupt_gate(handler_addr!(spurious_stub), 0);

    // =====================================================================
    // Load the IDT via LIDT
    // =====================================================================
    let idt_ptr = IdtPointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: idt.as_ptr() as u64,
    };

    unsafe {
        core::arch::asm!(
            "lidt [{}]",
            in(reg) &idt_ptr,
            options(nostack, preserves_flags),
        );
    }

    kprintln!("[idt] IDT loaded: {} exception handlers, {} IRQ handlers, spurious @255",
        15, 16); // 15 exception vectors registered + 16 IRQ stubs
}
