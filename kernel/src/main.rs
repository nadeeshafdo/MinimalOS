// =============================================================================
// MinimalOS NextGen — Kernel Entry Point
// =============================================================================
//
// This is the first Rust code that runs when the kernel boots.
//
// WHAT HAPPENED BEFORE WE GOT HERE:
//   1. You pressed the power button on your HP 15-ay028tu
//   2. UEFI firmware initialized hardware (RAM training, PCIe, USB)
//   3. UEFI loaded the Limine bootloader from the EFI System Partition
//   4. Limine read its config, found our kernel binary
//   5. Limine loaded our ELF binary into physical memory
//   6. Limine set up 64-bit long mode with paging:
//      - Identity map of low memory (for its own use)
//      - Higher-half map of the kernel at 0xFFFFFFFF80000000+
//      - HHDM (all physical RAM mapped at a fixed offset)
//   7. Limine filled in our request structures (memory map, framebuffer, etc.)
//   8. Limine jumped to our entry point: kmain()
//
// WHAT WE DO HERE:
//   Phase 1: "Deaf and Blind" → Get serial output working
//   Phase 2: "Can See"        → Get framebuffer output working
//   Phase 3: "Can Remember"   → Initialize memory management
//   Phase 4: "Can Think"      → Initialize scheduler + processes
//   Phase 5: "Alive"          → Load init process, enter userspace
//
// Currently implementing through Phase 2. Subsequent phases come in
// later sprints as we build each subsystem.
//
// =============================================================================

// =============================================================================
// Crate-level attributes
// =============================================================================
//
// #![no_std] — We don't link against Rust's standard library.
//   The standard library depends on an operating system (for files, threads,
//   networking, etc.). We ARE the operating system, so we provide those things.
//   We only use `core` (language primitives) and `alloc` (heap, once we set
//   up an allocator).
//
// #![no_main] — We don't use Rust's normal entry point.
//   Normally, Rust programs start at `fn main()` after the runtime does
//   setup (stack guard, args parsing, etc.). We have no runtime — the
//   bootloader jumps directly to our `kmain()` function.
//
// Feature gates for unstable features we need:
//   - `asm_const` — allows using const values in inline assembly
// =============================================================================

#![no_std]
#![no_main]
// Allow dead code during development — foundation APIs are used in future sprints.
#![allow(dead_code)]
// Required for #[alloc_error_handler] in heap.rs
#![feature(alloc_error_handler)]

// Enable the `alloc` crate for heap-allocated types (Vec, Box, String, etc.).
// This works because we provide a #[global_allocator] in memory::heap.
extern crate alloc;

// =============================================================================
// Module declarations
// =============================================================================
//
// These tell the Rust compiler about our module tree. Each `mod` declaration
// corresponds to a file or directory under `src/`.
// =============================================================================

/// Architecture-specific code (x86_64 HAL).
/// Contains: serial, CPU utilities, boot protocol, GDT, IDT, paging, etc.
mod arch;

/// Memory management subsystem.
/// Contains: physical/virtual address types, PMM, VMM, kernel heap.
mod memory;

/// Synchronization primitives.
/// Contains: ticket spinlock, sleeping mutex (future), once-cell.
mod sync;

/// Kernel utility modules.
/// Contains: kprint!/kprintln! logging macros, panic handler.
mod util;

/// In-kernel drivers (boot-critical only).
/// Contains: framebuffer text console, LAPIC timer (future).
mod drivers;

/// Scheduler subsystem.
/// Contains: CPU-local storage, threads, context switching, scheduler.
mod sched;

/// Capability subsystem.
/// Contains: CNode (per-thread capability table), rights, kernel object references.
mod cap;

/// Inter-process communication subsystem.
/// Contains: IPC message format, synchronous endpoints.
mod ipc;

// =============================================================================
// Imports
// =============================================================================

use arch::boot;
use arch::serial::SERIAL;
use memory::address;
use memory::address::PhysAddr;
use memory::pmm;
use memory::heap;

