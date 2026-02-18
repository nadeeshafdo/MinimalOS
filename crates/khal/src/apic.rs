//! Local APIC (Advanced Programmable Interrupt Controller) driver.
//!
//! The Local APIC is the modern interrupt controller on x86_64 systems.
//! Each CPU core has its own Local APIC. It handles:
//! - Local interrupt sources (timer, thermal, performance counters)
//! - Inter-Processor Interrupts (IPI)
//! - External interrupt routing from the I/O APIC
//!
//! The APIC registers are memory-mapped starting at the base address
//! stored in the IA32_APIC_BASE MSR (0x1B).

use core::ptr;

/// IA32_APIC_BASE Model Specific Register.
const IA32_APIC_BASE_MSR: u32 = 0x1B;

/// Bit 11 of IA32_APIC_BASE MSR: Global APIC enable/disable.
const APIC_BASE_ENABLE: u64 = 1 << 11;

// --- APIC Register Offsets (from APIC base address) ---

/// Local APIC ID Register.
const APIC_REG_ID: u32 = 0x020;
/// Local APIC Version Register.
const APIC_REG_VERSION: u32 = 0x030;
/// Task Priority Register.
const APIC_REG_TPR: u32 = 0x080;
/// End of Interrupt Register.
const APIC_REG_EOI: u32 = 0x0B0;
/// Spurious Interrupt Vector Register.
const APIC_REG_SVR: u32 = 0x0F0;
/// LVT Timer Register.
const APIC_REG_LVT_TIMER: u32 = 0x320;
/// Timer Initial Count Register.
const APIC_REG_TIMER_INIT: u32 = 0x380;
/// Timer Current Count Register.
#[allow(dead_code)]
const APIC_REG_TIMER_CURRENT: u32 = 0x390;
/// Timer Divide Configuration Register.
const APIC_REG_TIMER_DIV: u32 = 0x3E0;

/// SVR bit 8: APIC Software Enable.
const SVR_APIC_ENABLE: u32 = 1 << 8;

/// Spurious interrupt vector number (must be 0xF0-0xFF range recommended).
pub const SPURIOUS_VECTOR: u8 = 0xFF;

/// Timer interrupt vector number.
pub const TIMER_VECTOR: u8 = 32;

/// Timer mode: Periodic (bit 17 set).
const TIMER_PERIODIC: u32 = 1 << 17;

/// Timer divider values for APIC_REG_TIMER_DIV.
#[allow(dead_code)]
#[repr(u32)]
pub enum TimerDivide {
    By1   = 0b1011,
    By2   = 0b0000,
    By4   = 0b0001,
    By8   = 0b0010,
    By16  = 0b0011,
    By32  = 0b1000,
    By64  = 0b1001,
    By128 = 0b1010,
}

/// The Local APIC virtual base address (set during initialization).
static mut APIC_BASE: u64 = 0;

/// Read a Model Specific Register (MSR).
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack, preserves_flags)
    );
    (high as u64) << 32 | low as u64
}

/// Write a Model Specific Register (MSR).
#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a 32-bit APIC register.
#[inline]
unsafe fn read_reg(offset: u32) -> u32 {
    let addr = APIC_BASE + offset as u64;
    ptr::read_volatile(addr as *const u32)
}

/// Write a 32-bit APIC register.
#[inline]
unsafe fn write_reg(offset: u32, value: u32) {
    let addr = APIC_BASE + offset as u64;
    ptr::write_volatile(addr as *mut u32, value);
}

/// Initialize and enable the Local APIC.
///
/// This function:
/// 1. Reads the APIC base address from the IA32_APIC_BASE MSR
/// 2. Converts to a virtual address using the HHDM offset
/// 3. Ensures the global APIC enable bit is set
/// 4. Sets the Spurious Interrupt Vector Register to enable the APIC
/// 5. Sets the Task Priority Register to 0 (accept all interrupts)
///
/// # Arguments
///
/// * `hhdm_offset` - The Higher Half Direct Map offset from Limine
///
/// Returns the APIC ID of the current processor.
pub fn init(hhdm_offset: u64) -> u32 {
    unsafe {
        // Read the APIC base address from MSR (this is a physical address)
        let msr_value = rdmsr(IA32_APIC_BASE_MSR);
        let phys_base = msr_value & 0xFFFF_FFFF_FFFF_F000; // Mask to page-aligned address

        // The APIC registers live in MMIO space (typically 0xFEE00000).
        // The caller must have mapped this page before calling init().
        // Use the HHDM offset to compute the virtual address.
        APIC_BASE = hhdm_offset + phys_base;

        // Ensure the global APIC enable bit is set
        if msr_value & APIC_BASE_ENABLE == 0 {
            wrmsr(IA32_APIC_BASE_MSR, msr_value | APIC_BASE_ENABLE);
        }

        // Set Spurious Interrupt Vector Register:
        // - Set the spurious vector number (bits 0-7)
        // - Set the APIC Software Enable bit (bit 8)
        let svr = SVR_APIC_ENABLE | SPURIOUS_VECTOR as u32;
        write_reg(APIC_REG_SVR, svr);

        // Set Task Priority Register to 0 (accept all priority levels)
        write_reg(APIC_REG_TPR, 0);

        // Read and return the APIC ID
        let id = read_reg(APIC_REG_ID) >> 24;
        id
    }
}

/// Send an End of Interrupt (EOI) signal to the Local APIC.
///
/// This must be called at the end of every interrupt handler for
/// APIC-sourced interrupts (timer, IPI, etc.).
pub fn eoi() {
    unsafe {
        write_reg(APIC_REG_EOI, 0);
    }
}

/// Get the Local APIC version.
#[allow(dead_code)]
pub fn version() -> u32 {
    unsafe { read_reg(APIC_REG_VERSION) }
}

/// Enable the Local APIC Timer in periodic mode.
///
/// # Arguments
///
/// * `vector` - Interrupt vector number for timer interrupts
/// * `initial_count` - Timer initial count value
/// * `divider` - Timer frequency divider
pub fn enable_timer(vector: u8, initial_count: u32, divider: TimerDivide) {
    unsafe {
        // Set the timer divider
        write_reg(APIC_REG_TIMER_DIV, divider as u32);

        // Configure LVT Timer: vector number + periodic mode
        write_reg(APIC_REG_LVT_TIMER, TIMER_PERIODIC | vector as u32);

        // Set initial count (starts the timer)
        write_reg(APIC_REG_TIMER_INIT, initial_count);
    }
}

/// Disable the Local APIC Timer.
#[allow(dead_code)]
pub fn disable_timer() {
    unsafe {
        // Mask the timer (bit 16 = mask)
        let lvt = read_reg(APIC_REG_LVT_TIMER);
        write_reg(APIC_REG_LVT_TIMER, lvt | (1 << 16));
    }
}
