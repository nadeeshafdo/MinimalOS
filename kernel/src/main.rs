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

/// Filesystem parsers (initrd TarFS, ELF loader).
/// Contains: USTAR TAR parser, ELF64 executable loader.
mod fs;

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
    arch::gdt::init(0);

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
    // PHASE 6: SYSCALL MSR Initialization (Sprint 6 → Sprint 7)
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 6: SYSCALL MSR initialization");

    // --- 6a. SYSCALL MSR initialization ---
    arch::syscall::init();

    // =========================================================================
    // PHASE 7: Userspace Serial Driver via ELF + TarFS (Sprint 8)
    // =========================================================================
    //
    // The era of flat binaries is over. The kernel now:
    //   1. Reads the initrd TAR archive (loaded by Limine as a boot module)
    //   2. Locates the serial_drv ELF binary within the archive
    //   3. Parses the ELF64 headers and loads PT_LOAD segments
    //   4. Zeroes .bss (p_memsz - p_filesz) — no more garbage statics
    //   5. Extracts the entry point (e_entry) from the ELF header
    //   6. Spawns the driver as a Ring 3 thread with capabilities
    //
    // This is the proper process loading pipeline for a real microkernel.
    //
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 7: ELF Loader + TarFS Initrd + Process Isolation");

    // --- 7a. Load the initrd TAR archive from Limine modules ---
    let initrd = {
        let modules = boot::get_modules()
            .expect("[init] FATAL: no boot modules — is initrd.tar in limine.conf?");
        kprintln!("[init] Limine loaded {} boot module(s)", modules.len());

        let module = modules[0];
        let addr = module.addr();
        let size = module.size() as usize;
        let path = module.path().to_bytes();
        kprintln!("[init] Module 0: {:?} ({} bytes at {:p})",
            core::str::from_utf8(path).unwrap_or("<invalid>"), size, addr);

        // Create a byte slice over the module's memory.
        // SAFETY: Limine loaded this into physical memory and maps it via HHDM.
        // The data is read-only and lives for the entire kernel lifetime.
        unsafe { core::slice::from_raw_parts(addr, size) }
    };

    // --- 7b. List initrd contents ---
    kprintln!("[initrd] Archive contents:");
    fs::tar::for_each_file(initrd, |name, size| {
        kprintln!("[initrd]   {} ({} bytes)", name, size);
    });

    // --- 7c. Locate serial_drv ELF in the initrd ---
    let serial_drv_elf = fs::tar::find_file(initrd, "serial_drv")
        .expect("[init] FATAL: serial_drv not found in initrd");
    kprintln!("[init] Found '{}' in initrd ({} bytes)",
        serial_drv_elf.name, serial_drv_elf.data.len());

    // --- 7d. Validate the ELF header ---
    let ehdr = fs::elf::validate_header(serial_drv_elf.data)
        .expect("[init] FATAL: serial_drv is not a valid ELF64 executable");
    // Copy packed fields to locals to avoid unaligned reference UB
    let elf_entry = ehdr.e_entry;
    let elf_phnum = ehdr.e_phnum;
    let elf_phoff = ehdr.e_phoff;
    kprintln!("[elf] Valid ELF64: entry={:#010X} phnum={} phoff={:#X}",
        elf_entry, elf_phnum, elf_phoff);

    // --- 7e. Create Process for serial_drv (isolated address space) ---
    //
    // Sprint 9: Each user program gets its own Process, which owns:
    //   - A unique PML4 (user half zeroed, kernel half mirrored)
    //   - A CNode (capability table)
    //   - A PID (auto-incremented)
    //
    // The ELF segments and user stack are mapped into the Process's PML4,
    // NOT the kernel's. This is the isolation boundary.
    let serial_proc = {
        use alloc::boxed::Box;
        use sched::process::Process;

        let proc = Box::new(Process::new("serial-drv"));
        kprintln!("[init] Created Process '{}' (PID={}, PML4={:#010X})",
            proc.name_str(), proc.pid, proc.pml4().as_u64());
        Box::into_raw(proc)
    };

    // --- 7f. Load ELF segments into the serial_drv's PML4 ---
    let serial_pml4 = unsafe { (*serial_proc).pml4() };
    let elf_result = fs::elf::load(serial_drv_elf.data, serial_pml4)
        .expect("[init] FATAL: failed to load serial_drv ELF");

    kprintln!("[elf] Loaded into PID {} PML4: entry={:#010X} pages={} copied={} bss={}",
        unsafe { (*serial_proc).pid },
        elf_result.entry_point, elf_result.pages_mapped,
        elf_result.bytes_copied, elf_result.bss_zeroed);

    // --- 7g. Map user stack into serial_drv's PML4 ---
    let user_stack_top = {
        use memory::vmm::{self, PageTableFlags};

        let stack_phys = memory::pmm::alloc_frame_zeroed()
            .expect("[init] FATAL: cannot allocate user stack page");
        let stack_virt = memory::address::VirtAddr::new(0x0000_0000_0080_0000);

        unsafe {
            vmm::map_page(serial_pml4, stack_virt, stack_phys,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE
                    | PageTableFlags::USER | PageTableFlags::NO_EXECUTE,
            ).expect("[init] FATAL: cannot map user stack page");
        }

        let top = stack_virt.as_u64() + memory::address::PAGE_SIZE as u64;
        kprintln!("[init] User stack mapped in PID {} PML4: {:#010X} — {:#010X}",
            unsafe { (*serial_proc).pid }, stack_virt.as_u64(), top);
        top
    };

    // --- 7h. Create IPC endpoint for driver commands ---
    let drv_ep = alloc::boxed::Box::new(ipc::endpoint::Endpoint::new(2));
    let drv_ep_ptr = alloc::boxed::Box::into_raw(drv_ep);
    unsafe { arch::syscall::register_endpoint(drv_ep_ptr); }
    unsafe { SERIAL_DRV_ENDPOINT = drv_ep_ptr; }
    kprintln!("[init] Registered serial driver endpoint (ID=2)");

    // --- 7i. Install capabilities into serial_drv's Process CNode ---
    unsafe {
        use cap::cnode::{CapObject, CapRights, Capability};

        // Slot 0: IoPort capability for COM1 (0x3F8-0x3FF, 8 ports)
        (*serial_proc).cnode.insert_at(0, Capability::new(
            CapObject::IoPort { base: 0x3F8, size: 8 },
            CapRights::ALL,
        )).expect("[init] FATAL: cannot install IoPort capability");

        // Slot 1: Endpoint capability for command reception (READ only)
        (*serial_proc).cnode.insert_at(1, Capability::new(
            CapObject::Endpoint { id: 2 },
            CapRights::READ,
        )).expect("[init] FATAL: cannot install Endpoint capability");

        // Slot 2: Interrupt capability for COM1 IRQ 4
        (*serial_proc).cnode.insert_at(2, Capability::new(
            CapObject::Interrupt { irq: 4 },
            CapRights::READ,
        )).expect("[init] FATAL: cannot install Interrupt capability");
    }

    kprintln!("[init] Serial driver CNode (Process PID={}):",
        unsafe { (*serial_proc).pid });
    kprintln!("[init]   Slot 0: IoPort(0x3F8, 8) [ALL]");
    kprintln!("[init]   Slot 1: Endpoint(id=2) [READ]");
    kprintln!("[init]   Slot 2: Interrupt(irq=4) [READ]");

    // --- 7j. Spawn serial_drv thread owned by its Process ---
    {
        let serial_drv_thread = arch::syscall::spawn_user(
            "serial-drv", elf_result.entry_point, user_stack_top, serial_proc
        );
        sched::scheduler::spawn_thread(serial_drv_thread);
    }

    // --- 7k. Create kernel pseudo-process (PID 0) ---
    //
    // The kernel process uses KERNEL_PML4 and an empty CNode.
    // The BSP main thread and kern-init thread both belong to this process.
    let kernel_proc = {
        use alloc::boxed::Box;
        use sched::process::Process;

        let proc = Box::new(Process::kernel());
        kprintln!("[init] Created kernel Process (PID={}, PML4={:#010X})",
            proc.pid, proc.pml4().as_u64());
        Box::into_raw(proc)
    };

    // --- 7l. Spawn kernel "init" thread ---
    sched::scheduler::spawn("kern-init", kernel_init_thread, 0, kernel_proc);

    // --- 7m. Initialize scheduler (BSP thread belongs to kernel process) ---
    sched::scheduler::init(kernel_proc);

    // --- 7n. Start Application Processors ---
    arch::smp::init();

    // =========================================================================
    // BSP IDLE LOOP
    // =========================================================================
    kprintln!();
    kprintln!("==========================================================");
    kprintln!("  Sprint 9 — Process Isolation + CR3 Swap LIVE!");
    kprintln!("  serial_drv runs in isolated Process (PID 1, own PML4)");
    kprintln!("  ELF loaded from initrd.tar into per-process page tables");
    kprintln!("  CR3 swapped on context switch for address space isolation");
    kprintln!("  Ring 3 execution with capability-gated IPC + port I/O");
    kprintln!("  BSP entering idle loop.");
    kprintln!("==========================================================");

    loop {
        arch::cpu::halt();
    }
}