// =============================================================================
// Linker-provided symbols
// =============================================================================
//
// These symbols are defined in `kernel/linker.ld`. They mark the boundaries
// of each section in the kernel binary. We use them to:
//   1. Calculate the kernel's total size in memory
//   2. Know which physical frames the kernel occupies (don't free them!)
//   3. Set correct page permissions per section (W^X enforcement)
//
// IMPORTANT: These are NOT variables. They're linker symbols. To get their
// address, we take a reference: `&_kernel_start as *const u8 as usize`.
// Reading their VALUE is undefined behavior — only their ADDRESS is meaningful.
// =============================================================================
unsafe extern "C" {
    static _kernel_start: u8;
    static _kernel_end: u8;
    static _text_start: u8;
    static _text_end: u8;
    static _rodata_start: u8;
    static _rodata_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _bss_start: u8;
    static _bss_end: u8;
}

// =============================================================================
// Kernel Entry Point
// =============================================================================

/// The kernel's main entry point.
///
/// Called by the Limine bootloader after setting up 64-bit long mode,
/// paging, and the higher-half kernel mapping.
///
/// # Execution Environment
/// When we enter this function:
///   - CPU is in 64-bit long mode
///   - Paging is enabled (Limine's page tables)
///   - The kernel is mapped in the higher half
///   - All physical RAM is accessible via the HHDM
///   - A stack is set up (Limine-provided, ~64KB)
///   - Interrupts are DISABLED (we haven't set up an IDT yet)
///   - Only BSP (Bootstrap Processor, core 0) is running
///   - AP cores are halted, waiting for SIPI
///
/// # Never Returns
/// This function initializes the kernel subsystems and then enters the
/// scheduler loop. It never returns to the bootloader.
#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    // =========================================================================
    // PHASE 1: "Deaf and Blind" → Get serial output working
    // =========================================================================
    //
    // Before this phase, we have NO output. If something crashes here,
    // we get a silent hang. Keep this phase as simple as possible.
    // =========================================================================

    // Initialize the serial UART (COM1) for debug output.
    // After this call, kprintln!() works over serial.
    // This touches only I/O ports — no memory allocation, no page tables.
    {
        let serial = SERIAL.lock();
        serial.init();
    }

    // First sign of life! If you see this on serial, the kernel is alive.
    kprintln!();
    kprintln!("==========================================================");
    kprintln!("  MinimalOS NextGen v0.1.0");
    kprintln!("  Capability-based microkernel for x86_64");
    kprintln!("==========================================================");
    kprintln!();

    // =========================================================================
    // PHASE 2: "Can See" → Parse boot info, init framebuffer
    // =========================================================================
    //
    // Now we have serial output. We can print debug messages if things go
    // wrong. Parse the boot information Limine provided.
    // =========================================================================

    // --- HHDM (Higher Half Direct Map) ---
    // Get the offset where all physical memory is mapped in virtual space.
    // This is critical — without it, we can't convert between physical
    // and virtual addresses.
    let hhdm_offset = boot::get_hhdm_offset();
    kprintln!("[boot] HHDM offset: {:#018X}", hhdm_offset);

    // Initialize the global HHDM offset so PhysAddr::to_virt() works.
    // SAFETY: Called once during single-core boot, before any other use.
    unsafe {
        address::init_hhdm(hhdm_offset);
    }

    // --- Kernel location ---
    // Where is the kernel loaded in physical and virtual memory?
    let (kernel_phys, kernel_virt) = boot::get_kernel_address();
    let kernel_size = unsafe {
        &_kernel_end as *const u8 as usize - &_kernel_start as *const u8 as usize
    };
    kprintln!("[boot] Kernel physical base: {:#010X}", kernel_phys);
    kprintln!("[boot] Kernel virtual base:  {:#018X}", kernel_virt);
    kprintln!("[boot] Kernel size:          {} KiB ({} pages)",
        kernel_size / 1024,
        (kernel_size + 4095) / 4096
    );

    // Print section layout for debugging.
    kprintln!("[boot] Sections:");
    unsafe {
        kprintln!("  .text:   {:#018X} — {:#018X} ({} bytes)",
            &_text_start as *const u8 as usize,
            &_text_end as *const u8 as usize,
            &_text_end as *const u8 as usize - &_text_start as *const u8 as usize);
        kprintln!("  .rodata: {:#018X} — {:#018X} ({} bytes)",
            &_rodata_start as *const u8 as usize,
            &_rodata_end as *const u8 as usize,
            &_rodata_end as *const u8 as usize - &_rodata_start as *const u8 as usize);
        kprintln!("  .data:   {:#018X} — {:#018X} ({} bytes)",
            &_data_start as *const u8 as usize,
            &_data_end as *const u8 as usize,
            &_data_end as *const u8 as usize - &_data_start as *const u8 as usize);
        kprintln!("  .bss:    {:#018X} — {:#018X} ({} bytes)",
            &_bss_start as *const u8 as usize,
            &_bss_end as *const u8 as usize,
            &_bss_end as *const u8 as usize - &_bss_start as *const u8 as usize);
    }

    // --- Memory Map ---
    // Print the physical memory map from Limine.
    // This tells us which regions of RAM are usable.
    let memory_map = boot::get_memory_map();
    kprintln!();
    kprintln!("[boot] Physical memory map ({} entries):", memory_map.len());

    let mut total_usable: u64 = 0;
    let mut total_memory: u64 = 0;

    for entry in memory_map.iter() {
        let base = entry.base;
        let length = entry.length;
        let end = base + length;
        let entry_type = entry.entry_type;

        let type_str = match entry_type {
            limine::memory_map::EntryType::USABLE => {
                total_usable += length;
                "Usable"
            }
            limine::memory_map::EntryType::RESERVED => "Reserved",
            limine::memory_map::EntryType::ACPI_RECLAIMABLE => "ACPI Reclaimable",
            limine::memory_map::EntryType::ACPI_NVS => "ACPI NVS",
            limine::memory_map::EntryType::BAD_MEMORY => "Bad Memory",
            limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE => "Bootloader Reclaimable",
            limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => "Kernel & Modules",
            limine::memory_map::EntryType::FRAMEBUFFER => "Framebuffer",
            _ => "Unknown",
        };

        total_memory += length;
        kprintln!("  {:#012X} — {:#012X}  {:>10} KiB  {}",
            base, end, length / 1024, type_str);
    }

    kprintln!();
    kprintln!("[boot] Total memory:  {} MiB", total_memory / 1024 / 1024);
    kprintln!("[boot] Usable memory: {} MiB ({} pages)",
        total_usable / 1024 / 1024,
        total_usable / 4096
    );

    // --- ACPI (for future hardware discovery) ---
    if let Some(rsdp) = boot::get_rsdp_address() {
        kprintln!("[boot] ACPI RSDP at: {:#018X}", rsdp);
    } else {
        kprintln!("[boot] WARNING: No ACPI RSDP found (hardware discovery limited)");
    }

    // --- Framebuffer ---
    // Initialize the framebuffer console for on-screen text output.
    if let Some(fb_info) = boot::get_framebuffer_info() {
        kprintln!("[boot] Framebuffer: {}x{} @ {} bpp, pitch={} bytes",
            fb_info.width, fb_info.height, fb_info.bpp, fb_info.pitch);
        kprintln!("[boot] Framebuffer address: {:p}", fb_info.address);

        // Initialize the framebuffer console.
        // After this, we could also write to the screen.
        drivers::framebuffer::init(fb_info);

        // Write the boot banner to the framebuffer too.
        drivers::framebuffer::write_fmt(format_args!(
            "MinimalOS NextGen v0.1.0\n\
             Capability-based microkernel for x86_64\n\
             \n\
             Kernel loaded at {:#018X} ({} KiB)\n\
             Usable memory: {} MiB\n\
             Framebuffer: {}x{}\n\
             \n",
            kernel_virt,
            kernel_size / 1024,
            total_usable / 1024 / 1024,
            fb_info.width, fb_info.height,
        ));

        kprintln!("[boot] Framebuffer console initialized");
    } else {
        kprintln!("[boot] WARNING: No framebuffer available (serial only)");
    }

    // =========================================================================
    // PHASE 3: "Can Remember" → Memory Management (Sprint 2)
    // =========================================================================
    //
    // Initialize the memory subsystem bottom-up:
    //   1. PMM — track which physical frames are free / used
    //   2. Kernel heap — enable alloc crate (Vec, Box, String)
    //   3. VMM infrastructure is ready but we don't switch page tables yet
    //      (deferred to Sprint 3 when we have IDT for debugging faults)
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 3: Memory management");

    // --- Physical Memory Manager ---
    // Build the bitmap from the Limine memory map. After this call,
    // pmm::alloc_frame() and pmm::free_frame() are available.
    pmm::init(memory_map);

    let mem_stats = pmm::stats();
    kprintln!(
        "[pmm] {} total frames, {} used, {} free ({} MiB free)",
        mem_stats.total_frames,
        mem_stats.used_frames,
        mem_stats.free_frames,
        mem_stats.free_frames as u64 * 4096 / 1024 / 1024,
    );

    // --- Kernel Heap ---
    // Allocate contiguous physical pages from the PMM and set up the
    // linked-list heap allocator. After this call, alloc::Vec and friends work.
    heap::init();

    // Verify the heap works with a quick test allocation.
    {
        use alloc::vec::Vec;
        let mut v: Vec<u64> = Vec::new();
        v.push(42);
        v.push(1337);
        v.push(0xDEAD_BEEF);
        kprintln!(
            "[heap] Test allocation OK: {:?} (heap used: {} bytes)",
            v,
            heap::allocated_bytes(),
        );
        // Vec is dropped here, memory returned to the heap.
    }

    kprintln!(
        "[heap] After drop: {} bytes used / {} KiB total",
        heap::allocated_bytes(),
        heap::total_bytes() / 1024,
    );

    // --- VMM (infrastructure only) ---
    // The page table types and manipulation functions (map_page, unmap_page,
    // translate) are available in memory::vmm but we don't switch away from
    // Limine's page tables yet. That requires IDT/exception handlers for
    // safe debugging (Sprint 3).
    kprintln!("[vmm] Page table infrastructure ready (CR3 switch deferred to Sprint 3)");

    // =========================================================================
    // PHASE 4: "Can Think" → Interrupts & Exceptions (Sprint 3)
    // =========================================================================
    //
    // Boot the interrupt subsystem bottom-up:
    //   1. GDT + TSS (segments for IDT, IST stacks for double fault)
    //   2. IDT (exception handlers become active)
    //   3. ACPI → MADT (discover LAPIC / I/O APIC topology)
    //   4. Disable legacy 8259 PIC
    //   5. LAPIC enable + timer calibration
    //   6. I/O APIC routing
    //   7. Enable interrupts (STI)
    //   8. Test: arm one-shot LAPIC timer
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 4: Interrupts & exceptions");

    // --- 4a. GDT + TSS ---
    // Must come before IDT — the IDT entries reference the GDT's kernel CS
    // selector, and the TSS provides the IST1 stack for double fault.
    arch::gdt::init();

    // --- 4b. IDT ---
    // Exception handlers (divide error, GPF, page fault, double fault) and
    // IRQ stubs (vectors 32-47) are now armed.
    // Note: interrupts are still disabled (IF=0). The IDT is loaded but
    // no interrupts will fire until we STI.
    arch::idt::init();

    // --- 4c. Map ACPI regions into HHDM ---
    // Limine base revision 3 only maps Usable/Bootloader/Kernel regions in
    // the HHDM. ACPI Reclaimable, ACPI NVS, and Reserved regions (which
    // contain RSDP, XSDT, MADT, and LAPIC/IOAPIC MMIO) are NOT mapped.
    // We must map them before the ACPI parser can safely dereference them.
    {
        use memory::vmm::{self, PageTableFlags};

        let cr3 = PhysAddr::new(arch::cpu::read_cr3() & !0xFFF);
        let mut mapped_pages = 0u64;

        for entry in memory_map.iter() {
            let et = entry.entry_type;
            // Only map ACPI Reclaimable and ACPI NVS — these hold RSDP/XSDT/MADT.
            // Reserved regions include huge MMIO ranges (12+ TB) that we
            // must NOT try to map page-by-page. LAPIC/IOAPIC MMIO regions
            // are mapped on-demand by their respective drivers.
            let needs_mapping =
                et == limine::memory_map::EntryType::ACPI_RECLAIMABLE ||
                et == limine::memory_map::EntryType::ACPI_NVS;

            if !needs_mapping || entry.length > 16 * 1024 * 1024 {
                continue;
            }

            // Map each 4K page in this region into the HHDM.
            let base = entry.base & !0xFFF; // page-align down
            let end = (entry.base + entry.length + 0xFFF) & !0xFFF; // page-align up
            let mut phys = base;
            while phys < end {
                let pa = PhysAddr::new(phys);
                let va = pa.to_virt();
                // SAFETY: Single-core early boot, mapping read-only ACPI data.
                let result = unsafe {
                    vmm::map_page(
                        cr3,
                        va,
                        pa,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    )
                };
                match result {
                    Ok(()) => {
                        // Flush TLB for this page
                        arch::cpu::invlpg(va.as_u64());
                        mapped_pages += 1;
                    }
                    Err(vmm::MapError::AlreadyMapped) => {
                        // Already mapped (e.g., by Limine's huge pages) — fine
                    }
                    Err(vmm::MapError::HugePageConflict) => {
                        // A parent is a huge page covering this range — fine
                    }
                    Err(e) => {
                        kprintln!("[vmm] WARNING: Failed to map ACPI page @ {:#010X}: {:?}",
                            phys, e);
                    }
                }
                phys += 0x1000;
            }
        }

        if mapped_pages > 0 {
            kprintln!("[vmm] Mapped {} ACPI/Reserved pages into HHDM", mapped_pages);
        }
    }

    // --- 4d. ACPI → MADT ---
    // Parse the MADT to discover LAPIC and I/O APIC topology.
    // XSDT is used exclusively for ACPI 2.0+ (64-bit pointers).
    let madt_info = if let Some(rsdp) = boot::get_rsdp_address() {
        let info = arch::acpi::parse_madt(rsdp);
        kprintln!("[acpi] Summary: LAPIC @ {:#010X}, {} CPUs, {} I/O APICs, {} overrides",
            info.lapic_addr, info.cpu_count, info.ioapic_count, info.override_count);
        Some(info)
    } else {
        kprintln!("[acpi] WARNING: No RSDP found — cannot configure APIC");
        None
    };

    // --- 4e. Map LAPIC / I/O APIC MMIO into HHDM ---
    // Limine rev3 does not map device MMIO regions. The LAPIC (0xFEE00000)
    // and I/O APIC(s) must be explicitly mapped with uncacheable attributes
    // before their drivers can read/write the hardware registers.
    if let Some(ref madt) = madt_info {
        use memory::vmm::{self, PageTableFlags};

        let cr3 = PhysAddr::new(arch::cpu::read_cr3() & !0xFFF);
        let mmio_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_CACHE
            | PageTableFlags::NO_EXECUTE;

        // Map the LAPIC page (typically 0xFEE00000, 4K is enough)
        let lapic_phys = PhysAddr::new(madt.lapic_addr & !0xFFF);
        let lapic_virt = lapic_phys.to_virt();
        match unsafe { vmm::map_page(cr3, lapic_virt, lapic_phys, mmio_flags) } {
            Ok(()) => {
                arch::cpu::invlpg(lapic_virt.as_u64());
                kprintln!("[vmm] Mapped LAPIC MMIO at {:#010X}", lapic_phys.as_u64());
            }
            Err(vmm::MapError::AlreadyMapped) | Err(vmm::MapError::HugePageConflict) => {}
            Err(e) => kprintln!("[vmm] WARNING: Failed to map LAPIC MMIO: {:?}", e),
        }

        // Map each I/O APIC page
        for i in 0..madt.ioapic_count {
            let ioapic_phys = PhysAddr::new(madt.ioapics[i].address & !0xFFF);
            let ioapic_virt = ioapic_phys.to_virt();
            match unsafe { vmm::map_page(cr3, ioapic_virt, ioapic_phys, mmio_flags) } {
                Ok(()) => {
                    arch::cpu::invlpg(ioapic_virt.as_u64());
                    kprintln!("[vmm] Mapped I/O APIC #{} MMIO at {:#010X}",
                        madt.ioapics[i].id, ioapic_phys.as_u64());
                }
                Err(vmm::MapError::AlreadyMapped) | Err(vmm::MapError::HugePageConflict) => {}
                Err(e) => kprintln!("[vmm] WARNING: Failed to map I/O APIC MMIO: {:?}", e),
            }
        }
    }

    // --- 4f. Disable legacy 8259 PIC ---
    // Must happen before enabling I/O APIC to prevent spurious legacy IRQs.
    arch::ioapic::disable_pic();

    // --- 4e. LAPIC ---
    // Enable the local APIC and calibrate the timer.
    if let Some(ref madt) = madt_info {
        arch::lapic::init(PhysAddr::new(madt.lapic_addr));
        arch::lapic::calibrate_timer();
    } else {
        kprintln!("[lapic] SKIPPED — no MADT available");
    }

    // --- 4f. I/O APIC ---
    // Initialize each I/O APIC from the MADT and route interrupts.
    if let Some(ref madt) = madt_info {
        for i in 0..madt.ioapic_count {
            let ioapic = &madt.ioapics[i];
            arch::ioapic::init(
                PhysAddr::new(ioapic.address),
                ioapic.gsi_base,
                &madt.overrides[..madt.override_count],
            );
        }

        // Enable COM1 serial interrupt (IRQ 4).
        // Check if there's an ISO override for IRQ 4.
        let com1_gsi = {
            let mut gsi = 4u32; // Default: IRQ 4 → GSI 4
            for j in 0..madt.override_count {
                if madt.overrides[j].irq_source == 4 {
                    gsi = madt.overrides[j].gsi;
                    break;
                }
            }
            gsi
        };
        arch::ioapic::enable_irq(com1_gsi, 36, 0);
    }

    // --- 4g. Pristine PML4 + CR3 swap ---
    // Build clean page tables replacing Limine's contaminated ones.
    // Maps HHDM (2M huge), kernel W^X (4K), MMIO (uncacheable).
    let pristine_pml4 = memory::pml4::build();
    unsafe { memory::pml4::activate(pristine_pml4); }

    // --- 4h. Enable interrupts ---
    // Everything is set up. STI allows the CPU to begin processing
    // hardware interrupts.
    unsafe { core::arch::asm!("sti"); }
    kprintln!("[init] Interrupts ENABLED");

    // =========================================================================
    // PHASE 5: Scheduler + Threads (Sprint 4)
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 5: Scheduler initialization");

    // --- 5a. BSP CpuLocal ---
    // Set up per-core local storage on the BSP before any thread creation.
    {
        let mut bsp_local = alloc::boxed::Box::new(
            sched::percpu::CpuLocal::new(0, 0) // BSP = LAPIC 0, core 0
        );
        unsafe { bsp_local.install(); }
        // Leak the Box so CpuLocal lives forever (it's per-core kernel state)
        let _ = alloc::boxed::Box::into_raw(bsp_local);
    }

    // =========================================================================
    // PHASE 6: Userspace & Syscalls (Sprint 6)
    // =========================================================================
    //
    // Ring 0 → Ring 3 privilege transition. We push execution down into
    // Ring 3 where the CPU will trap and fault on any unauthorized action,
    // forcing user threads to use SYSCALL to request IPC through capabilities.
    //
    //   1. Configure SYSCALL MSRs (STAR, LSTAR, FMASK)
    //   2. Create IPC endpoint + register in syscall table
    //   3. Map user code + stack pages at user-accessible addresses
    //   4. Spawn kernel receiver + user sender threads
    //   5. User sender executes SYSCALL → capability validation → IPC
    //
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 6: Userspace & Syscalls");

    // --- 6a. SYSCALL MSR initialization ---
    // Configure IA32_STAR, IA32_LSTAR, IA32_FMASK on the BSP.
    // APs will call this in ap_rust_entry (smp.rs).
    arch::syscall::init();

    // --- 6b. Create IPC endpoint + register in syscall table ---
    // Endpoint ID=1, used by both the kernel receiver and user sender.
    let test_ep = alloc::boxed::Box::new(ipc::endpoint::Endpoint::new(1));
    let test_ep_ptr = alloc::boxed::Box::into_raw(test_ep);
    // Register in the global endpoint table so syscall_dispatch can find it.
    unsafe { arch::syscall::register_endpoint(test_ep_ptr); }
    // Also keep a raw pointer for the kernel receiver thread.
    unsafe { IPC_TEST_ENDPOINT = test_ep_ptr; }

    // --- 6c. Map user code page ---
    // Allocate a physical frame, map it at a user-accessible virtual address,
    // and write the user test program (raw x86_64 machine code) into it.
    let user_code_virt = {
        use memory::vmm::{self, PageTableFlags};

        let code_phys = memory::pmm::alloc_frame_zeroed()
            .expect("[init] FATAL: cannot allocate user code page");
        let code_virt = memory::address::VirtAddr::new(0x0000_0000_0040_0000);
        let cr3 = memory::address::PhysAddr::new(arch::cpu::read_cr3() & !0xFFF);

        // Map with USER + PRESENT (executable, not writable).
        // NX is intentionally NOT set — this is a code page.
        unsafe {
            vmm::map_page(cr3, code_virt, code_phys,
                PageTableFlags::PRESENT | PageTableFlags::USER,
            ).expect("[init] FATAL: cannot map user code page");
            arch::cpu::invlpg(code_virt.as_u64());
        }

        // Write user test program directly to the mapped page via HHDM.
        //
        // User program: sends 2 IPC messages via SYSCALL, then loops forever.
        //
        //   mov rax, 1          ; SYS_SEND
        //   mov rdi, 0          ; CNode slot 0 (endpoint capability)
        //   mov rsi, 0xBEEF     ; message label
        //   mov rdx, 0x1111     ; data[0]
        //   mov r10, 0x2222     ; data[1]
        //   syscall
        //
        //   mov rax, 1          ; SYS_SEND (second message)
        //   mov rdi, 0          ; slot 0
        //   mov rsi, 0xCAFE     ; label
        //   mov rdx, 0x3333     ; data[0]
        //   mov r10, 0x4444     ; data[1]
        //   syscall
        //
        //   jmp $               ; infinite loop (can't HLT in Ring 3)
        //
        let user_code_bytes: &[u8] = &[
            // --- Message 1: label=0xBEEF, data=[0x1111, 0x2222] ---
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,  // mov rax, 1
            0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00,  // mov rdi, 0
            0x48, 0xC7, 0xC6, 0xEF, 0xBE, 0x00, 0x00,  // mov rsi, 0xBEEF
            0x48, 0xC7, 0xC2, 0x11, 0x11, 0x00, 0x00,  // mov rdx, 0x1111
            0x49, 0xC7, 0xC2, 0x22, 0x22, 0x00, 0x00,  // mov r10, 0x2222
            0x0F, 0x05,                                  // syscall
            // --- Message 2: label=0xCAFE, data=[0x3333, 0x4444] ---
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,  // mov rax, 1
            0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00,  // mov rdi, 0
            0x48, 0xC7, 0xC6, 0xFE, 0xCA, 0x00, 0x00,  // mov rsi, 0xCAFE
            0x48, 0xC7, 0xC2, 0x33, 0x33, 0x00, 0x00,  // mov rdx, 0x3333
            0x49, 0xC7, 0xC2, 0x44, 0x44, 0x00, 0x00,  // mov r10, 0x4444
            0x0F, 0x05,                                  // syscall
            // --- Done: loop forever ---
            0xEB, 0xFE,                                  // jmp $ (infinite loop)
        ];

        // Copy code to the physical frame (via HHDM virtual mapping)
        let code_dst = code_phys.to_virt().as_u64() as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(
                user_code_bytes.as_ptr(),
                code_dst,
                user_code_bytes.len(),
            );
        }

        kprintln!("[init] User code page: {:#018X} → phys {:#010X} ({} bytes)",
            code_virt.as_u64(), code_phys.as_u64(), user_code_bytes.len());

        code_virt.as_u64()
    };

    // --- 6d. Map user stack page ---
    let user_stack_top = {
        use memory::vmm::{self, PageTableFlags};

        let stack_phys = memory::pmm::alloc_frame_zeroed()
            .expect("[init] FATAL: cannot allocate user stack page");
        let stack_virt = memory::address::VirtAddr::new(0x0000_0000_0080_0000);
        let cr3 = memory::address::PhysAddr::new(arch::cpu::read_cr3() & !0xFFF);

        // Map with USER + WRITABLE + NO_EXECUTE (data page, not code).
        unsafe {
            vmm::map_page(cr3, stack_virt, stack_phys,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE
                    | PageTableFlags::USER | PageTableFlags::NO_EXECUTE,
            ).expect("[init] FATAL: cannot map user stack page");
            arch::cpu::invlpg(stack_virt.as_u64());
        }

        // Stack grows down. RSP starts at the top of the page.
        let top = stack_virt.as_u64() + memory::address::PAGE_SIZE as u64;
        kprintln!("[init] User stack page: {:#018X} — {:#018X} (RSP={:#018X})",
            stack_virt.as_u64(), top, top);
        top
    };

    // --- 6e. Spawn kernel receiver thread ---
    // This kernel thread uses the proven ep.recv() to receive messages
    // sent by the Ring 3 user thread. Proves the full path:
    //   Ring 3 → SYSCALL → capability validation → IPC send → kernel recv
    sched::scheduler::spawn("kern-recv", userspace_test_receiver, 0);

    // --- 6f. Create user sender thread with capabilities ---
    {
        use cap::cnode::{CapObject, CapRights, Capability};

        let mut user_sender = arch::syscall::spawn_user(
            "user-send", user_code_virt, user_stack_top);

        // Install Endpoint capability at CNode slot 0.
        // The user code references slot 0 in its SYSCALL (mov rdi, 0).
        // WRITE right allows SYS_SEND on this endpoint.
        user_sender.cnode.insert_at(0, Capability::new(
            CapObject::Endpoint { id: 1 },
            CapRights::WRITE,
        )).expect("[init] FATAL: cannot install endpoint capability");

        kprintln!("[init] User sender: CNode slot 0 = Endpoint(id=1, WRITE)");
        sched::scheduler::spawn_thread(user_sender);
    }

    // --- 6g. Initialize scheduler ---
    // Drains BOOT_QUEUE into BSP run queue, arms LAPIC timer.
    sched::scheduler::init();

    // --- 6h. Start Application Processors ---
    arch::smp::init();

    // =========================================================================
    // BSP IDLE LOOP
    // =========================================================================
    kprintln!();
    kprintln!("==========================================================");
    kprintln!("  Sprint 6 — Userspace & Syscalls LIVE!");
    kprintln!("  SYSCALL/SYSRET, Ring 3 execution, capability-gated IPC");
    kprintln!("  User sender (Ring 3) → SYSCALL → EP1 → kernel receiver");
    kprintln!("  BSP entering idle loop.");
    kprintln!("==========================================================");

    loop {
        arch::cpu::halt(); // hlt with IF=1 — LAPIC timer will wake us
    }
}

