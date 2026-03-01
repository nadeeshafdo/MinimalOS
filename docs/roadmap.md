---
title: Roadmap
layout: default
nav_order: 5
---

# Roadmap
{: .no_toc }

Development plan from bare-metal boot to self-hosting OS.
{: .fs-6 .fw-300 }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Sprint Overview

| Sprint | Focus | Status | Description |
|:-------|:------|:-------|:------------|
| 1 | [Boot & Serial]({% link subsystems/boot.md %}) | ✅ Complete | Toolchain, bootloader, serial UART, framebuffer console |
| 2 | [Memory]({% link subsystems/memory.md %}) | ✅ Complete | PMM bitmap allocator, VMM page tables, kernel heap |
| 3 | [Interrupts]({% link subsystems/interrupts.md %}) | ✅ Complete | GDT/TSS, IDT, ACPI/MADT, LAPIC, I/O APIC, W^X remap |
| 4 | [Scheduler]({% link subsystems/scheduler.md %}) | 🔲 Planned | Processes, threads, tickless scheduler, SMP |
| 5 | [Capabilities]({% link subsystems/capabilities.md %}) | 🔲 Planned | Capability tables, IPC channels, memory grants |
| 6 | [Syscalls]({% link subsystems/syscalls.md %}) | 🔲 Planned | SYSCALL/SYSRET, ELF loader, Ring 3 entry |
| 7 | [Userspace]({% link subsystems/userspace.md %}) | 🔲 Planned | Init process, libmnos, first userspace driver |

---

## Sprint 1 — Boot & Serial Output ✅

> *Get the kernel running, prove it with visible output.*

- [x] Rust nightly toolchain configuration (`rust-toolchain.toml`)
- [x] Cargo workspace with kernel crate, dev/release profiles
- [x] `.cargo/config.toml` — bare-metal target, kernel code-model, no SSE/AVX, build-std
- [x] Linker script — higher-half kernel, page-aligned sections
- [x] Limine bootloader configuration
- [x] IRQ-safe ticket spinlock
- [x] `PhysAddr` / `VirtAddr` newtypes with HHDM translation
- [x] COM1 serial UART driver — 16550, 115200 baud 8N1
- [x] CPU primitives — halt, CR2/CR3, INVLPG, RDTSC, MSRs
- [x] Limine boot protocol interface
- [x] `kprint!` / `kprintln!` logging macros
- [x] Kernel panic handler
- [x] Framebuffer text console — 8×16 bitmap font, scrolling
- [x] Kernel entry point — 5-phase boot
- [x] Makefile build system
- [x] Boots in QEMU with UEFI

---

## Sprint 2 — Memory Management ✅

> *Teach the kernel to manage physical and virtual memory.*

- [x] **Physical Memory Manager** — bitmap allocator (alloc, free, zeroed, contiguous)
- [x] **VMM page table infrastructure** (map, unmap, translate)
- [x] **Kernel Heap** — linked-list allocator with coalescing, enables `alloc` crate
- [x] **Linker script fix** — `.got` section handling
- [x] Kernel higher-half remap (completed in Sprint 3)
- [x] W^X enforcement via remap (completed in Sprint 3)

---

## Sprint 3 — Interrupts & Exceptions ✅

> *Handle CPU exceptions and hardware interrupts safely.*

- [x] **GDT** — kernel/user segments, TSS with IST1 guard page, SYSRET layout
- [x] **IDT** — 15 exception handlers + 16 IRQ stubs + spurious @255 (swapgs + SysV ABI)
- [x] **ACPI** — XSDT-first MADT parser, Limine rev3 HHDM gap fix
- [x] **LAPIC** — CPUID 0x15 + PIT calibration, one-shot mode, spurious handler
- [x] **I/O APIC** — MADT parsing, IRQ routing, legacy PIC disable
- [x] **W^X remap** — in-place page table update (.text=R+X, .rodata=R, .data/.bss=R+W+NX)

---

## Sprint 4 — Processes & Scheduler 🔲

> *Run multiple threads of execution, share the CPU fairly.*

- [ ] **Process / Thread structures** — address space, capability table, state machine
- [ ] **Context switching** — register save/restore, CR3 switch, FPU lazy state
- [ ] **Tickless scheduler** — per-core run queues, work-stealing, idle thread
- [ ] **SMP initialization** — INIT/SIPI, per-core GDT/IDT/TSS/LAPIC

---

## Sprint 5 — Capabilities & IPC 🔲

> *The security model — unforgeable tokens and message passing.*

- [ ] **Capability table** — per-process, slot-based, typed with rights bitmask
- [ ] **Capability operations** — create, delete, transfer (rights reduction), revoke
- [ ] **Synchronous IPC** — send/receive on capability-protected endpoints
- [ ] **Call/Reply RPC** — synchronous request-response pattern
- [ ] **Notifications** — lightweight non-blocking event signaling
- [ ] **Zero-copy memory grants** — share physical pages via capabilities

---

## Sprint 6 — Syscall Interface & Userspace 🔲

> *Cross the Ring 0/Ring 3 boundary.*

- [ ] **SYSCALL/SYSRET** — MSR configuration, entry point, register save/restore
- [ ] **Syscall dispatch** — ~22 syscalls indexed by RAX
- [ ] **ELF loader** — parse ELF64, map segments, set up user stack
- [ ] **Ring 3 entry** — user page tables, SYSRET to entry point

---

## Sprint 7 — Init Process & First Program 🔲

> *Life outside the kernel.*

- [ ] **Init process** — capability receiver, service spawner, name directory
- [ ] **Userspace library (`libmnos`)** — syscall wrappers, IPC helpers, allocator
- [ ] **First userspace driver** — serial console via IRQ capability + IPC

---

## Future Milestones

These features are planned for after Sprint 7 completes the core OS:

| Milestone | Description |
|:----------|:------------|
| **VFS Service** | Virtual filesystem abstraction in userspace |
| **Initramfs** | In-memory tar filesystem for boot-time binaries |
| **Disk Driver** | AHCI/NVMe driver in userspace |
| **Filesystem** | ext2 or FAT32 implementation as userspace service |
| **Networking** | TCP/IP stack as a userspace service |
| **Shell** | Basic interactive command-line shell |
| **USB HID** | Keyboard and mouse drivers |
| **GPU Driver** | Intel HD 405 framebuffer driver |
| **Compositor** | Window manager / display server |
| **Rust std Port** | Port Rust's standard library to MinimalOS |
| **Self-hosting** | Compile MinimalOS on MinimalOS |

---

## Contributing

Contributions are welcome! The project is structured around sprints — each sprint focuses on a self-contained subsystem.

### How to Contribute

1. Check the current sprint's open items above
2. Read the relevant [subsystem documentation]({% link subsystems/index.md %})
3. Fork the repository and create a feature branch
4. Implement your changes with tests where applicable
5. Submit a pull request

### Development Guidelines

- **Rust edition 2024** with `no_std` — no standard library
- **No unsafe without justification** — document why each `unsafe` block is sound
- **Comments explain WHY**, not what — assume the reader knows Rust
- **One commit per logical change** — granular, descriptive commit messages
- **Test in QEMU** — `make run` before submitting
