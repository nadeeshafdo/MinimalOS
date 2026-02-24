# MinimalOS NextGen — Project Progress

> Capability-based microkernel for x86_64, written in Rust.
> Target hardware: Intel Pentium N3710 (HP 15-ay028tu) — 4-core Airmont, 8 GB DDR3L.

---

## Architecture Overview

| Layer | Description |
|-------|-------------|
| **Kernel** | Policy-free microkernel (~22 syscalls). Provides: address space isolation, capability enforcement, IPC delivery, CPU scheduling, interrupt routing, memory grants. |
| **Userspace** | Everything else — drivers, filesystems, networking, GUI — runs as unprivileged processes communicating via IPC. |
| **Boot** | Limine v8.6.0 (UEFI), higher-half kernel at `0xFFFFFFFF80200000`. |

---

## Sprint Progress

### Sprint 1 — Boot & Serial Output
> *Get the kernel running, prove it with visible output.*

- [x] Rust nightly toolchain configuration (`rust-toolchain.toml`)
- [x] Cargo workspace with kernel crate, dev/release profiles
- [x] `.cargo/config.toml` — bare-metal target, kernel code-model, no SSE/AVX, build-std
- [x] Linker script — higher-half kernel, page-aligned sections, boundary symbols
- [x] Limine bootloader configuration (`boot/limine.conf`)
- [x] IRQ-safe ticket spinlock (`sync/spinlock.rs`)
- [x] `PhysAddr` / `VirtAddr` newtypes with HHDM translation (`memory/address.rs`)
- [x] COM1 serial UART driver — 16550, 115200 baud 8N1 (`arch/x86_64/serial.rs`)
- [x] CPU primitives — halt, CR2/CR3, INVLPG, RDTSC, MSRs (`arch/x86_64/cpu.rs`)
- [x] Limine boot protocol interface — HHDM, memory map, framebuffer, RSDP, kernel address (`arch/x86_64/boot.rs`)
- [x] `kprint!` / `kprintln!` logging macros (`util/logger.rs`)
- [x] Kernel panic handler — serial output + halt (`util/panic.rs`)
- [x] Framebuffer text console — 8×16 bitmap font, scrolling (`drivers/framebuffer.rs`)
- [x] Kernel entry point — 5-phase boot, memory map dump (`main.rs`)
- [x] Makefile build system — `make`, `make iso`, `make run`, `make run-headless`
- [x] Boots in QEMU with UEFI (OVMF) — serial + framebuffer output verified

---

### Sprint 2 — Memory Management
> *Teach the kernel to manage physical and virtual memory.*

- [x] **Physical Memory Manager (PMM)** — bitmap allocator
  - [x] Parse Limine memory map, count usable pages
  - [x] Bitmap data structure (1 bit per 4 KiB page)
  - [x] `pmm::alloc_frame()` → `PhysAddr`
  - [x] `pmm::free_frame(PhysAddr)`
  - [x] `pmm::alloc_frame_zeroed()` — zeroed pages for page tables
  - [x] `pmm::alloc_contiguous(count)` — physically contiguous frames
  - [x] Mark kernel, framebuffer, ACPI, and bitmap regions as used
  - [x] Statistics: total, used, free frame counts
  - [x] Optimized u64-at-a-time scanning + byte-at-a-time range clear
- [x] **Virtual Memory Manager (VMM)** — 4-level page table infrastructure
  - [x] `PageTableFlags` bitflags (R/W, User, NX, Global, Huge, etc.)
  - [x] `PageTableEntry` type with address/flags extraction
  - [x] `PageTable` — 512-entry, 4 KiB-aligned table type
  - [x] `vmm::map_page(pml4, virt, phys, flags)` — walk/create page tables
  - [x] `vmm::unmap_page(pml4, virt)` — remove mapping
  - [x] `vmm::translate(pml4, virt)` — virtual-to-physical translation
  - [x] `vmm::new_table()` — allocate zeroed page table from PMM
  - [x] TLB flush helpers (`flush()` single page, `flush_all()`)
  - [ ] Kernel higher-half remap (deferred to Sprint 3 — needs IDT for debugging)
  - [ ] W^X enforcement via remap (deferred to Sprint 3)
- [x] **Kernel Heap Allocator**
  - [x] Linked-list free-list allocator with coalescing
  - [x] `GlobalAlloc` implementation with spinlock
  - [x] `#[global_allocator]` + `#[alloc_error_handler]`
  - [x] 256 KiB initial heap (64 contiguous pages via PMM + HHDM)
  - [x] Enable `alloc` crate (`Box`, `Vec`, `String` in kernel)
  - [x] Verified: alloc + dealloc + coalescing tested in QEMU boot
- [x] **Linker script updated** — `.got` section handling for alloc/PMM code

---

### Sprint 3 — Interrupts & Exceptions
> *Handle CPU exceptions and hardware interrupts safely.*

- [ ] **GDT (Global Descriptor Table)**
  - [ ] Kernel code/data segments (Ring 0)
  - [ ] User code/data segments (Ring 3)
  - [ ] TSS (Task State Segment) — per-core, with IST stacks