// =============================================================================
// IPC Test Infrastructure (Sprint 6: Ring 3 → Ring 0 IPC)
// =============================================================================

/// Global pointer to the test endpoint. Written once during Phase 6b,
/// read by the kernel receiver thread.
static mut IPC_TEST_ENDPOINT: *mut ipc::endpoint::Endpoint = core::ptr::null_mut();

/// Returns a reference to the global test endpoint.
///
/// # Safety
/// Must only be called after Phase 6b has initialized IPC_TEST_ENDPOINT.
unsafe fn get_test_endpoint() -> &'static ipc::endpoint::Endpoint {
    unsafe { &*IPC_TEST_ENDPOINT }
}

/// Kernel receiver thread — receives IPC messages from the Ring 3 sender.
///
/// This runs in Ring 0 and uses the direct ep.recv() API (no syscall needed).
/// It proves the full userspace IPC path:
///   Ring 3 user code → SYSCALL → syscall_dispatch → capability check → ep.send()
///   → this thread's ep.recv() → message printed to serial
pub extern "C" fn userspace_test_receiver(_arg: u64) {
    kprintln!("[ring3-test] Kernel receiver thread started (Ring 0)");

    for i in 0..2 {
        kprintln!("[ring3-test] Receiver: calling recv() (iteration {})", i);
        let ep = unsafe { get_test_endpoint() };
        let msg = ep.recv();
        kprintln!("[ring3-test] Receiver: GOT MESSAGE from Ring 3!");
        kprintln!("[ring3-test]   label={:#X}, data=[{:#X}, {:#X}, {:#X}, {:#X}]",
            msg.label, msg.regs[0], msg.regs[1], msg.regs[2], msg.regs[3]);
    }

    kprintln!("[ring3-test] ================================================");
    kprintln!("[ring3-test] SUCCESS: 2 messages received from Ring 3 sender!");
    kprintln!("[ring3-test] Ring 3 → SYSCALL → Capability → IPC PROVEN!");
    kprintln!("[ring3-test] ================================================");
    loop { arch::cpu::halt(); }
}
