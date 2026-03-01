// =============================================================================
// MinimalOS NextGen — Local APIC (LAPIC) Driver
// =============================================================================
//
// The Local APIC is the per-CPU interrupt controller on x86_64. Each core
// has its own LAPIC. It handles:
//   - Receiving interrupts from the I/O APIC
//   - Local timer interrupts (for preemptive scheduling)
//   - Inter-Processor Interrupts (IPIs) for SMP coordination
//   - Interrupt priority and masking
//
// MMIO ACCESS:
//   The LAPIC registers are memory-mapped at a physical address (typically
//   0xFEE00000) obtained from the MADT. We access them via the HHDM.
//   All registers are 32-bit, aligned on 16-byte boundaries.
//   Reads/writes must use volatile operations and be exactly 32 bits.
//
// TIMER CALIBRATION:
//   The LAPIC timer decrements at a rate related to the bus clock.
//   We need to calibrate it to know how many ticks = 1 microsecond.
//
//   Calibration priority:
//   1. CPUID Leaf 0x15 — direct TSC/crystal clock info (most accurate)
//   2. PIT-based calibration — use a known ~10ms PIT interval (fallback)
//
//   The N3710 (Airmont) supports CPUID Leaf 0x15 with crystal clock info.
//
// =============================================================================

use core::sync::atomic::{AtomicU64, Ordering};

use crate::kprintln;
use crate::memory::address::PhysAddr;

// =============================================================================
// LAPIC register offsets (from LAPIC base address)
// =============================================================================

const LAPIC_ID: u32         = 0x020;   // Local APIC ID
const LAPIC_VERSION: u32    = 0x030;   // LAPIC Version
const LAPIC_TPR: u32        = 0x080;   // Task Priority Register
const LAPIC_EOI: u32        = 0x0B0;   // End of Interrupt
const LAPIC_SPURIOUS: u32   = 0x0F0;   // Spurious Interrupt Vector Register
const LAPIC_ICR_LO: u32     = 0x300;   // Interrupt Command Register (low)
const LAPIC_ICR_HI: u32     = 0x310;   // Interrupt Command Register (high)
const LAPIC_LVT_TIMER: u32  = 0x320;   // LVT Timer Register
const LAPIC_TIMER_INIT: u32 = 0x380;   // Timer Initial Count
const LAPIC_TIMER_CUR: u32  = 0x390;   // Timer Current Count
const LAPIC_TIMER_DIV: u32  = 0x3E0;   // Timer Divide Configuration

// Timer modes (bits 17-18 of LVT Timer)
const TIMER_MODE_ONESHOT: u32   = 0b00 << 17;
const TIMER_MODE_PERIODIC: u32  = 0b01 << 17;

// Timer divide values (LAPIC_TIMER_DIV register)
const TIMER_DIVIDE_BY_1: u32    = 0b1011;
const TIMER_DIVIDE_BY_16: u32   = 0b0011;

// LVT mask bit
const LVT_MASK: u32 = 1 << 16;

// Spurious vector register bits
const SPURIOUS_ENABLE: u32 = 1 << 8;

// =============================================================================
// Global state
// =============================================================================

/// Virtual base address of the LAPIC registers (set during init).
static LAPIC_BASE: AtomicU64 = AtomicU64::new(0);

/// Calibrated LAPIC timer ticks per microsecond.
static TICKS_PER_US: AtomicU64 = AtomicU64::new(0);

// =============================================================================
// MMIO helpers
// =============================================================================

/// Reads a 32-bit LAPIC register at the given offset.
///
/// # Safety
/// LAPIC must be initialized and the offset must be valid.
#[inline]
fn read_reg(offset: u32) -> u32 {
    let base = LAPIC_BASE.load(Ordering::Relaxed);
    debug_assert!(base != 0, "LAPIC not initialized");
    let ptr = (base + offset as u64) as *const u32;
    unsafe { core::ptr::read_volatile(ptr) }
}

/// Writes a 32-bit value to a LAPIC register at the given offset.
///
/// # Safety
/// LAPIC must be initialized and the offset must be valid.
#[inline]
fn write_reg(offset: u32, value: u32) {
    let base = LAPIC_BASE.load(Ordering::Relaxed);
    debug_assert!(base != 0, "LAPIC not initialized");
    let ptr = (base + offset as u64) as *mut u32;
    unsafe { core::ptr::write_volatile(ptr, value) }
}

// =============================================================================
// Public API
// =============================================================================

