---
title: Architecture
layout: default
nav_order: 3
has_children: true
---

# Architecture
{: .no_toc }

The design and structure of the MinimalOS NextGen kernel.
{: .fs-6 .fw-300 }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Microkernel Design

MinimalOS NextGen follows a **strict microkernel** architecture. The kernel provides only the minimal set of mechanisms required to build a complete operating system — all policy decisions are made in userspace.

```
┌─────────────────────────────────────────────────────────────┐
│                      USERSPACE (Ring 3)                      │
│                                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │  Init     │  │  Serial  │  │  VFS     │  │  Network │    │
│  │  Process  │  │  Driver  │  │  Service │  │  Stack   │    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘    │
│       │              │              │              │          │
│       └──────────────┴──────────────┴──────────────┘          │
│                           IPC                                │
├──────────────────────────────────────────────────────────────┤
│                    KERNEL (Ring 0)                            │
│                                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ Address  │  │ Capabil- │  │   IPC    │  │ Sched-   │    │
│  │ Spaces   │  │  ities   │  │ Delivery │  │  uler    │    │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────┤    │
│  │ Interrupt│  │ Memory   │  │          │  │          │    │
│  │ Routing  │  │ Grants   │  │          │  │          │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
├──────────────────────────────────────────────────────────────┤
│                    HARDWARE (x86_64)                          │
└──────────────────────────────────────────────────────────────┘
```

### Kernel Responsibilities (6 services)

1. **Address space isolation** — Create and manage per-process page tables
2. **Capability enforcement** — Validate unforgeable tokens on every resource access
3. **IPC message delivery** — Transfer opaque byte buffers + capabilities between processes
4. **CPU time multiplexing** — Tickless scheduler with per-core run queues
5. **Interrupt routing** — Deliver hardware interrupts to capability holders
6. **Memory grant transfers** — Zero-copy page sharing between address spaces

### What the kernel does NOT do

- No filesystem code in the kernel
- No device drivers (except boot-critical serial/framebuffer)
- No networking
- No process management policy
- No graphical rendering
- No user authentication

All of these are implemented as userspace services communicating via IPC.

---

## Capability-Based Security

Instead of traditional Unix-style permissions (user/group/other), MinimalOS uses **capabilities** — unforgeable tokens that grant specific rights to specific resources.

### Key Properties

- **No ambient authority** — A process can only access resources for which it holds a capability. There is no "root" user.
- **Principle of least privilege** — Capabilities carry specific rights flags (Read, Write, Execute, Grant, Revoke). A process only gets the rights it needs.
- **Delegatable** — A process can transfer (grant) capabilities to other processes via IPC, enabling controlled sharing.
- **Revocable** — The creator of a capability can revoke it, instantly removing access.

### Capability Types (Planned)

| Type | Description |
|:-----|:------------|
| **Memory** | Access to a region of physical memory (for DMA, MMIO) |
| **IPC Endpoint** | Permission to send/receive on a communication channel |
| **Interrupt** | Right to receive a specific hardware interrupt |
| **Process** | Control over a process (suspend, resume, kill) |
| **Thread** | Control over a thread within a process |

---

## Memory Layout

### Virtual Address Space

```
0xFFFFFFFFFFFFFFFF  ┬─────────────────────────────────────┐
                    │  (reserved / guard)                  │
0xFFFFFFFF80200000  ├─────────────────────────────────────┤
                    │  Kernel (.text, .rodata, .data, .bss)│
0xFFFFFFFF80000000  ├─────────────────────────────────────┤ ← Kernel base
                    │  (2MB padding)                       │
                    ├─────────────────────────────────────┤
                    │                                      │
                    │  Higher Half Direct Map (HHDM)       │
                    │  All physical RAM mapped here        │
                    │  Offset: ~0xFFFF800000000000         │
                    │                                      │
                    ├─────────────────────────────────────┤
                    │                                      │
                    │  Canonical hole (not addressable)     │
                    │  x86_64 architecture gap              │
                    │                                      │
0x00007FFFFFFFFFFF  ├─────────────────────────────────────┤
                    │                                      │
                    │  User address space                   │
                    │  (One per process, full lower half)   │
                    │                                      │
0x0000000000000000  └─────────────────────────────────────┘
```

### Kernel Sections

Each section is page-aligned (4 KiB) for W^X enforcement:

| Section | Permissions | Contents |
|:--------|:-----------|:---------|
| `.text` | Read + Execute | Executable code |
| `.rodata` | Read only | String literals, constants, Limine requests, `.got` |
| `.data` | Read + Write | Initialized mutable globals |
| `.bss` | Read + Write | Zero-initialized mutable globals |

The linker script places the kernel at `0xFFFFFFFF80200000` (higher-half base + 2MB offset). Limine sets up the page tables at boot.

### Physical Memory Management

The kernel uses a **bitmap allocator** for physical frames:

- 1 bit per 4 KiB page frame
- Tracks only up to the highest usable address (avoids wasting bitmap space on PCI MMIO holes)
- Optimized scanning: u64-at-a-time search for free frames
- Reserved regions (kernel, framebuffer, ACPI, bitmap itself) are marked as used at init

---

## Boot Sequence

MinimalOS boots via the [Limine](https://limine-bootloader.org/) boot protocol:

### Firmware → Bootloader → Kernel

1. **Power on** → UEFI firmware initializes hardware
2. **UEFI** loads Limine from the EFI System Partition
3. **Limine** reads `boot/limine.conf`, loads the kernel ELF
4. **Limine** sets up 64-bit long mode with paging:
   - Identity map of low memory
   - Higher-half kernel map at `0xFFFFFFFF80000000+`
   - HHDM (all physical RAM accessible at a fixed offset)
5. **Limine** fills in request structures and jumps to `kmain()`

### Kernel Initialization (5 Phases)

| Phase | Name | Description |
|:------|:-----|:------------|
| 1 | **Deaf and Blind** | Initialize COM1 serial UART. After this, `kprintln!()` works. |
| 2 | **Can See** | Parse boot info (HHDM, memory map, framebuffer, ACPI). Init framebuffer console. |
| 3 | **Can Remember** | Init PMM (bitmap allocator), kernel heap (`Vec`, `Box`, `String` available). VMM infrastructure ready. |
| 4 | **Can Think** | *(Planned)* GDT/IDT, LAPIC, I/O APIC, scheduler, SMP. |
| 5 | **Alive** | *(Planned)* SYSCALL/SYSRET, ELF loader, init process, enter Ring 3. |

---

## Syscall Interface (Planned)

The kernel will expose approximately 22 syscalls, grouped by subsystem:

### Memory Syscalls
- `sys_mem_map` — Map physical pages into an address space
- `sys_mem_unmap` — Remove a page mapping
- `sys_mem_grant` — Share pages with another process (zero-copy)
- `sys_mem_alloc` — Allocate physical frames

### IPC Syscalls
- `sys_ipc_send` — Send a message to an endpoint
- `sys_ipc_recv` — Receive a message from an endpoint
- `sys_ipc_call` — Send + wait for reply (RPC pattern)
- `sys_ipc_reply` — Reply to a call
- `sys_ipc_notify` — Lightweight event signal

### Capability Syscalls
- `sys_cap_create` — Create a new capability
- `sys_cap_delete` — Destroy a capability
- `sys_cap_transfer` — Transfer a capability via IPC
- `sys_cap_revoke` — Revoke a previously granted capability
- `sys_cap_inspect` — Query capability type and rights

### Process/Thread Syscalls
- `sys_proc_create` — Create a new process
- `sys_proc_destroy` — Destroy a process
- `sys_thread_create` — Create a new thread
- `sys_thread_destroy` — Destroy a thread
- `sys_thread_yield` — Voluntarily yield CPU time
- `sys_thread_block` — Block current thread
- `sys_thread_wake` — Wake a blocked thread

### System Syscalls
- `sys_irq_bind` — Bind an interrupt to a capability endpoint
- `sys_debug_write` — Write to serial console (debug only)

---

## Technology Stack

| Component | Choice | Rationale |
|:----------|:-------|:----------|
| **Language** | Rust (nightly) | Memory safety, zero-cost abstractions, `no_std` support |
| **Target** | `x86_64-unknown-none` | Bare metal, no OS dependency |
| **Bootloader** | Limine v8.6.0 | UEFI support, higher-half mapping, rich boot protocol |
| **Build** | Cargo + Make | Cargo for Rust, Make for ISO & QEMU orchestration |
| **CI** | GitHub Actions | Automated build, ISO creation, release management |
| **Edition** | 2024 | Latest Rust edition |
| **Key crates** | `limine`, `spin`, `bitflags`, `x86_64`, `log` | Minimal, audited dependencies for Ring 0 |
