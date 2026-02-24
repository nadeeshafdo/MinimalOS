// =============================================================================
// MinimalOS NextGen — CPU Utilities (x86_64)
// =============================================================================
//
// Low-level CPU operations that don't fit in a specific subsystem.
// These are thin wrappers around privileged x86_64 instructions.
//
// DESIGN: These functions are the "bottom" of the abstraction stack.
// They're called by higher-level kernel code (scheduler, memory manager)
// and should have minimal logic — just execute the instruction and return.
//
// N3710 SPECIFIC NOTES:
//   - Airmont microarchitecture (14nm Silvermont derivative)
//   - Supports: SSE4.2, AES-NI, PCLMULQDQ, VT-x
//   - Does NOT support: AVX, AVX2, TSX, SGX
//   - Has: 4 cores, no hyper-threading
//   - Max physical address bits: 34 (16GB addressable)
//   - Supports C-states C1 through C6 for power saving
//
// =============================================================================

/// Halts the CPU until the next interrupt arrives.
///
/// This is the kernel's idle instruction. When a core has nothing to run,
/// it executes HLT to:
///   1. Stop executing instructions (saves power)
///   2. Enter a low-power C-state (N3710 supports C1-C6)
///   3. Wake up when an interrupt arrives (timer, IPI, device IRQ)
///
/// On the N3710 (6W TDP laptop chip), this is critical for battery life.
/// An idle core burning cycles in a busy loop would waste ~1.5W per core.
///
/// # Prerequisites
/// Interrupts must be enabled (STI executed) before calling this.
/// If interrupts are disabled, HLT will hang forever.
///
/// # Pattern
/// ```
/// // Typical idle loop:
/// loop {
///     enable_interrupts();
///     halt();
///     // Interrupt fired, we wake up here
///     // Check if there's work to do...
/// }
/// ```
#[inline(always)]
pub fn halt() {
    // SAFETY: HLT is a privileged instruction that stops CPU execution
    // until an interrupt fires. This is always safe in kernel mode
    // as long as interrupts are (or will be) enabled.
    unsafe {
        core::arch::asm!("hlt", options(nomem, nostack));
    }
}

/// Halts the CPU in an unrecoverable state.
///
/// Disables interrupts and then halts. The CPU will never wake up.
/// Used for fatal errors (double fault, panic) where we can't continue.
///
/// This function never returns.
#[inline(always)]
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: CLI + HLT in a loop ensures the CPU stays stopped.
        // No interrupt can wake us because interrupts are disabled.
        unsafe {
            core::arch::asm!(
                "cli",
                "hlt",
                options(nomem, nostack)
            );
        }
    }
}

/// Reads the current value of the CR2 register.
///
/// CR2 contains the linear (virtual) address that caused the most recent
/// page fault. The page fault handler reads this to determine WHICH address
/// the process tried to access.
///
/// # Returns
/// The faulting virtual address as a raw u64.
///
/// # When to use
/// Only meaningful inside a page fault handler (IDT vector 14).
/// At other times, CR2 holds stale data from the last page fault.
#[inline]
pub fn read_cr2() -> u64 {
    let value: u64;
    // SAFETY: Reading CR2 is a privileged operation but has no side effects.
    // It simply returns the value the CPU stored during the last page fault.
    unsafe {
        core::arch::asm!(
            "mov {}, cr2",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Reads the current value of the CR3 register.
///
/// CR3 contains the physical address of the current PML4 (top-level page
/// table). Switching CR3 switches the entire virtual address space.
///
/// # Returns
/// The physical address of the current PML4 page table root.
///
/// # Uses
/// - Saving the current address space before a context switch
/// - Debugging: verify which page tables are active
#[inline]
pub fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: Reading CR3 is privileged but has no side effects.
    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Writes a new value to the CR3 register, switching address spaces.
///
/// This is one of the most critical operations in the kernel:
///   1. Loads a new PML4 physical address into CR3
///   2. The CPU flushes the entire TLB (Translation Lookaside Buffer)
///   3. All subsequent memory accesses use the new page tables
///
/// # Safety
/// The new PML4 must:
///   - Be a valid, properly structured 4-level page table
///   - Map the currently executing kernel code (or we crash immediately)
///   - Be at a 4KB-aligned physical address
///
/// TLB flush on N3710 takes ~50-100 cycles. We minimize CR3 writes
/// by skipping the switch when the new CR3 equals the current one
/// (same-process thread switch).
pub unsafe fn write_cr3(value: u64) {
    // SAFETY: Caller guarantees the page tables are valid.
    // This instruction flushes the entire TLB.
    unsafe {
        core::arch::asm!(
            "mov cr3, {}",
            in(reg) value,
            options(nostack, preserves_flags)
        );
    }
}

/// Invalidates a single page in the TLB.
///
/// When we change a single page table entry (e.g., mapping a new page),
/// we don't need to flush the ENTIRE TLB — we just invalidate the entry
/// for that specific virtual address.
///
/// On N3710: INVLPG takes ~10-20 cycles vs ~50-100 cycles for full TLB flush.
/// For single-page changes, this is 5x faster.
///
/// # Parameters
/// - `addr`: The virtual address whose TLB entry should be invalidated.
///
/// # Note
/// On multi-core systems, this only invalidates the TLB on the CURRENT core.
/// Other cores that may have cached this translation need a TLB shootdown IPI.
#[inline]
pub fn invlpg(addr: u64) {
    // SAFETY: Invalidating a TLB entry is always safe. At worst, it causes
    // a harmless extra page table walk on the next access to that address.
    unsafe {
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) addr,
            options(nostack, preserves_flags)
        );
    }
}

/// Reads the Time Stamp Counter (TSC).
///
/// The TSC is a 64-bit counter that increments on every CPU clock cycle
/// (or at a fixed rate on modern CPUs with "invariant TSC").
///
/// # Uses
/// - High-precision timing (sub-nanosecond on 2.56GHz N3710)
/// - Calibrating the LAPIC timer
/// - Random number seed (combined with other sources)
///
/// # N3710 Note
/// The N3710 has an invariant TSC (constant rate regardless of C-state
/// or frequency scaling), making it reliable for timekeeping.
#[inline]
pub fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: RDTSC is available on all x86_64 CPUs and has no side effects.
    // It returns the 64-bit TSC in EDX:EAX.
    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// Reads a Model-Specific Register (MSR).
///
/// MSRs are CPU configuration registers accessed by index. Each x86_64
/// CPU model has different MSRs. Common ones we use:
///   - 0xC000_0080 (EFER): Extended Feature Enable Register
///   - 0xC000_0081 (STAR): Syscall segment selectors
///   - 0xC000_0082 (LSTAR): Syscall entry point (RIP)
///   - 0xC000_0084 (SFMASK): Syscall RFLAGS mask
///   - 0x0000_001B (APIC_BASE): Local APIC base address
///
/// # Safety
/// The MSR index must be valid for this CPU model. Reading an invalid
/// MSR causes a General Protection Fault (#GP).
#[inline]
pub unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: Caller guarantees the MSR index is valid.
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// Writes a value to a Model-Specific Register (MSR).
///
/// # Safety
/// - The MSR index must be valid for this CPU model
/// - The value must be appropriate for that MSR
/// - Writing incorrect values can crash the system or corrupt state
#[inline]
pub unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nomem, nostack)
        );
    }
}