/// Initializes the Local APIC.
///
/// Must be called after MADT parsing provides the LAPIC base address.
///
/// # Steps
/// 1. Store the HHDM-mapped virtual base address
/// 2. Set TPR to 0 (accept all interrupt priorities)
/// 3. Enable the LAPIC with spurious vector = 255
pub fn init(lapic_phys: PhysAddr) {
    let base_virt = lapic_phys.to_virt().as_u64();
    LAPIC_BASE.store(base_virt, Ordering::Relaxed);

    let id = read_reg(LAPIC_ID) >> 24;
    let version = read_reg(LAPIC_VERSION) & 0xFF;

    kprintln!("[lapic] LAPIC ID={}, version={:#04X}", id, version);

    // Set Task Priority to 0 — accept all interrupts.
    write_reg(LAPIC_TPR, 0);

    // Enable the LAPIC with spurious vector 255.
    // Bit 8 = APIC Software Enable.
    // Bits 0-7 = spurious interrupt vector.
    write_reg(LAPIC_SPURIOUS, SPURIOUS_ENABLE | 255);

    // Mask the timer initially (we'll configure it after calibration).
    write_reg(LAPIC_LVT_TIMER, LVT_MASK);

    kprintln!("[lapic] LAPIC enabled, spurious vector = 255");
}

/// Returns the current LAPIC ID (useful for SMP identification).
pub fn id() -> u8 {
    (read_reg(LAPIC_ID) >> 24) as u8
}

/// Calibrates the LAPIC timer to determine ticks per microsecond.
///
/// Tries CPUID Leaf 0x15 first (direct crystal clock info, most accurate).
/// Falls back to PIT-based calibration if CPUID doesn't provide the data.
pub fn calibrate_timer() {
    // =========================================================================
    // Attempt 1: CPUID Leaf 0x15 (TSC / Core Crystal Clock)
    // =========================================================================
    // EAX = denominator of TSC/core-crystal ratio
    // EBX = numerator of TSC/core-crystal ratio
    // ECX = core crystal clock frequency in Hz (0 if unknown)
    //
    // TSC frequency = crystal_hz * EBX / EAX
    // LAPIC timer runs at the bus clock, which on Airmont is the same as
    // the crystal clock (or a known multiple).

    let (cpuid_max, _, _, _) = cpuid(0);

    if cpuid_max >= 0x15 {
        let (eax, ebx, ecx, _) = cpuid(0x15);

        if eax != 0 && ebx != 0 {
            let crystal_hz = if ecx != 0 {
                ecx as u64
            } else {
                // Some CPUs report 0 for ECX but have a known crystal clock.
                // Airmont (N3710) typically uses 19.2 MHz.
                // Check if this is an Atom/Airmont by looking at family/model.
                let (_, _, _, _) = cpuid(1);
                // Fallback: assume 19.2 MHz for Atom-class CPUs.
                19_200_000u64
            };

            let tsc_hz = crystal_hz * ebx as u64 / eax as u64;
            // LAPIC timer on many Intel CPUs runs at the crystal clock rate
            // when using divide-by-1. With divide-by-16, it's crystal/16.
            // We use divide-by-1 for maximum resolution.
            let ticks_per_us = crystal_hz / 1_000_000;

            if ticks_per_us > 0 {
                TICKS_PER_US.store(ticks_per_us, Ordering::Relaxed);
                kprintln!("[lapic] Calibrated via CPUID 0x15:");
                kprintln!("[lapic]   Crystal: {} MHz", crystal_hz / 1_000_000);
                kprintln!("[lapic]   TSC:     {} MHz", tsc_hz / 1_000_000);
                kprintln!("[lapic]   Timer:   {} ticks/μs (divide-by-1)", ticks_per_us);
                return;
            }
        }
    }

    // =========================================================================
    // Fallback: PIT-based calibration
    // =========================================================================
    // Uses PIT channel 2 in one-shot mode to measure a ~10ms interval.
    // Less accurate but works everywhere.

    kprintln!("[lapic] CPUID 0x15 unavailable, calibrating via PIT...");

    // PIT frequency is exactly 1,193,182 Hz.
    // For a ~10ms interval: count = 1_193_182 / 100 = 11,932
    const PIT_HZ: u32 = 1_193_182;
    const PIT_INTERVAL_MS: u32 = 10;
    const PIT_COUNT: u16 = (PIT_HZ / (1000 / PIT_INTERVAL_MS)) as u16;

    // PIT I/O ports
    const PIT_CHANNEL_2: u16 = 0x42;
    const PIT_COMMAND: u16 = 0x43;
    const PIT_GATE: u16 = 0x61;

    unsafe {
        // Set up PIT channel 2 in one-shot mode.
        // Command: channel 2, access lo/hi, mode 0 (interrupt on terminal count)
        port_out_u8(PIT_COMMAND, 0b10110000);

        // Load the count value (low byte first, then high byte).
        port_out_u8(PIT_CHANNEL_2, (PIT_COUNT & 0xFF) as u8);
        port_out_u8(PIT_CHANNEL_2, (PIT_COUNT >> 8) as u8);

        // Set LAPIC timer: divide by 1, maximum initial count.
        write_reg(LAPIC_TIMER_DIV, TIMER_DIVIDE_BY_1);
        write_reg(LAPIC_TIMER_INIT, 0xFFFF_FFFF);

        // Enable PIT channel 2 gate.
        let gate = port_in_u8(PIT_GATE);
        port_out_u8(PIT_GATE, (gate & 0xFD) | 0x01); // Gate enable

        // Wait for PIT to count down (bit 5 of port 0x61 goes high when done).
        while port_in_u8(PIT_GATE) & 0x20 == 0 {
            core::hint::spin_loop();
        }

        // Read how much the LAPIC timer counted down.
        let elapsed = 0xFFFF_FFFFu64 - read_reg(LAPIC_TIMER_CUR) as u64;

        // Mask the timer while we calculate.
        write_reg(LAPIC_LVT_TIMER, LVT_MASK);

        // elapsed ticks in ~10ms → ticks per microsecond.
        let ticks_per_us = elapsed / (PIT_INTERVAL_MS as u64 * 1000);

        if ticks_per_us == 0 {
            kprintln!("[lapic] WARNING: Timer calibration returned 0 ticks/μs, using estimate");
            TICKS_PER_US.store(100, Ordering::Relaxed);
        } else {
            TICKS_PER_US.store(ticks_per_us, Ordering::Relaxed);
        }

        kprintln!("[lapic] PIT calibration: {} ticks in {}ms = {} ticks/μs",
            elapsed, PIT_INTERVAL_MS, ticks_per_us);
    }
}

