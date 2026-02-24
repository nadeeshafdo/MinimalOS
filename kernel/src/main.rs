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

// =============================================================================
// Imports
// =============================================================================

use arch::boot;
use arch::serial::SERIAL;
use memory::address;
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
    // PHASE 4: "Can Think" → Scheduler + Processes (Sprint 3-4)
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 4: Scheduler — NOT YET IMPLEMENTED");
    kprintln!("[init]   TODO: GDT + TSS");
    kprintln!("[init]   TODO: IDT + exception handlers");
    kprintln!("[init]   TODO: LAPIC + I/O APIC");
    kprintln!("[init]   TODO: Capability subsystem");
    kprintln!("[init]   TODO: IPC subsystem");
    kprintln!("[init]   TODO: Process + Thread management");
    kprintln!("[init]   TODO: Tickless scheduler");
    kprintln!("[init]   TODO: SMP init (start AP cores)");

    // =========================================================================
    // PHASE 5: "Alive" → Userspace (Sprint 5-6)
    // =========================================================================
    kprintln!();
    kprintln!("[init] Phase 5: Userspace — NOT YET IMPLEMENTED");
    kprintln!("[init]   TODO: SYSCALL/SYSRET setup");
    kprintln!("[init]   TODO: ELF loader");
    kprintln!("[init]   TODO: Load init process");
    kprintln!("[init]   TODO: Enter Ring 3");

    // =========================================================================
    // HALT
    // =========================================================================
    //
    // For now, we've done all we can. The kernel has successfully:
    //   ✓ Initialized serial output
    //   ✓ Parsed boot information (memory map, kernel location)
    //   ✓ Initialized framebuffer console
    //   ✓ Displayed boot information
    //
    // We halt the CPU in a loop. In future sprints, this is where we'd
    // enter the scheduler loop instead.
    // =========================================================================
    kprintln!();
    kprintln!("==========================================================");
    kprintln!("  Sprint 2 complete — memory management initialized!");
    kprintln!("  PMM, kernel heap, VMM infrastructure ready.");
    kprintln!("  Halting CPU. Next: Sprint 3 (interrupts + exceptions)");
    kprintln!("==========================================================");

    // Halt forever. In the future, this becomes: scheduler::run()
    arch::cpu::halt_forever()
}