// =============================================================================
// Sprint 7 Infrastructure — Userspace Serial Driver
// =============================================================================

/// Global pointer to the serial driver's command endpoint (ID=2).
/// Written once during Phase 7b, read by the kernel init thread.
static mut SERIAL_DRV_ENDPOINT: *mut ipc::endpoint::Endpoint = core::ptr::null_mut();

/// Returns a reference to the serial driver's command endpoint.
///
/// # Safety
/// Must only be called after Phase 7b has initialized SERIAL_DRV_ENDPOINT.
unsafe fn get_serial_drv_endpoint() -> &'static ipc::endpoint::Endpoint {
    unsafe { &*SERIAL_DRV_ENDPOINT }
}

/// Kernel "init" thread — sends test characters to the serial driver.
///
/// This thread runs in Ring 0 and sends IPC messages to the serial
/// driver's command endpoint (EP2). The driver receives them via
/// SYS_RECV and writes each character to COM1 via SYS_PORT_OUT.
///
/// Proves the full microkernel pipeline:
///   Kernel init → ep.send() → serial_drv SYS_RECV → SYS_PORT_OUT → COM1
pub extern "C" fn kernel_init_thread(_arg: u64) {
    kprintln!("[kern-init] Kernel init thread started");

    // Give the serial driver time to boot and print its banner.
    // Simple spin delay — about 1M iterations ≈ a few ms on real hardware.
    // QEMU emulation makes PAUSE much slower, so keep this small.
    for _ in 0..1_000_000u64 {
        core::hint::spin_loop();
    }

    kprintln!("[kern-init] Sending test message to serial driver via EP2...");

    let ep = unsafe { get_serial_drv_endpoint() };

    // IPC label 0x01 = CMD_PRINT_CHAR (matches serial_drv's constant)
    let test_msg = b"[kern-init] IPC->Ring3->COM1 OK!\r\n";
    for &ch in test_msg.iter() {
        let msg = ipc::message::IpcMessage::with_data(0x01, [ch as u64, 0, 0, 0]);
        ep.send(&msg);
    }

    kprintln!("[kern-init] ================================================");
    kprintln!("[kern-init] SUCCESS: All characters sent to serial driver!");
    kprintln!("[kern-init] Ring 3 serial driver + IPC + port I/O PROVEN!");
    kprintln!("[kern-init] ================================================");

    loop { arch::cpu::halt(); }
}