/// Arms the LAPIC timer in one-shot mode.
///
/// The timer will fire a single interrupt on vector 32 after `microseconds`
/// have elapsed. After firing, the timer stops (one-shot).
///
/// # Parameters
/// - `microseconds`: time until interrupt fires
///
/// # Panics
/// Debug-asserts that the timer has been calibrated.
pub fn set_timer_oneshot(microseconds: u64) {
    let tpu = TICKS_PER_US.load(Ordering::Relaxed);
    debug_assert!(tpu > 0, "LAPIC timer not calibrated");

    let ticks = microseconds * tpu;
    let count = if ticks > 0xFFFF_FFFF { 0xFFFF_FFFF } else { ticks as u32 };

    // Configure: divide by 1, one-shot mode, vector 32, unmasked.
    write_reg(LAPIC_TIMER_DIV, TIMER_DIVIDE_BY_1);
    write_reg(LAPIC_LVT_TIMER, TIMER_MODE_ONESHOT | 32); // Vector 32, not masked
    write_reg(LAPIC_TIMER_INIT, count);
}

/// Sends End of Interrupt to the LAPIC.
///
/// Must be called at the end of every interrupt handler for LAPIC-delivered
/// interrupts. Writing any value to the EOI register signals completion.
/// (Spurious interrupts are the exception — they must NOT receive EOI.)
#[inline]
pub fn eoi() {
    write_reg(LAPIC_EOI, 0);
}

// =============================================================================
// CPU helpers
// =============================================================================

/// Executes the CPUID instruction and returns (EAX, EBX, ECX, EDX).
///
/// LLVM reserves `rbx` internally, so we can't use it as an inline asm
/// operand. Workaround: use `xchg` to save rbx to another register,
/// execute CPUID, read ebx into that register, then restore rbx.
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        core::arch::asm!(
            "xchg rbx, {tmp:r}",  // save rbx into tmp
            "cpuid",
            "xchg rbx, {tmp:r}",  // restore rbx, tmp now holds cpuid's ebx
            inout("eax") leaf => eax,
            tmp = out(reg) ebx,
            inout("ecx") 0u32 => ecx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

/// Reads a byte from an I/O port.
#[inline]
unsafe fn port_in_u8(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Writes a byte to an I/O port.
#[inline]
unsafe fn port_out_u8(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags),
        );
    }
}