- [ ] **IDT (Interrupt Descriptor Table)**
  - [ ] Exception handlers (0–31): divide error, page fault, GPF, double fault, etc.
  - [ ] Page fault handler with detailed diagnostics (CR2 + error code)
  - [ ] Double fault handler on separate IST stack
- [ ] **LAPIC (Local APIC)**
  - [ ] LAPIC timer calibration using PIT or TSC
  - [ ] One-shot timer mode for tickless scheduling
  - [ ] Spurious interrupt handler
- [ ] **I/O APIC**
  - [ ] Parse MADT (ACPI table) for I/O APIC configuration
  - [ ] IRQ routing table
  - [ ] Redirect legacy IRQs (keyboard, serial, etc.)

---

### Sprint 4 — Processes & Scheduler
> *Run multiple threads of execution, share the CPU fairly.*

- [ ] **Process & Thread structures**
  - [ ] Process: address space, capability table, thread list
  - [ ] Thread: kernel stack, saved registers, state (Ready/Running/Blocked/Dead)
  - [ ] Per-thread kernel stack allocation
- [ ] **Context Switching**
  - [ ] Save/restore registers (GPRs, RSP, RIP, RFLAGS, FS/GS base)
  - [ ] Switch page tables (CR3)
  - [ ] FPU/SSE state lazy save/restore (if needed by userspace)
- [ ] **Tickless Scheduler**
  - [ ] Per-core run queues with priority levels
  - [ ] Work-stealing across cores
  - [ ] `scheduler::yield_now()`, `scheduler::block()`, `scheduler::wake()`
  - [ ] Idle thread per core (halts CPU when nothing to run)
- [ ] **SMP Initialization**
  - [ ] Parse MADT for AP (Application Processor) entries
  - [ ] Send INIT/SIPI to boot AP cores
  - [ ] Per-core GDT, IDT, TSS, LAPIC setup
  - [ ] Per-core scheduler run queue

---

### Sprint 5 — Capabilities & IPC
> *The security model — unforgeable tokens and message passing.*

- [ ] **Capability Table**
  - [ ] Per-process capability table (slot-based)
  - [ ] Capability types: Memory, IPC Endpoint, Interrupt, Process, Thread
  - [ ] Rights bitmask: Read, Write, Execute, Grant, Revoke
  - [ ] `cap_create()`, `cap_delete()`, `cap_transfer()`
- [ ] **IPC (Inter-Process Communication)**
  - [ ] Synchronous send/receive on capability-protected endpoints
  - [ ] Opaque message format (bytes + capability transfers)
  - [ ] Zero-copy memory grants (share physical pages via capabilities)
  - [ ] Call/Reply pattern for RPC-style communication
  - [ ] Notification objects (lightweight event signaling)

---

### Sprint 6 — Syscall Interface & Userspace Entry
> *Cross the Ring 0/Ring 3 boundary.*

- [ ] **SYSCALL/SYSRET setup**
  - [ ] Configure MSRs: STAR, LSTAR, SFMASK
  - [ ] Syscall entry point (save user state, switch to kernel stack)
  - [ ] Syscall dispatch table (~22 syscalls)
- [ ] **ELF Loader**
  - [ ] Parse ELF64 headers (program headers, entry point)
  - [ ] Map ELF segments into process address space
  - [ ] Set up user stack
- [ ] **Ring 3 Entry**
  - [ ] Switch to user page tables
  - [ ] SYSRET to user entry point
  - [ ] Verify that syscalls round-trip correctly

---

### Sprint 7 — Init Process & First Real Program
> *Life outside the kernel.*

- [ ] **Init process**
  - [ ] Minimal init that receives capabilities from kernel
  - [ ] Spawn child processes
  - [ ] IPC-based service registration
- [ ] **Userspace library (`libmnos`)**
  - [ ] Syscall wrappers (safe Rust API)
  - [ ] IPC message helpers
  - [ ] Memory allocation (userspace heap)
- [ ] **First userspace driver**
  - [ ] Serial console driver in userspace (receives IRQ capability)
  - [ ] Demonstrates: IPC, capabilities, interrupt routing

---

## Future Milestones (Post-Sprint 7)

- [ ] VFS (Virtual File System) service in userspace
- [ ] In-memory initramfs (tar) unpacking
- [ ] Disk driver (AHCI/NVMe) in userspace
- [ ] ext2/FAT32 filesystem service
- [ ] Networking stack (TCP/IP as userspace service)
- [ ] Basic shell
- [ ] USB HID (keyboard/mouse) driver
- [ ] Intel HD 405 GPU framebuffer driver
- [ ] Compositor / window manager
- [ ] Port Rust's `std` library to MinimalOS
- [ ] Self-hosting (compile MinimalOS on MinimalOS)

---

## Build & Run Quick Reference

```bash
make              # Build kernel (debug)
make release      # Build kernel (release, LTO)
make iso          # Build + create bootable ISO
make run          # Build + ISO + boot in QEMU (serial to terminal)
make run-headless # Automated headless boot, serial captured to file
make clean        # Remove build artifacts
make help         # Show all targets
```

---

*Last updated: 2026-02-24*
