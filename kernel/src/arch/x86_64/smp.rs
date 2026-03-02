// =============================================================================
// MinimalOS NextGen — SMP (Symmetric Multi-Processing) Initialization
// =============================================================================
//
// Uses the Limine MpRequest protocol to bring up Application Processors.
// Limine handles the 16-bit real mode → 64-bit long mode transition.
// We just write a function pointer to each AP's goto_address.
//
// CRITICAL: The APs wake up using LIMINE'S page tables. The first instruction
// executed by the AP trampoline MUST be a CR3 swap to our pristine PML4.
// Only after MMU synchronization can the AP safely access kernel heap,
// TCBs, or any Rust data structures.
//
// =============================================================================

use core::arch::naked_asm;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::kprintln;
use crate::arch::{boot, cpu, gdt, lapic};
use crate::memory::pml4;
use crate::sched::percpu::CpuLocal;

use alloc::boxed::Box;

/// Counter for online AP cores. BSP increments this after all APs are started.
static AP_ONLINE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Initializes SMP by waking all Application Processors via Limine MpRequest.
///
/// Must be called after:
/// - Pristine PML4 is built and KERNEL_PML4 is set
/// - BSP GDT, IDT, LAPIC are initialized
pub fn init() {
    let mp_response = match boot::get_mp_response() {
        Some(r) => r,
        None => {
            kprintln!("[smp] No MP response from Limine — single-core mode");
            return;
        }
    };

    let bsp_lapic_id = mp_response.bsp_lapic_id();
    let cpus = mp_response.cpus();
    kprintln!("[smp] BSP LAPIC ID: {}, {} CPUs detected", bsp_lapic_id, cpus.len());

    let mut ap_count = 0u32;

    for cpu_info in cpus.iter() {
        // Skip the BSP — it's already running
        if cpu_info.lapic_id == bsp_lapic_id {
            continue;
        }

        kprintln!("[smp] Waking AP: LAPIC ID {} (ACPI ID {})", cpu_info.lapic_id, cpu_info.id);

        // Write the trampoline function pointer — this atomically wakes the AP
        // The AP will jump to ap_trampoline with &Cpu as its argument
        cpu_info.goto_address.write(ap_trampoline);
        ap_count += 1;
    }

    // Wait for APs to come online (with timeout)
    if ap_count > 0 {
        kprintln!("[smp] Waiting for {} APs to come online...", ap_count);
        let mut timeout = 100_000_000u64; // ~1 second at ~100MHz loop
        while AP_ONLINE_COUNT.load(Ordering::SeqCst) < ap_count && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }

        let online = AP_ONLINE_COUNT.load(Ordering::SeqCst);
        if online == ap_count {
            kprintln!("[smp] All {} APs online", ap_count);
        } else {
            kprintln!("[smp] WARNING: Only {}/{} APs came online", online, ap_count);
        }
    }
}

/// Naked AP trampoline — the first function executed by a waking AP.
///
/// CRITICAL: The AP is still using Limine's page tables at this point.
/// The very first instruction MUST swap CR3 to our pristine PML4.
///
/// After MMU sync, we call the Rust entry point.
///
/// # ABI
/// Limine passes `&Cpu` in `rdi` (System V AMD64 ABI, 1st argument).
#[unsafe(naked)]
unsafe extern "C" fn ap_trampoline(_cpu_info: &limine::mp::Cpu) -> ! {
    naked_asm!(
        // 1. IMMEDIATELY sync the MMU with the BSP's pristine PML4
        "mov rax, qword ptr [rip + {pml4}]",
        "mov cr3, rax",

        // 2. rdi already contains &Cpu — call Rust entry
        "call {entry}",

        // Should never return
        "ud2",

        pml4 = sym pml4::KERNEL_PML4,
        entry = sym ap_rust_entry,
    );
}

/// Rust entry point for APs, called after CR3 is synced.
///
/// Sets up per-core infrastructure: GDT, IDT, GS base, LAPIC.
/// Then enters the scheduler idle loop.
extern "C" fn ap_rust_entry(cpu_info: &limine::mp::Cpu) -> ! {
    let lapic_id = cpu_info.lapic_id;
    let core_index = AP_ONLINE_COUNT.fetch_add(1, Ordering::SeqCst) + 1; // BSP is 0

    // --- 1. Load per-core GDT + TSS ---
    // Each AP needs its own TSS (for IST stacks). For now, we share the GDT
    // but load a per-core TSS. A simplified approach: reload the global GDT.
    // TODO: Per-core TSS with separate IST stacks for full isolation.
    gdt::init();

    // --- 2. Load shared IDT ---
    crate::arch::idt::init();

    // --- 3. Set up CPU-local storage (IA32_GS_BASE) ---
    {
        let mut ap_local = Box::new(CpuLocal::new(lapic_id, core_index));
        unsafe { ap_local.install(); }
        // Leak the Box — CpuLocal lives forever
        let _ = Box::into_raw(ap_local);
    }

    // --- 4. Enable LAPIC ---
    // LAPIC is at the standard 0xFEE00000 for all x86_64 cores
    lapic::init(crate::memory::address::PhysAddr::new(0xFEE0_0000));

    // --- 5. SYSCALL MSR configuration ---
    // Each core needs its own STAR/LSTAR/FMASK MSRs (MSRs are per-core).
    crate::arch::syscall::init();

    kprintln!("[smp] AP core {} (LAPIC {}) online", core_index, lapic_id);

    // --- 5. Enter idle loop ---
    // When the scheduler is fully wired, this becomes scheduler::run()
    loop {
        unsafe { core::arch::asm!("sti"); }
        cpu::halt();
    }
}
